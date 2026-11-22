package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"sort"
	"strings"
	"sync"
	"time"

	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/propagation"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
	"go.opentelemetry.io/otel/sdk/trace/tracetest"
	"go.opentelemetry.io/otel/trace"

	weft "github.com/weft-sdk/weft/go"
	"github.com/weft-sdk/weft/go/http/nethttp"
	"github.com/weft-sdk/weft/go/kafka/franz"
)

// The service.* operations: a real service, started inside the runner with this language's package, on a
// real HTTP socket and a real Kafka broker, so the harness can make two languages talk.

var (
	spanExporter = tracetest.NewInMemoryExporter()
	tracingOnce  sync.Once
	quietLogger  = slog.New(slog.NewTextHandler(io.Discard, nil))
)

func installTracing() {
	tracingOnce.Do(func() {
		otel.SetTracerProvider(sdktrace.NewTracerProvider(sdktrace.WithSpanProcessor(sdktrace.NewSimpleSpanProcessor(spanExporter))))
		otel.SetTextMapPropagator(propagation.TraceContext{})
	})
}

type script struct {
	Returns    json.RawMessage   `json:"returns"`
	ReturnsRaw json.RawMessage   `json:"returnsRaw"`
	Status     int               `json:"status"`
	Headers    map[string]string `json:"headers"`
	Throws     *struct {
		Category string `json:"category"`
		Declared *struct {
			Package string          `json:"package"`
			Name    string          `json:"name"`
			Fields  json.RawMessage `json:"fields"`
			Message string          `json:"message"`
		} `json:"declared"`
	} `json:"throws"`
}

type serviceStart struct {
	ID      string `json:"svc"`
	Service string `json:"service"`
	Kafka   *struct {
		Brokers string            `json:"brokers"`
		Group   string            `json:"group"`
		Topics  map[string]string `json:"topics"`
	} `json:"kafka"`
	Consumes []struct {
		Event         string `json:"event"`
		FromBeginning *bool  `json:"fromBeginning"`
		Attempts      int    `json:"attempts"`
		Fail          *struct {
			Times    json.RawMessage `json:"times"`
			Category string          `json:"category"`
		} `json:"fail"`
	} `json:"consumes"`
	Serves []struct {
		Route   string `json:"route"`
		Handler script `json:"handler"`
	} `json:"serves"`
	Services map[string]string `json:"services"`
}

type running struct {
	weft    *weft.Weft
	http    *weft.HTTP
	adapter *nethttp.Adapter
	m       *weft.Messaging

	mu       sync.Mutex
	received []map[string]any
	served   []map[string]any
}

var (
	servicesMu sync.Mutex
	services   = map[string]*running{}
)

func yamlFor(a serviceStart) string {
	lines := []string{"service: " + a.Service, "transports:", "  http:", "    port: 0"}
	if a.Kafka != nil {
		lines = append(lines, "  kafka:", "    brokers: "+a.Kafka.Brokers, "    group: "+a.Kafka.Group)
		if len(a.Kafka.Topics) > 0 {
			lines = append(lines, "topics:")
			for _, e := range sortedKeys(a.Kafka.Topics) {
				lines = append(lines, "  "+e+": "+a.Kafka.Topics[e])
			}
		}
	}
	if len(a.Services) > 0 {
		lines = append(lines, "services:")
		for _, n := range sortedKeys(a.Services) {
			lines = append(lines, "  "+n+": "+a.Services[n])
		}
	}
	return strings.Join(lines, "\n")
}

func sortedKeys(m map[string]string) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}

func (s script) thrown(c *weft.Contracts, pkg string) error {
	t := s.Throws
	if t.Declared != nil {
		d := t.Declared
		p := d.Package
		if p == "" {
			p = pkg
		}
		wire, err := weft.ParseJSON(d.Fields)
		if err != nil {
			return err
		}
		fields, err := c.DecodeErrorFields(p, d.Name, wire)
		if err != nil {
			return err
		}
		return &weft.DeclaredError{Package: p, Name: d.Name, Fields: fields, Message: d.Message}
	}
	return weft.NewError(weft.Category(t.Category), "failed with "+t.Category)
}

