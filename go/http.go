package weft

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"net/url"
	"strings"
	"time"

	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/codes"
	"go.opentelemetry.io/otel/metric"
	"go.opentelemetry.io/otel/trace"
)

// HTTPRequest is a request as an adapter hands it to core. Nothing is decoded yet.
type HTTPRequest struct {
	Method string
	// Template is the route template that matched, such as /v1/orders/{id}.
	Template string
	Params   map[string]string
	Query    map[string][]string
	// Headers has names in lower case.
	Headers map[string]string
	// Body is the raw body, if there was one. Core parses it.
	Body    *string
	Context context.Context
}

// HTTPResponse is what core wants written back.
type HTTPResponse struct {
	Status  int
	Headers map[string]string
	Body    string
}

// HTTPAdapter is the HTTP library underneath, as core needs it. It owns the socket, the router and the
// parsing of the URL; core does the contracts, the middleware, the errors and the tracing.
type HTTPAdapter interface {
	Transport
	Raw() any
	// Route serves template (with {name} parameters) for method. Called before Start.
	Route(method, template string, handler func(HTTPRequest) HTTPResponse)
}

// HTTPSettings is implemented by adapters that read transports.http from weft.yaml.
type HTTPSettings interface {
	Configure(settings map[string]any)
}

// HTTPContext is what an HTTP handler sees besides its input.
type HTTPContext struct {
	*Call
	Route    *RouteDef
	Method   string
	Template string
	// Response: set Status and Headers to change the response. Status defaults to 200.
	Response struct {
		Status  int
		Headers map[string]string
	}
}

// HTTPInput is what a handler is given, decoded and validated against the route.
type HTTPInput struct {
	Body    any
	Params  map[string]any
	Query   map[string]any
	Headers map[string]any
	Ctx     *HTTPContext
}

// HTTPHandler handles one route. It returns the route's result in code form, or an error: an *Error
// of a category, or a *DeclaredError the route lists. Returning (nil, nil) for a route whose result is
// optional answers 404.
type HTTPHandler func(ctx context.Context, in *HTTPInput) (any, error)

// HTTPMiddleware wraps an HTTP handler.
type HTTPMiddleware func(ctx context.Context, hc *HTTPContext, next Next) error

// HTTPOptions configures an HTTP.
type HTTPOptions struct {
	// Adapter serves requests. Leave it nil to use HTTP only as a client.
	Adapter HTTPAdapter
	// Client is used for calls to other services. Default: http.DefaultClient.
	Client *http.Client
	Logger *slog.Logger
	Sleep  func(ctx context.Context, d time.Duration)
	Random func() float64
}

type registered struct {
	pkg     string
	route   *RouteDef
	handler HTTPHandler
}

// HTTP is weft.http: serve the routes of the contracts, and call other services. Serving is core's
// work over an adapter; calling uses net/http.
type HTTP struct {
	weft       *Weft
	opts       HTTPOptions
	logger     *slog.Logger
	routes     []registered
	middleware []HTTPMiddleware
	started    bool
}

// NewHTTP makes an HTTP and registers it with w.
func NewHTTP(w *Weft, o HTTPOptions) (*HTTP, error) {
	h := &HTTP{weft: w, opts: o, logger: o.Logger}
	if h.logger == nil {
		h.logger = slog.Default()
	}
	if o.Client == nil {
		h.opts.Client = http.DefaultClient
	}
	if h.opts.Sleep == nil {
		h.opts.Sleep = Sleep
	}
	if h.opts.Random == nil {
		h.opts.Random = randomFloat
	}
	if s, ok := o.Adapter.(HTTPSettings); ok {
		s.Configure(w.Config.Transports["http"])
	}
	if err := w.Register(h); err != nil {
		return nil, err
	}
	return h, nil
}

// Raw is the library underneath, if serving.
func (h *HTTP) Raw() any {
	if h.opts.Adapter == nil {
		return nil
	}
	return h.opts.Adapter.Raw()
}

// Use adds middleware for HTTP only. It runs after the middleware added to Weft.
func (h *HTTP) Use(m HTTPMiddleware) { h.middleware = append(h.middleware, m) }

// NotFound is the error to return for "nothing here".
func (h *HTTP) NotFound(message string) error {
	if message == "" {
		message = "Not found"
	}
	return NewError(NotFound, message)
}

// FullPath is the base and path of a route: /v1 + /orders/{id}.
func FullPath(r *RouteDef) string { return r.Base + r.Path }

