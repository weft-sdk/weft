// Command weft-conformance-runner is the Go runner for the conformance harness. It speaks
// spec/conformance-protocol.md on standard input and output, and uses nothing but this module.
package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"

	weft "github.com/weft-sdk/weft/go"
)

const protocol = "0.1"

type request struct {
	ID   json.Number       `json:"id"`
	Op   string            `json:"op"`
	IR   []weft.Ir         `json:"ir"`
	Type weft.TypeRef      `json:"type"`
	Wire json.RawMessage   `json:"wire"`
	YAML string            `json:"yaml"`
	Env  map[string]string `json:"env"`
}

type response struct {
	ID     json.Number `json:"id"`
	OK     bool        `json:"ok"`
	Result any         `json:"result,omitempty"`
	Error  string      `json:"error,omitempty"`
}

func main() {
	var contracts *weft.Contracts
	in := bufio.NewScanner(os.Stdin)
	in.Buffer(make([]byte, 1<<20), 1<<28)
	out := bufio.NewWriter(os.Stdout)
	write := func(r response) {
		var buf bytes.Buffer
		enc := json.NewEncoder(&buf)
		enc.SetEscapeHTML(false)
		if err := enc.Encode(r); err != nil {
			buf.Reset()
			_ = json.NewEncoder(&buf).Encode(response{ID: r.ID, Error: "the answer cannot be written: " + err.Error()})
		}
		out.Write(buf.Bytes())
		out.Flush()
	}

	for in.Scan() {
		line := bytes.TrimSpace(in.Bytes())
		if len(line) == 0 {
			continue
		}
		var req request
		dec := json.NewDecoder(bytes.NewReader(line))
		if err := dec.Decode(&req); err != nil {
			write(response{ID: "-1", Error: "not a request: " + err.Error()})
			continue
		}
		switch req.Op {
		case "hello":
			write(response{ID: req.ID, OK: true, Result: map[string]any{"language": "go", "version": weft.Version, "protocol": protocol}})
		case "load":
			contracts = weft.NewContracts(req.IR...)
			write(response{ID: req.ID, OK: true, Result: map[string]any{}})
		case "decode":
			if contracts == nil {
				write(response{ID: req.ID, Error: "no contracts loaded: send `load` first"})
				continue
			}
			wire, err := weft.ParseJSON(req.Wire)
			if err != nil {
				write(response{ID: req.ID, Error: "the wire value is not JSON: " + err.Error()})
				continue
			}
			write(response{ID: req.ID, OK: true, Result: decode(contracts, req.Type, wire)})
		case "messaging":
			if contracts == nil {
				write(response{ID: req.ID, Error: "no contracts loaded: send `load` first"})
				continue
			}
			var m messagingRequest
			if err := json.Unmarshal(line, &m); err != nil {
				write(response{ID: req.ID, Error: "not a messaging request: " + err.Error()})
				continue
			}
			result, err := messaging(contracts, m)
			if err != nil {
				write(response{ID: req.ID, Error: err.Error()})
				continue
			}
			write(response{ID: req.ID, OK: true, Result: result})
		case "http":
			if contracts == nil {
				write(response{ID: req.ID, Error: "no contracts loaded: send `load` first"})
				continue
			}
			var h httpRequest
			if err := json.Unmarshal(line, &h); err != nil {
				write(response{ID: req.ID, Error: "not an http request: " + err.Error()})
				continue
			}
			result, err := httpScenario(contracts, h)
			if err != nil {
				write(response{ID: req.ID, Error: err.Error()})
				continue
			}
			write(response{ID: req.ID, OK: true, Result: result})
		case "service.start", "service.publish", "service.call", "service.received", "service.served", "service.spans", "service.stop":
			if contracts == nil {
				write(response{ID: req.ID, Error: "no contracts loaded: send `load` first"})
				continue
			}
			result, err := serviceOp(contracts, req.Op, line)
			if err != nil {
				write(response{ID: req.ID, Error: err.Error()})
				continue
			}
			write(response{ID: req.ID, OK: true, Result: result})
		case "config":
			config, err := weft.ParseConfig(req.YAML, req.Env)
			if err != nil {
				var ce *weft.ConfigError
				if errors.As(err, &ce) {
					write(response{ID: req.ID, OK: true, Result: map[string]any{"valid": false, "problems": ce.Problems}})
				} else {
					write(response{ID: req.ID, Error: err.Error()})
				}
				continue
			}
			write(response{ID: req.ID, OK: true, Result: map[string]any{"valid": true, "config": config}})
		default:
			write(response{ID: req.ID, Error: fmt.Sprintf("unknown op %s", req.Op)})
		}
	}
}

// decode decodes the wire value, then encodes the result again (the canonical form).
func decode(c *weft.Contracts, ref weft.TypeRef, wire any) map[string]any {
	value, err := c.Decode(ref, wire)
	if err != nil {
		return map[string]any{"valid": false, "issues": weft.AsError(err).Issues}
	}
	canonical, err := c.Encode(ref, value)
	if err != nil {
		return map[string]any{"valid": false, "issues": weft.AsError(err).Issues}
	}
	return map[string]any{"valid": true, "canonical": canonical}
}

// serviceOp runs one of the service.* operations.
func serviceOp(c *weft.Contracts, op string, line []byte) (map[string]any, error) {
	switch op {
	case "service.start":
		var a serviceStart
		if err := json.Unmarshal(line, &a); err != nil {
			return nil, err
		}
		return serviceStartOp(c, a)
	case "service.publish":
		var a servicePublish
		if err := json.Unmarshal(line, &a); err != nil {
			return nil, err
		}
		return servicePublishOp(c, a)
	case "service.call":
		var a serviceCall
		if err := json.Unmarshal(line, &a); err != nil {
			return nil, err
		}
		return serviceCallOp(c, a)
	case "service.received":
		var a struct {
			ID        string `json:"svc"`
			Count     int    `json:"count"`
			TimeoutMs int    `json:"timeoutMs"`
		}
		if err := json.Unmarshal(line, &a); err != nil {
			return nil, err
		}
		return serviceReceivedOp(a)
	case "service.served", "service.stop":
		var a struct {
			ID string `json:"svc"`
		}
		if err := json.Unmarshal(line, &a); err != nil {
			return nil, err
		}
		if op == "service.served" {
			return serviceServedOp(a.ID)
		}
		return serviceStopOp(a.ID)
	}
	return serviceSpansOp(), nil
}