func serviceStartOp(c *weft.Contracts, a serviceStart) (map[string]any, error) {
	installTracing()
	cfg, err := weft.ParseConfig(yamlFor(a), nil)
	if err != nil {
		return nil, err
	}
	w := weft.New(weft.Options{Contracts: c, Config: cfg, ShutdownGrace: 3 * time.Second})
	adapter := nethttp.New(nethttp.Options{})
	h, err := weft.NewHTTP(w, weft.HTTPOptions{Adapter: adapter, Logger: quietLogger})
	if err != nil {
		return nil, err
	}
	svc := &running{weft: w, http: h, adapter: adapter}

	for _, s := range a.Serves {
		s := s
		ir, route, err := c.Route(s.Route, "")
		if err != nil {
			return nil, err
		}
		err = h.HandleIn(ir.Package, route.Method, weft.FullPath(route), func(ctx context.Context, in *weft.HTTPInput) (any, error) {
			entry := map[string]any{"route": route.Name, "traceId": nil}
			if sc := trace.SpanContextFromContext(ctx); sc.IsValid() {
				entry["traceId"] = sc.TraceID().String()
			}
			_, hasDeadline := in.Ctx.Headers["weft-deadline"]
			_, hasTraceparent := in.Ctx.Headers["traceparent"]
			entry["hasDeadline"], entry["hasTraceparent"] = hasDeadline, hasTraceparent
			entry["seen"] = encodeInput(c, route, in)
			svc.mu.Lock()
			svc.served = append(svc.served, entry)
			svc.mu.Unlock()

			if s.Handler.Status != 0 {
				in.Ctx.Response.Status = s.Handler.Status
			}
			for k, v := range s.Handler.Headers {
				in.Ctx.Response.Headers[k] = v
			}
			if s.Handler.Throws != nil {
				return nil, s.Handler.thrown(c, ir.Package)
			}
			if len(s.Handler.ReturnsRaw) > 0 {
				var raw any
				_ = json.Unmarshal(s.Handler.ReturnsRaw, &raw)
				return raw, nil
			}
			if len(s.Handler.Returns) == 0 || string(s.Handler.Returns) == "null" {
				return nil, nil
			}
			wire, err := weft.ParseJSON(s.Handler.Returns)
			if err != nil {
				return nil, err
			}
			return c.Decode(route.Result.Value, wire)
		})
		if err != nil {
			return nil, err
		}
	}

	if a.Kafka != nil {
		m, err := weft.NewMessaging(w, weft.MessagingOptions{Adapter: franz.New(franz.Options{}), Name: "kafka", Logger: quietLogger})
		if err != nil {
			return nil, err
		}
		svc.m = m
		for _, sub := range a.Consumes {
			sub := sub
			_, def, err := c.Event(sub.Event, "")
			if err != nil {
				return nil, err
			}
			failures := 0
			from := true
			if sub.FromBeginning != nil {
				from = *sub.FromBeginning
			}
			err = m.On(sub.Event, func(ctx context.Context, payload any, mc *weft.MessageContext) error {
				if f := sub.Fail; f != nil {
					always := string(f.Times) == `"always"`
					var times int
					_ = json.Unmarshal(f.Times, &times)
					if always || failures < times {
						failures++
						return weft.NewError(weft.Category(f.Category), "failed with "+f.Category)
					}
				}
				wire, err := c.Encode(def.Payload, payload)
				if err != nil {
					return err
				}
				entry := map[string]any{
					"payload": wire, "key": nilIfEmpty(mc.Key), "attempt": mc.Attempt,
					"envelope": map[string]any{
						"type": mc.Envelope.Type, "source": mc.Envelope.Source, "datacontenttype": mc.Envelope.DataContentType,
						"hasId": mc.Envelope.ID != "", "hasTime": mc.Envelope.Time != "",
					},
					"headers": mc.Envelope.Headers, "traceId": nil, "parentSpanId": nil,
				}
				span := trace.SpanFromContext(ctx)
				if sc := span.SpanContext(); sc.IsValid() {
					entry["traceId"] = sc.TraceID().String()
				}
				if ro, ok := span.(sdktrace.ReadOnlySpan); ok && ro.Parent().IsValid() {
					entry["parentSpanId"] = ro.Parent().SpanID().String()
				}
				svc.mu.Lock()
				svc.received = append(svc.received, entry)
				svc.mu.Unlock()
				return nil
			}, weft.SubscribeOptions{FromBeginning: from, Attempts: sub.Attempts})
			if err != nil {
				return nil, err
			}
		}
	}
	if err := w.Start(context.Background()); err != nil {
		return nil, err
	}
	servicesMu.Lock()
	services[a.ID] = svc
	servicesMu.Unlock()
	return map[string]any{"httpPort": adapter.Port()}, nil
}