// FindRoute finds the route a method and path name: by the full path or, if that is not ambiguous, by
// the path without its base.
func (c *Contracts) FindRoute(method, path, pkg string) (*Ir, *RouteDef, error) {
	type hit struct {
		ir *Ir
		r  *RouteDef
	}
	var all []hit
	for i := range c.packages {
		if pkg != "" && c.packages[i].Package != pkg {
			continue
		}
		for j := range c.packages[i].Routes {
			if c.packages[i].Routes[j].Method == method {
				all = append(all, hit{&c.packages[i], &c.packages[i].Routes[j]})
			}
		}
	}
	var found []hit
	for _, x := range all {
		if FullPath(x.r) == path {
			found = append(found, x)
		}
	}
	if len(found) == 0 {
		for _, x := range all {
			if x.r.Path == path {
				found = append(found, x)
			}
		}
	}
	switch len(found) {
	case 0:
		return nil, nil, errorf(Internal, "The contracts have no route %s %s", method, path)
	case 1:
		return found[0].ir, found[0].r, nil
	}
	return nil, nil, errorf(Internal, "The route %s %s is in more than one package: pass the package", method, path)
}

// Handle serves a route of the contracts. Register before Weft.Start.
func (h *HTTP) Handle(method, path string, handler HTTPHandler) error {
	return h.HandleIn("", method, path, handler)
}

// HandleIn is Handle for a route of one package, when two packages declare the same route.
func (h *HTTP) HandleIn(pkg, method, path string, handler HTTPHandler) error {
	if h.started {
		return errorf(Internal, "Register the handler for %s %s before Start()", method, path)
	}
	ir, route, err := h.weft.Contracts.FindRoute(method, path, pkg)
	if err != nil {
		return err
	}
	for _, r := range h.routes {
		if r.route == route {
			return errorf(Internal, "%s %s already has a handler", method, FullPath(route))
		}
	}
	h.routes = append(h.routes, registered{pkg: ir.Package, route: route, handler: handler})
	return nil
}

func (h *HTTP) Get(path string, handler HTTPHandler) error   { return h.Handle("GET", path, handler) }
func (h *HTTP) Post(path string, handler HTTPHandler) error  { return h.Handle("POST", path, handler) }
func (h *HTTP) Put(path string, handler HTTPHandler) error   { return h.Handle("PUT", path, handler) }
func (h *HTTP) Patch(path string, handler HTTPHandler) error { return h.Handle("PATCH", path, handler) }
func (h *HTTP) Delete(path string, handler HTTPHandler) error {
	return h.Handle("DELETE", path, handler)
}

// ---- Transport ----

func (h *HTTP) Name() string { return "http" }

func (h *HTTP) Start(ctx context.Context) error {
	if h.opts.Adapter == nil {
		if len(h.routes) > 0 {
			return NewError(Internal, "Routes are registered, but there is no HTTP adapter to serve them")
		}
		h.started = true
		return nil
	}
	for i := range h.routes {
		entry := &h.routes[i]
		h.opts.Adapter.Route(entry.route.Method, FullPath(entry.route), func(req HTTPRequest) HTTPResponse { return h.handle(entry, req) })
	}
	if err := h.opts.Adapter.Start(ctx); err != nil {
		return err
	}
	h.started = true
	return nil
}

func (h *HTTP) StopIntake(ctx context.Context) error {
	if h.opts.Adapter == nil {
		return nil
	}
	return h.opts.Adapter.StopIntake(ctx)
}

func (h *HTTP) Close(ctx context.Context) error {
	if h.opts.Adapter == nil {
		return nil
	}
	return h.opts.Adapter.Close(ctx)
}

// ---- serving ----

func (h *HTTP) handle(entry *registered, req HTTPRequest) HTTPResponse {
	route := entry.route
	template := FullPath(route)
	ctx := req.Context
	if ctx == nil {
		ctx = context.Background()
	}
	if ms, ok := parseInt(req.Headers["weft-deadline"]); ok && ms > 0 {
		var cancel context.CancelFunc
		ctx, cancel = context.WithDeadline(ctx, time.UnixMilli(ms))
		defer cancel()
	}
	hc := &HTTPContext{Call: NewCall("http", route.Name), Route: route, Method: route.Method, Template: template}
	hc.Response.Status = 200
	hc.Response.Headers = map[string]string{}
	for k, v := range req.Headers {
		hc.Headers[k] = v
	}

	started := nowMs()
	parent := ExtractTraceContext(ctx, req.Headers)
	spanCtx, span := tracer().Start(parent, route.Method+" "+template, trace.WithSpanKind(trace.SpanKindServer),
		trace.WithAttributes(attribute.String("http.request.method", route.Method), attribute.String("http.route", template), attribute.String("weft.operation", route.Name)))
	defer span.End()
	out := h.respond(spanCtx, entry, hc, req)
	span.SetAttributes(attribute.Int("http.response.status_code", out.Status))
	if out.Status >= 500 {
		span.SetStatus(codes.Error, fmt.Sprintf("HTTP %d", out.Status))
	}
	httpMetrics().serverDuration.Record(spanCtx, nowMs()-started, metric.WithAttributes(
		attribute.String("method", route.Method), attribute.String("route", template), attribute.Int("status", out.Status)))
	return out
}

