package main

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"log/slog"
	"sort"
	"strings"
	"sync"

	weft "github.com/weft-sdk/weft/go"
)

// httpRequest is the `http` operation: one request to one route, through core's HTTP handling, with a
// capturing adapter instead of a socket, so every language is held to the same decoding, problems and
// statuses.
type httpRequest struct {
	Route   string `json:"route"`
	Request struct {
		Params  map[string]string `json:"params"`
		Query   map[string]any    `json:"query"`
		Headers map[string]string `json:"headers"`
		Body    *string           `json:"body"`
	} `json:"request"`
	Handler struct {
		Returns    json.RawMessage `json:"returns"`
		ReturnsRaw json.RawMessage `json:"returnsRaw"`
		Throws     *struct {
			Category string `json:"category"`
			Plain    bool   `json:"plain"`
			Declared *struct {
				Package string          `json:"package"`
				Name    string          `json:"name"`
				Fields  json.RawMessage `json:"fields"`
				Message string          `json:"message"`
			} `json:"declared"`
		} `json:"throws"`
		Status  int               `json:"status"`
		Headers map[string]string `json:"headers"`
	} `json:"handler"`
}

type capture struct {
	handlers map[string]func(weft.HTTPRequest) weft.HTTPResponse
}

func (c *capture) Name() string { return "http" }
func (c *capture) Raw() any     { return nil }
func (c *capture) Route(method, template string, h func(weft.HTTPRequest) weft.HTTPResponse) {
	c.handlers[method+" "+template] = h
}
func (c *capture) Start(context.Context) error      { return nil }
func (c *capture) StopIntake(context.Context) error { return nil }
func (c *capture) Close(context.Context) error      { return nil }

func httpScenario(contracts *weft.Contracts, req httpRequest) (map[string]any, error) {
	ir, route, err := contracts.Route(req.Route, "")
	if err != nil {
		return nil, err
	}
	cfg, err := weft.ParseConfig("service: conformance", nil)
	if err != nil {
		return nil, err
	}
	w := weft.New(weft.Options{Contracts: contracts, Config: cfg})
	adapter := &capture{handlers: map[string]func(weft.HTTPRequest) weft.HTTPResponse{}}
	h, err := weft.NewHTTP(w, weft.HTTPOptions{Adapter: adapter, Logger: slog.New(slog.NewTextHandler(io.Discard, nil))})
	if err != nil {
		return nil, err
	}

	var mu sync.Mutex
	calls := 0
	var seen any
	err = h.HandleIn(ir.Package, route.Method, weft.FullPath(route), func(_ context.Context, in *weft.HTTPInput) (any, error) {
		mu.Lock()
		calls++
		seen = encodeInput(contracts, route, in)
		mu.Unlock()
		if req.Handler.Status != 0 {
			in.Ctx.Response.Status = req.Handler.Status
		}
		for k, v := range req.Handler.Headers {
			in.Ctx.Response.Headers[k] = v
		}
		if t := req.Handler.Throws; t != nil {
			switch {
			case t.Plain:
				return nil, errors.New("a plain exception with a secret: hunter2")
			case t.Declared != nil:
				pkg := t.Declared.Package
				if pkg == "" {
					pkg = ir.Package
				}
				wire, err := weft.ParseJSON(t.Declared.Fields)
				if err != nil {
					return nil, err
				}
				fields, err := contracts.DecodeErrorFields(pkg, t.Declared.Name, wire)
				if err != nil {
					return nil, err
				}
				return nil, &weft.DeclaredError{Package: pkg, Name: t.Declared.Name, Fields: fields, Message: t.Declared.Message}
			default:
				return nil, weft.NewError(weft.Category(t.Category), "failed with "+t.Category)
			}
		}
		if len(req.Handler.ReturnsRaw) > 0 {
			var raw any
			_ = json.Unmarshal(req.Handler.ReturnsRaw, &raw)
			return raw, nil
		}
		if len(req.Handler.Returns) == 0 || string(req.Handler.Returns) == "null" {
			return nil, nil
		}
		wire, err := weft.ParseJSON(req.Handler.Returns)
		if err != nil {
			return nil, err
		}
		return contracts.Decode(route.Result.Value, wire)
	})
	if err != nil {
		return nil, err
	}
	ctx := context.Background()
	if err := w.Start(ctx); err != nil {
		return nil, err
	}

	query := map[string][]string{}
	for k, v := range req.Request.Query {
		switch x := v.(type) {
		case string:
			query[k] = []string{x}
		case []any:
			for _, item := range x {
				query[k] = append(query[k], item.(string))
			}
		}
	}
	headers := map[string]string{}
	for k, v := range req.Request.Headers {
		headers[strings.ToLower(k)] = v
	}
	params := req.Request.Params
	if params == nil {
		params = map[string]string{}
	}
	response := adapter.handlers[route.Method+" "+weft.FullPath(route)](weft.HTTPRequest{
		Method: route.Method, Template: weft.FullPath(route), Params: params, Query: query, Headers: headers,
		Body: req.Request.Body, Context: ctx,
	})
	w.Stop(ctx)

	responseHeaders := map[string]string{}
	contentType := ""
	for k, v := range response.Headers {
		if strings.ToLower(k) == "content-type" {
			contentType, _, _ = strings.Cut(v, ";")
		} else {
			responseHeaders[strings.ToLower(k)] = v
		}
	}
	var body any
	var issues any
	if response.Body != "" {
		parsed, err := weft.ParseJSON([]byte(response.Body))
		if err != nil {
			return nil, err
		}
		body = parsed
		if m, ok := parsed.(map[string]any); ok {
			if list, ok := m["issues"].([]any); ok {
				paths := []string{}
				for _, i := range list {
					paths = append(paths, i.(map[string]any)["path"].(string))
				}
				sort.Strings(paths)
				issues = paths
			}
		}
	}
	mu.Lock()
	defer mu.Unlock()
	return map[string]any{"status": response.Status, "contentType": contentType, "body": body, "issues": issues, "headers": responseHeaders, "seen": seen, "calls": calls, "leaksSecret": strings.Contains(response.Body, "hunter2")}, nil
}

// encodeInput is what the handler was given, in wire form, so it can be compared across languages.
func encodeInput(c *weft.Contracts, route *weft.RouteDef, in *weft.HTTPInput) any {
	out := map[string]any{"params": map[string]any{}, "query": nil, "headers": nil, "body": nil}
	params := map[string]any{}
	for _, p := range route.PathParams {
		if wire, err := c.Encode(p.Type, in.Params[p.Name]); err == nil {
			params[p.Name] = wire
		}
	}
	out["params"] = params
	if route.Query != nil {
		out["query"], _ = c.Encode(*route.Query, in.Query)
	}
	if route.Headers != nil {
		out["headers"], _ = c.Encode(*route.Headers, in.Headers)
	}
	if route.Body != nil {
		out["body"], _ = c.Encode(*route.Body, in.Body)
	}
	return out
}