func nilIfEmpty(s string) any {
	if s == "" {
		return nil
	}
	return s
}

func find(id string) (*running, error) {
	servicesMu.Lock()
	defer servicesMu.Unlock()
	s, ok := services[id]
	if !ok {
		return nil, fmt.Errorf("no service %s: send service.start first", id)
	}
	return s, nil
}

type servicePublish struct {
	ID      string            `json:"svc"`
	Event   string            `json:"event"`
	Payload json.RawMessage   `json:"payload"`
	Key     string            `json:"key"`
	Headers map[string]string `json:"headers"`
	InSpan  bool              `json:"inSpan"`
}

func servicePublishOp(c *weft.Contracts, a servicePublish) (map[string]any, error) {
	s, err := find(a.ID)
	if err != nil {
		return nil, err
	}
	_, def, err := c.Event(a.Event, "")
	if err != nil {
		return nil, err
	}
	wire, err := weft.ParseJSON(a.Payload)
	if err != nil {
		return nil, err
	}
	value, err := c.Decode(def.Payload, wire)
	if err != nil {
		return nil, err
	}
	ctx := context.Background()
	var traceID any
	if a.InSpan {
		var span trace.Span
		ctx, span = otel.Tracer("test").Start(ctx, "test")
		defer span.End()
		traceID = span.SpanContext().TraceID().String()
	}
	if err := s.m.Publish(ctx, a.Event, value, weft.PublishOptions{Key: a.Key, Headers: a.Headers}); err != nil {
		return map[string]any{"ok": false, "error": describe(c, err), "traceId": traceID}, nil
	}
	return map[string]any{"ok": true, "traceId": traceID}, nil
}

func describe(c *weft.Contracts, err error) map[string]any {
	var de *weft.DeclaredError
	if errors.As(err, &de) {
		fields, _ := c.EncodeErrorFields(de.Package, de.Name, de.Fields)
		return map[string]any{"declared": map[string]any{"package": de.Package, "name": de.Name, "fields": fields}, "message": de.Error()}
	}
	we := weft.AsError(err)
	paths := []string{}
	for _, i := range we.Issues {
		paths = append(paths, i.Path)
	}
	sort.Strings(paths)
	return map[string]any{"category": string(we.Category), "message": we.Message, "issues": paths}
}

type serviceCall struct {
	ID      string `json:"svc"`
	Service string `json:"service"`
	Route   string `json:"route"`
	Request struct {
		Params  map[string]json.RawMessage `json:"params"`
		Query   json.RawMessage            `json:"query"`
		Headers json.RawMessage            `json:"headers"`
		Body    json.RawMessage            `json:"body"`
	} `json:"request"`
	InSpan         bool   `json:"inSpan"`
	TimeoutMs      int    `json:"timeoutMs"`
	IdempotencyKey string `json:"idempotencyKey"`
}

func parseOptional(raw json.RawMessage) (any, error) {
	if len(raw) == 0 {
		return map[string]any{}, nil
	}
	return weft.ParseJSON(raw)
}