func (h *HTTP) respond(ctx context.Context, entry *registered, hc *HTTPContext, req HTTPRequest) HTTPResponse {
	route := entry.route
	chain := make([]Middleware, 0, len(h.middleware))
	for _, mw := range h.middleware {
		mw := mw
		chain = append(chain, func(ctx context.Context, _ *Call, next Next) error { return mw(ctx, hc, next) })
	}
	var returned any
	err := h.weft.Dispatch(ctx, hc.Call, func(ctx context.Context, _ *Call) error {
		in, err := h.decodeInput(route, req)
		if err != nil {
			return err
		}
		in.Ctx = hc
		returned, err = entry.handler(ctx, in)
		return err
	}, chain...)
	if err == nil && returned == nil {
		if route.Result.Optional {
			err = NewError(NotFound, "Not found")
		} else {
			err = errorf(Internal, "The handler of %s returned nothing", route.Name)
		}
	}
	if err == nil {
		wire, encErr := h.weft.Contracts.Encode(route.Result.Value, returned)
		if encErr != nil {
			// The caller did nothing wrong: the handler returned something the contract does not allow.
			err = &Error{Category: Internal, Message: fmt.Sprintf("The handler of %s returned a value that does not match the contract", route.Name), Cause: encErr}
		} else if body, jsonErr := json.Marshal(wire); jsonErr != nil {
			err = &Error{Category: Internal, Message: "The result cannot be written as JSON", Cause: jsonErr}
		} else {
			headers := map[string]string{"content-type": "application/json"}
			for k, v := range hc.Response.Headers {
				headers[k] = v
			}
			return HTTPResponse{Status: hc.Response.Status, Headers: headers, Body: string(body)}
		}
	}
	status, problem, unexpected := ToProblem(err, route, h.weft.Contracts)
	if unexpected != nil {
		h.logger.Error(route.Method+" "+FullPath(route)+" failed", "error", unexpected.Error())
	}
	body, _ := json.Marshal(problem)
	headers := map[string]string{"content-type": "application/problem+json"}
	for k, v := range hc.Response.Headers {
		headers[k] = v
	}
	return HTTPResponse{Status: status, Headers: headers, Body: string(body)}
}

func parseInt(s string) (int64, bool) {
	var n int64
	if s == "" {
		return 0, false
	}
	if _, err := fmt.Sscan(s, &n); err != nil {
		return 0, false
	}
	return n, true
}

func (h *HTTP) decodeInput(route *RouteDef, req HTTPRequest) (*HTTPInput, error) {
	c := h.weft.Contracts
	var issues []Issue
	prefixed := func(prefix string, found []Issue) {
		for _, i := range found {
			path := prefix
			if i.Path != "" {
				path = prefix + "." + i.Path
			}
			issues = append(issues, Issue{Path: path, Message: i.Message})
		}
	}
	in := &HTTPInput{Params: map[string]any{}, Query: map[string]any{}, Headers: map[string]any{}}

	for _, p := range route.PathParams {
		raw, present := req.Params[p.Name]
		var wire any
		if present {
			wire = coerceText(p.Type, raw)
		}
		value, err := c.Decode(p.Type, wire)
		if err != nil {
			prefixed("params."+p.Name, AsError(err).Issues)
			continue
		}
		in.Params[p.Name] = value
	}
	if route.Query != nil {
		q, found := c.decodeTextRecord(*route.Query, func(name string) (any, bool) {
			v, ok := req.Query[name]
			if !ok {
				return nil, false
			}
			if len(v) == 1 {
				return v[0], true
			}
			return v, true
		}, "query")
		in.Query = q
		issues = append(issues, found...)
	}
	if route.Headers != nil {
		hd, found := c.decodeTextRecord(*route.Headers, func(name string) (any, bool) {
			v, ok := req.Headers[strings.ToLower(name)]
			return v, ok
		}, "headers")
		in.Headers = hd
		issues = append(issues, found...)
	}
	if route.Body != nil {
		if req.Body == nil || strings.TrimSpace(*req.Body) == "" {
			issues = append(issues, Issue{Path: "body", Message: "is required"})
		} else if wire, err := ParseJSON([]byte(*req.Body)); err != nil {
			issues = append(issues, Issue{Path: "body", Message: "is not valid JSON"})
		} else if value, err := c.Decode(*route.Body, wire); err != nil {
			prefixed("body", AsError(err).Issues)
		} else {
			in.Body = value
		}
	}
	if len(issues) > 0 {
		return nil, invalidInput(issues)
	}
	return in, nil
}

// ---- calling ----

// CallOptions are the options of a call to another service.
type CallOptions struct {
	Body any
	// Params are values for the {name} parts of the path.
	Params  map[string]any
	Query   map[string]any
	Headers map[string]any
	// Timeout overrides the 10 s call timeout.
	Timeout time.Duration
	// IdempotencyKey is sent as Idempotency-Key; it makes a POST safe to retry.
	IdempotencyKey string
	// Package is for the case where two packages declare the same route.
	Package string
}

// ServiceClient calls one other service: h.To("orders").Call(ctx, "POST", "/orders", opts).
type ServiceClient struct {
	h       *HTTP
	service string
}

// To calls another service, whose base URL is under services: in weft.yaml.
func (h *HTTP) To(service string) *ServiceClient { return &ServiceClient{h: h, service: service} }

// Call calls a route of the service.
func (s *ServiceClient) Call(ctx context.Context, method, path string, o CallOptions) (any, error) {
	return s.h.call(ctx, s.service, method, path, o)
}
func (s *ServiceClient) Get(ctx context.Context, path string, o CallOptions) (any, error) {
	return s.Call(ctx, "GET", path, o)
}
func (s *ServiceClient) Post(ctx context.Context, path string, o CallOptions) (any, error) {
	return s.Call(ctx, "POST", path, o)
}
func (s *ServiceClient) Put(ctx context.Context, path string, o CallOptions) (any, error) {
	return s.Call(ctx, "PUT", path, o)
}
func (s *ServiceClient) Delete(ctx context.Context, path string, o CallOptions) (any, error) {
	return s.Call(ctx, "DELETE", path, o)
}

var idempotent = map[string]bool{"GET": true, "HEAD": true, "PUT": true, "DELETE": true, "OPTIONS": true}

type httpReply struct {
	status int
	body   []byte
}

func (h *HTTP) call(ctx context.Context, service, method, path string, o CallOptions) (any, error) {
	c := h.weft.Contracts
	base, ok := h.weft.Config.ServiceURL(service)
	if !ok {
		return nil, errorf(Internal, "There is no URL for the service %q: add it under services: in weft.yaml", service)
	}
	_, route, err := c.FindRoute(method, path, o.Package)
	if err != nil {
		return nil, err
	}
	template := FullPath(route)

	// Build the request from the contract. Mistakes here are the caller's, so they are InvalidInput.
	target := template
	for _, p := range route.PathParams {
		v, ok := o.Params[p.Name]
		if !ok {
			return nil, errorf(InvalidInput, "Invalid input: the path parameter %s is required", p.Name)
		}
		wire, err := c.Encode(p.Type, v)
		if err != nil {
			return nil, err
		}
		target = strings.Replace(target, "{"+p.Name+"}", url.PathEscape(fmt.Sprint(wire)), 1)
	}
	query := url.Values{}
	if route.Query != nil {
		values, err := c.encodeTextRecord(*route.Query, orEmpty(o.Query))
		if err != nil {
			return nil, err
		}
		for k, vs := range values {
			for _, v := range vs {
				query.Add(k, v)
			}
		}
	}
	headers := map[string]string{}
	if route.Headers != nil {
		values, err := c.encodeTextRecord(*route.Headers, orEmpty(o.Headers))
		if err != nil {
			return nil, err
		}
		for k, vs := range values {
			headers[k] = strings.Join(vs, ",")
		}
	} else {
		for k, v := range o.Headers {
			headers[k] = fmt.Sprint(v)
		}
	}
	var body []byte
	if route.Body != nil {
		wire, err := c.Encode(*route.Body, o.Body)
		if err != nil {
			return nil, err
		}
		if body, err = json.Marshal(wire); err != nil {
			return nil, &Error{Category: Internal, Message: "the body cannot be written as JSON", Cause: err}
		}
		headers["content-type"] = "application/json"
	}
	if o.IdempotencyKey != "" {
		headers["idempotency-key"] = o.IdempotencyKey
	}
	headers["accept"] = "application/json, application/problem+json"
	full := strings.TrimRight(base, "/") + target
	if len(query) > 0 {
		full += "?" + query.Encode()
	}

	timeout := o.Timeout
	if timeout == 0 {
		timeout = CallTimeout
	}
	safe := idempotent[method] || o.IdempotencyKey != ""
	started := nowMs()
	status := 0
	defer func() {
		httpMetrics().clientDuration.Record(ctx, nowMs()-started, metric.WithAttributes(
			attribute.String("method", method), attribute.String("route", template), attribute.String("service", service), attribute.Int("status", status)))
	}()

	for attempt := 1; ; attempt++ {
		var value any
		result, err := h.attempt(ctx, method, template, service, route.Name, full, headers, body, timeout, &status)
		if err == nil {
			// An answer that carries an error is retried like a call that failed on the way.
			if value, err = h.interpret(route, result); err == nil {
				return value, nil
			}
		}
		we := AsError(err)
		retryable := we.Category == Unavailable || we.Category == Timeout
		if !retryable || !safe || attempt >= CallAttempts {
			return nil, err
		}
		h.opts.Sleep(h.weft.ShutdownContext(), BackoffDelay(attempt, Backoff{}, h.opts.Random))
	}
}