func serviceCallOp(c *weft.Contracts, a serviceCall) (map[string]any, error) {
	s, err := find(a.ID)
	if err != nil {
		return nil, err
	}
	ir, route, err := c.Route(a.Route, "")
	if err != nil {
		return nil, err
	}
	o := weft.CallOptions{Package: ir.Package, IdempotencyKey: a.IdempotencyKey, Timeout: time.Duration(a.TimeoutMs) * time.Millisecond}
	if route.Body != nil {
		wire, err := parseOptional(a.Request.Body)
		if err != nil {
			return nil, err
		}
		if len(a.Request.Body) == 0 {
			wire = nil
		}
		if o.Body, err = c.Decode(*route.Body, wire); err != nil {
			return nil, err
		}
	}
	if len(route.PathParams) > 0 {
		o.Params = map[string]any{}
		for _, p := range route.PathParams {
			var wire any
			if raw, ok := a.Request.Params[p.Name]; ok {
				if wire, err = weft.ParseJSON(raw); err != nil {
					return nil, err
				}
			}
			if o.Params[p.Name], err = c.Decode(p.Type, wire); err != nil {
				return nil, err
			}
		}
	}
	for _, part := range []struct {
		ref *weft.TypeRef
		raw json.RawMessage
		set func(map[string]any)
	}{
		{route.Query, a.Request.Query, func(m map[string]any) { o.Query = m }},
		{route.Headers, a.Request.Headers, func(m map[string]any) { o.Headers = m }},
	} {
		if part.ref == nil {
			continue
		}
		wire, err := parseOptional(part.raw)
		if err != nil {
			return nil, err
		}
		value, err := c.Decode(*part.ref, wire)
		if err != nil {
			return nil, err
		}
		m, _ := value.(map[string]any)
		part.set(m)
	}

	ctx := context.Background()
	var traceID any
	if a.InSpan {
		var span trace.Span
		ctx, span = otel.Tracer("test").Start(ctx, "test")
		defer span.End()
		traceID = span.SpanContext().TraceID().String()
	}
	result, err := s.http.To(a.Service).Call(ctx, route.Method, weft.FullPath(route), o)
	if err != nil {
		return map[string]any{"ok": false, "error": describe(c, err), "traceId": traceID}, nil
	}
	var encoded any
	if result != nil {
		if encoded, err = c.Encode(route.Result.Value, result); err != nil {
			return nil, err
		}
	}
	return map[string]any{"ok": true, "result": encoded, "traceId": traceID}, nil
}

func serviceReceivedOp(a struct {
	ID        string `json:"svc"`
	Count     int    `json:"count"`
	TimeoutMs int    `json:"timeoutMs"`
}) (map[string]any, error) {
	s, err := find(a.ID)
	if err != nil {
		return nil, err
	}
	timeout := time.Duration(a.TimeoutMs) * time.Millisecond
	if timeout == 0 {
		timeout = 30 * time.Second
	}
	for deadline := time.Now().Add(timeout); time.Now().Before(deadline); time.Sleep(25 * time.Millisecond) {
		s.mu.Lock()
		n := len(s.received)
		s.mu.Unlock()
		if n >= a.Count {
			break
		}
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	messages := append([]map[string]any{}, s.received...)
	return map[string]any{"count": len(messages), "messages": messages}, nil
}

func serviceServedOp(id string) (map[string]any, error) {
	s, err := find(id)
	if err != nil {
		return nil, err
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	return map[string]any{"calls": len(s.served), "requests": append([]map[string]any{}, s.served...)}, nil
}

func serviceSpansOp() map[string]any {
	out := []map[string]any{}
	for _, s := range spanExporter.GetSpans() {
		parent := any(nil)
		if s.Parent.IsValid() {
			parent = s.Parent.SpanID().String()
		}
		out = append(out, map[string]any{
			"kind": spanKind(s.SpanKind), "name": s.Name,
			"traceId": s.SpanContext.TraceID().String(), "spanId": s.SpanContext.SpanID().String(), "parentSpanId": parent,
		})
	}
	return map[string]any{"spans": out}
}

func spanKind(k trace.SpanKind) string {
	switch k {
	case trace.SpanKindServer:
		return "SERVER"
	case trace.SpanKindClient:
		return "CLIENT"
	case trace.SpanKindProducer:
		return "PRODUCER"
	case trace.SpanKindConsumer:
		return "CONSUMER"
	}
	return "INTERNAL"
}

func serviceStopOp(id string) (map[string]any, error) {
	s, err := find(id)
	if err != nil {
		return nil, err
	}
	r := s.weft.Stop(context.Background())
	servicesMu.Lock()
	delete(services, id)
	servicesMu.Unlock()
	return map[string]any{"drained": r.Drained, "abandoned": r.Abandoned}, nil
}