func orEmpty(m map[string]any) map[string]any {
	if m == nil {
		return map[string]any{}
	}
	return m
}

// attempt sends the request once, inside a client span, with a timeout.
func (h *HTTP) attempt(ctx context.Context, method, template, service, operation, target string, headers map[string]string, body []byte, timeout time.Duration, status *int) (httpReply, error) {
	ctx, span := tracer().Start(ctx, method+" "+template, trace.WithSpanKind(trace.SpanKindClient),
		trace.WithAttributes(attribute.String("http.request.method", method), attribute.String("http.route", template),
			attribute.String("server.address", service), attribute.String("weft.operation", operation)))
	defer span.End()
	sent := map[string]string{}
	for k, v := range headers {
		sent[k] = v
	}
	InjectTraceContext(ctx, sent)
	sent["weft-deadline"] = fmt.Sprint(time.Now().Add(timeout).UnixMilli())

	reply, err := WithTimeout(ctx, timeout, func(ctx context.Context) (httpReply, error) {
		var reader io.Reader
		if body != nil {
			reader = strings.NewReader(string(body))
		}
		req, err := http.NewRequestWithContext(ctx, method, target, reader)
		if err != nil {
			return httpReply{}, &Error{Category: Internal, Message: "the request cannot be built: " + err.Error(), Cause: err}
		}
		for k, v := range sent {
			req.Header.Set(k, v)
		}
		res, err := h.opts.Client.Do(req)
		if err != nil {
			return httpReply{}, &Error{Category: Unavailable, Message: fmt.Sprintf("Could not reach %s: %v", service, err), Cause: err}
		}
		defer res.Body.Close()
		text, err := io.ReadAll(res.Body)
		if err != nil {
			return httpReply{}, &Error{Category: Unavailable, Message: fmt.Sprintf("Could not read the answer of %s: %v", service, err), Cause: err}
		}
		return httpReply{res.StatusCode, text}, nil
	})
	if err != nil {
		RecordError(span, err)
		return httpReply{}, err
	}
	*status = reply.status
	span.SetAttributes(attribute.Int("http.response.status_code", reply.status))
	return reply, nil
}

// interpret turns an answer into the route's result, or into the error it carries.
func (h *HTTP) interpret(route *RouteDef, r httpReply) (any, error) {
	c := h.weft.Contracts
	if r.status >= 200 && r.status < 300 {
		if strings.TrimSpace(string(r.body)) == "" {
			if route.Result.Optional {
				return nil, nil
			}
			return nil, errorf(Internal, "The service answered %s with no body", route.Name)
		}
		return c.DecodeJSON(route.Result.Value, r.body)
	}
	var parsed map[string]any
	if wire, err := ParseJSON(r.body); err == nil {
		parsed, _ = wire.(map[string]any)
	}
	err := FromProblem(r.status, parsed, c)
	if we, ok := err.(*Error); ok && we.Category == NotFound && route.Result.Optional {
		return nil, nil
	}
	return nil, err
}
