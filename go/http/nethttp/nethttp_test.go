package nethttp_test

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"net"
	"net/http"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/propagation"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
	"go.opentelemetry.io/otel/sdk/trace/tracetest"
	"go.opentelemetry.io/otel/trace"

	weft "github.com/weft-sdk/weft/go"
	"github.com/weft-sdk/weft/go/http/nethttp"
)

const id = "123e4567-e89b-42d3-a456-426614174000"

func prim(n string) weft.TypeRef { return weft.Primitive(n) }
func f(name string, t weft.TypeRef) weft.FieldDef {
	return weft.FieldDef{Name: name, WireName: name, Type: t}
}

func orderRef() weft.TypeRef { return weft.Named("t", "Order") }

var contracts = weft.NewContracts(weft.Ir{
	IrVersion: "0.1", Package: "t",
	Types: []weft.TypeDef{
		{Name: "CreateOrder", Kind: "record", Fields: []weft.FieldDef{f("customerId", prim("string")), f("qty", prim("int32"))}},
		{Name: "Order", Kind: "record", Fields: []weft.FieldDef{f("id", prim("uuid")), f("customerId", prim("string"))}},
		{Name: "ListQuery", Kind: "record", Inline: true, Fields: []weft.FieldDef{{Name: "page", WireName: "page", Type: prim("int"), Optional: true}}},
	},
	Errors: []weft.ErrorDef{{Name: "OutOfStock", Status: 409, Fields: []weft.FieldDef{f("sku", prim("string")), f("available", prim("int"))}}},
	Routes: []weft.RouteDef{
		{Name: "createOrder", Method: "POST", Base: "/v1", Path: "/orders", Body: ptr(weft.Named("t", "CreateOrder")),
			Result: weft.ResultDef{Value: orderRef(), Errors: []weft.ErrorRef{{Package: "t", Name: "OutOfStock"}, {Package: "weft", Name: "InvalidInput"}}}},
		{Name: "getOrder", Method: "GET", Base: "/v1", Path: "/orders/{id}", PathParams: []weft.ParamDef{{Name: "id", Type: prim("uuid")}},
			Result: weft.ResultDef{Value: orderRef(), Optional: true}},
		{Name: "listOrders", Method: "GET", Base: "/v1", Path: "/orders", Query: ptr(weft.Named("t", "ListQuery")),
			Result: weft.ResultDef{Value: weft.List(orderRef())}},
	},
})

func ptr[T any](v T) *T { return &v }

var quiet = slog.New(slog.NewTextHandler(io.Discard, nil))

func freePort(t *testing.T) int {
	t.Helper()
	l, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer l.Close()
	return l.Addr().(*net.TCPAddr).Port
}

type rig struct {
	weft *weft.Weft
	http *weft.HTTP
	url  string
}

func setup(t *testing.T, client *http.Client) *rig {
	t.Helper()
	port := freePort(t)
	cfg, err := weft.ParseConfig(fmt.Sprintf("service: svc\ntransports:\n  http:\n    port: %d\nservices:\n  self: http://127.0.0.1:%d\n  nobody: http://127.0.0.1:1", port, port), nil)
	if err != nil {
		t.Fatal(err)
	}
	w := weft.New(weft.Options{Contracts: contracts, Config: cfg, ShutdownGrace: 500 * time.Millisecond})
	h, err := weft.NewHTTP(w, weft.HTTPOptions{
		Adapter: nethttp.New(nethttp.Options{}), Client: client, Logger: quiet,
		Sleep: func(context.Context, time.Duration) {}, Random: func() float64 { return 0.5 },
	})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { w.Stop(context.Background()) })
	return &rig{weft: w, http: h, url: fmt.Sprintf("http://127.0.0.1:%d", port)}
}

func (r *rig) start(t *testing.T) {
	t.Helper()
	if err := r.weft.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
}

func must(t *testing.T, err error) {
	t.Helper()
	if err != nil {
		t.Fatal(err)
	}
}

func order(customer string) map[string]any { return map[string]any{"id": id, "customerId": customer} }

func raw(t *testing.T, method, url, body string) (int, string, string) {
	t.Helper()
	var reader io.Reader
	if body != "" {
		reader = strings.NewReader(body)
	}
	req, _ := http.NewRequest(method, url, reader)
	res, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	defer res.Body.Close()
	text, _ := io.ReadAll(res.Body)
	return res.StatusCode, res.Header.Get("Content-Type"), string(text)
}

func TestServeAndCallFromTheSameContract(t *testing.T) {
	r := setup(t, nil)
	must(t, r.http.Post("/orders", func(_ context.Context, in *weft.HTTPInput) (any, error) {
		return order(in.Body.(map[string]any)["customerId"].(string)), nil
	}))
	r.start(t)
	got, err := r.http.To("self").Post(context.Background(), "/orders", weft.CallOptions{Body: map[string]any{"customerId": "zed", "qty": int64(2)}})
	if err != nil {
		t.Fatal(err)
	}
	if got.(map[string]any)["customerId"] != "zed" {
		t.Errorf("got %v", got)
	}
}

func TestTheHandlerGetsDecodedParamsAndQuery(t *testing.T) {
	r := setup(t, nil)
	var seen []any
	var mu sync.Mutex
	must(t, r.http.Get("/orders/{id}", func(_ context.Context, in *weft.HTTPInput) (any, error) {
		mu.Lock()
		seen = append(seen, in.Params["id"])
		mu.Unlock()
		return order("c"), nil
	}))
	must(t, r.http.Get("/v1/orders", func(_ context.Context, in *weft.HTTPInput) (any, error) {
		mu.Lock()
		seen = append(seen, in.Query["page"])
		mu.Unlock()
		return []any{}, nil
	}))
	r.start(t)
	_, err := r.http.To("self").Get(context.Background(), "/orders/{id}", weft.CallOptions{Params: map[string]any{"id": id}})
	must(t, err)
	_, err = r.http.To("self").Get(context.Background(), "/orders", weft.CallOptions{Query: map[string]any{"page": int64(2)}})
	must(t, err)
	if len(seen) != 2 || seen[0] != id || seen[1] != int64(2) {
		t.Errorf("seen = %v", seen)
	}
}

func TestARequestThatBreaksTheContractGetsAProblemListingEveryIssue(t *testing.T) {
	r := setup(t, nil)
	must(t, r.http.Post("/orders", func(context.Context, *weft.HTTPInput) (any, error) { return order("x"), nil }))
	must(t, r.http.Get("/orders/{id}", func(context.Context, *weft.HTTPInput) (any, error) { return order("x"), nil }))
	must(t, r.http.Get("/orders", func(context.Context, *weft.HTTPInput) (any, error) { return []any{}, nil }))
	r.start(t)
	status, ctype, body := raw(t, "POST", r.url+"/v1/orders", `{"customerId": 5}`)
	if status != 400 || !strings.HasPrefix(ctype, "application/problem+json") ||
		!strings.Contains(body, `"urn:weft:error:InvalidInput"`) || !strings.Contains(body, `"body.customerId"`) || !strings.Contains(body, `"body.qty"`) {
		t.Errorf("%d %s %s", status, ctype, body)
	}
	if _, _, b := raw(t, "POST", r.url+"/v1/orders", ""); !strings.Contains(b, `"is required"`) {
		t.Errorf("a missing body: %s", b)
	}
	if _, _, b := raw(t, "POST", r.url+"/v1/orders", "{nope"); !strings.Contains(b, "is not valid JSON") {
		t.Errorf("bad JSON: %s", b)
	}
	if _, _, b := raw(t, "GET", r.url+"/v1/orders/nope", ""); !strings.Contains(b, `"params.id"`) {
		t.Errorf("bad path parameter: %s", b)
	}
	if _, _, b := raw(t, "GET", r.url+"/v1/orders?page=abc", ""); !strings.Contains(b, `"query.page"`) {
		t.Errorf("bad query: %s", b)
	}
	if s, _, _ := raw(t, "GET", r.url+"/v1/orders?page=3", ""); s != 200 {
		t.Errorf("a good query: %d", s)
	}
}

func TestUnknownPathsAndBigBodies(t *testing.T) {
	r := setup(t, nil)
	must(t, r.http.Post("/orders", func(context.Context, *weft.HTTPInput) (any, error) { return order("x"), nil }))
	r.start(t)
	if s, _, b := raw(t, "GET", r.url+"/nothing", ""); s != 404 || !strings.Contains(b, `"NotFound"`) {
		t.Errorf("%d %s", s, b)
	}
	if s, ct, _ := raw(t, "POST", r.url+"/v1/orders", strings.Repeat("x", 2<<20)); s != 413 || !strings.Contains(ct, "problem+json") {
		t.Errorf("%d %s", s, ct)
	}
}

func TestErrorsCrossTheWireWithTheirCategoryAndFields(t *testing.T) {
	r := setup(t, nil)
	var mu sync.Mutex
	var current error
	set := func(err error) { mu.Lock(); current = err; mu.Unlock() }
	must(t, r.http.Post("/orders", func(context.Context, *weft.HTTPInput) (any, error) {
		mu.Lock()
		defer mu.Unlock()
		return nil, current
	}))
	r.start(t)
	call := func() error {
		_, err := r.http.To("self").Post(context.Background(), "/orders", weft.CallOptions{Body: map[string]any{"customerId": "c", "qty": int64(1)}, Timeout: 2 * time.Second})
		return err
	}
	table := []struct {
		category weft.Category
		status   int
	}{
		{weft.NotFound, 404}, {weft.Unauthorized, 401}, {weft.Forbidden, 403}, {weft.Conflict, 409},
		{weft.Unavailable, 503}, {weft.Timeout, 504}, {weft.Internal, 500}, {weft.InvalidInput, 400},
	}
	for _, c := range table {
		set(weft.NewError(c.category, "because"))
		s, _, body := raw(t, "POST", r.url+"/v1/orders", `{"customerId":"c","qty":1}`)
		if s != c.status || !strings.Contains(body, `"urn:weft:error:`+string(c.category)+`"`) {
			t.Errorf("%s: %d %s", c.category, s, body)
		}
		if got := weft.AsError(call()); got.Category != c.category {
			t.Errorf("%s came back as %s", c.category, got.Category)
		}
	}

	// A declared error keeps its status and fields, and comes back as the same type.
	set(&weft.DeclaredError{Package: "t", Name: "OutOfStock", Fields: map[string]any{"sku": "apple", "available": int64(0)}, Message: "No apples"})
	s, _, body := raw(t, "POST", r.url+"/v1/orders", `{"customerId":"c","qty":1}`)
	if s != 409 || !strings.Contains(body, `"urn:weft:error:t.OutOfStock"`) || !strings.Contains(body, `"sku":"apple"`) || !strings.Contains(body, `"No apples"`) {
		t.Errorf("%d %s", s, body)
	}
	de, ok := call().(*weft.DeclaredError)
	if !ok || de.Name != "OutOfStock" || de.Fields["sku"] != "apple" || de.Fields["available"] != int64(0) {
		t.Errorf("declared error = %#v", de)
	}

	// What the route does not list, and what is not an error of ours, is a server bug and is not described.
	set(&weft.DeclaredError{Package: "t", Name: "Other"})
	if s, _, _ := raw(t, "POST", r.url+"/v1/orders", `{"customerId":"c","qty":1}`); s != 500 {
		t.Errorf("an undeclared error: %d", s)
	}
	set(fmt.Errorf("database password is hunter2"))
	s, _, body = raw(t, "POST", r.url+"/v1/orders", `{"customerId":"c","qty":1}`)
	if s != 500 || strings.Contains(body, "hunter2") || !strings.Contains(body, "Internal server error") {
		t.Errorf("%d %s", s, body)
	}
}

func TestAnOptionalResultThatIsMissingIs404AndTheClientSeesNil(t *testing.T) {
	r := setup(t, nil)
	must(t, r.http.Get("/orders/{id}", func(context.Context, *weft.HTTPInput) (any, error) { return nil, nil }))
	r.start(t)
	if s, _, _ := raw(t, "GET", r.url+"/v1/orders/"+id, ""); s != 404 {
		t.Errorf("status %d", s)
	}
	got, err := r.http.To("self").Get(context.Background(), "/orders/{id}", weft.CallOptions{Params: map[string]any{"id": id}})
	if err != nil || got != nil {
		t.Errorf("%v %v", got, err)
	}
}

func TestAWrongResultIsAServerBugAndNotSentAsData(t *testing.T) {
	r := setup(t, nil)
	must(t, r.http.Get("/orders", func(context.Context, *weft.HTTPInput) (any, error) { return []any{map[string]any{"nope": true}}, nil }))
	r.start(t)
	if s, _, _ := raw(t, "GET", r.url+"/v1/orders", ""); s != 500 {
		t.Errorf("status %d", s)
	}
}

func TestHandlersCanSetTheStatusAndHeadersAndMiddlewareRunsAround(t *testing.T) {
	r := setup(t, nil)
	var mu sync.Mutex
	var log []string
	add := func(s string) { mu.Lock(); log = append(log, s); mu.Unlock() }
	r.weft.Use(func(ctx context.Context, c *weft.Call, next weft.Next) error { add("weft"); return next(ctx) })
	r.http.Use(func(ctx context.Context, hc *weft.HTTPContext, next weft.Next) error {
		add("http " + hc.Call.Operation)
		err := next(ctx)
		hc.Response.Headers["x-seen"] = "yes"
		return err
	})
	must(t, r.http.Post("/orders", func(_ context.Context, in *weft.HTTPInput) (any, error) {
		in.Ctx.Response.Status = 201
		in.Ctx.Response.Headers["location"] = "/v1/orders/1"
		return order("x"), nil
	}))
	r.start(t)
	req, _ := http.NewRequest("POST", r.url+"/v1/orders", strings.NewReader(`{"customerId":"c","qty":1}`))
	res, err := http.DefaultClient.Do(req)
	must(t, err)
	res.Body.Close()
	if res.StatusCode != 201 || res.Header.Get("Location") != "/v1/orders/1" || res.Header.Get("X-Seen") != "yes" {
		t.Errorf("%d %v", res.StatusCode, res.Header)
	}
	if strings.Join(log, ",") != "weft,http createOrder" {
		t.Errorf("log = %v", log)
	}
}

func TestRegistrationIsChecked(t *testing.T) {
	r := setup(t, nil)
	if err := r.http.Get("/nope", nil); err == nil || !strings.Contains(err.Error(), "no route") {
		t.Errorf("err = %v", err)
	}
	must(t, r.http.Get("/orders", func(context.Context, *weft.HTTPInput) (any, error) { return []any{}, nil }))
	if err := r.http.Get("/orders", nil); err == nil || !strings.Contains(err.Error(), "already has a handler") {
		t.Errorf("err = %v", err)
	}
	if _, ok := r.http.Raw().(*http.ServeMux); !ok {
		t.Errorf("raw = %T", r.http.Raw())
	}
}

func TestCallingOtherServices(t *testing.T) {
	r := setup(t, nil)
	r.start(t)
	ctx := context.Background()
	if _, err := r.http.To("ghost").Get(ctx, "/orders", weft.CallOptions{}); err == nil || !strings.Contains(err.Error(), `no URL for the service "ghost"`) {
		t.Errorf("err = %v", err)
	}
	if _, err := r.http.To("nobody").Get(ctx, "/orders", weft.CallOptions{Timeout: time.Second}); weft.AsError(err).Category != weft.Unavailable {
		t.Errorf("unreachable: %v", err)
	}
	if _, err := r.http.To("self").Post(ctx, "/orders", weft.CallOptions{Body: map[string]any{"customerId": 1}}); weft.AsError(err).Category != weft.InvalidInput {
		t.Errorf("a call that breaks the contract is refused before it is sent: %v", err)
	}
	if _, err := r.http.To("self").Get(ctx, "/orders/{id}", weft.CallOptions{}); weft.AsError(err).Category != weft.InvalidInput {
		t.Errorf("a missing path parameter: %v", err)
	}
}

// counting counts the requests a client makes.
type counting struct {
	n atomic.Int32
}

func (c *counting) RoundTrip(req *http.Request) (*http.Response, error) {
	c.n.Add(1)
	return http.DefaultTransport.RoundTrip(req)
}

func TestRetriesOnlyWhatIsSafeToRepeat(t *testing.T) {
	c := &counting{}
	r := setup(t, &http.Client{Transport: c})
	var gets, posts atomic.Int32
	must(t, r.http.Get("/orders", func(context.Context, *weft.HTTPInput) (any, error) {
		if gets.Add(1) < 3 {
			return nil, weft.NewError(weft.Unavailable, "warming up")
		}
		return []any{}, nil
	}))
	must(t, r.http.Post("/orders", func(context.Context, *weft.HTTPInput) (any, error) {
		if posts.Add(1) < 3 {
			return nil, weft.NewError(weft.Unavailable, "warming up")
		}
		return order("x"), nil
	}))
	r.start(t)
	ctx := context.Background()
	if _, err := r.http.To("self").Get(ctx, "/orders", weft.CallOptions{}); err != nil || c.n.Load() != 3 {
		t.Errorf("a GET is retried: %v after %d requests", err, c.n.Load())
	}
	c.n.Store(0)
	body := map[string]any{"customerId": "c", "qty": int64(1)}
	if _, err := r.http.To("self").Post(ctx, "/orders", weft.CallOptions{Body: body}); weft.AsError(err).Category != weft.Unavailable || c.n.Load() != 1 {
		t.Errorf("a POST is not retried: %v after %d requests", err, c.n.Load())
	}
	c.n.Store(0)
	if _, err := r.http.To("self").Post(ctx, "/orders", weft.CallOptions{Body: body, IdempotencyKey: "k"}); err != nil || c.n.Load() != 2 {
		t.Errorf("with a key a POST is retried: %v after %d requests", err, c.n.Load())
	}
}

func TestACallThatTakesTooLongTimesOutAndIsRetriedWhenSafe(t *testing.T) {
	r := setup(t, nil)
	var n atomic.Int32
	must(t, r.http.Get("/orders", func(context.Context, *weft.HTTPInput) (any, error) {
		n.Add(1)
		time.Sleep(300 * time.Millisecond) // ignores the caller's deadline on purpose
		return []any{}, nil
	}))
	r.start(t)
	_, err := r.http.To("self").Get(context.Background(), "/orders", weft.CallOptions{Timeout: 30 * time.Millisecond})
	if err == nil || weft.AsError(err).Category != weft.Timeout || n.Load() != 3 {
		t.Errorf("%v after %d attempts", err, n.Load())
	}
}

func TestOneTraceSpansBothServicesAndTheDeadlineTravels(t *testing.T) {
	recorder := tracetest.NewSpanRecorder()
	old, oldProp := otel.GetTracerProvider(), otel.GetTextMapPropagator()
	otel.SetTracerProvider(sdktrace.NewTracerProvider(sdktrace.WithSpanProcessor(recorder)))
	otel.SetTextMapPropagator(propagation.TraceContext{})
	t.Cleanup(func() { otel.SetTracerProvider(old); otel.SetTextMapPropagator(oldProp) })

	r := setup(t, nil)
	var seen atomic.Value
	must(t, r.http.Get("/orders", func(ctx context.Context, in *weft.HTTPInput) (any, error) {
		seen.Store(in.Ctx.Headers)
		if d, ok := ctx.Deadline(); !ok || time.Until(d) <= 0 {
			t.Error("the handler's context should carry the caller's deadline")
		}
		return []any{}, nil
	}))
	r.start(t)
	ctx, root := otel.Tracer("test").Start(context.Background(), "checkout")
	_, err := r.http.To("self").Get(ctx, "/orders", weft.CallOptions{})
	root.End()
	must(t, err)
	headers, _ := seen.Load().(map[string]string)
	if !strings.Contains(headers["traceparent"], root.SpanContext().TraceID().String()) || headers["weft-deadline"] == "" {
		t.Errorf("headers = %v", headers)
	}

	var client, server sdktrace.ReadOnlySpan
	for _, s := range recorder.Ended() {
		if s.SpanContext().TraceID() != root.SpanContext().TraceID() {
			t.Errorf("%s is in another trace", s.Name())
		}
		switch s.SpanKind() {
		case trace.SpanKindClient:
			client = s
		case trace.SpanKindServer:
			server = s
		}
	}
	if client == nil || server == nil || server.Parent().SpanID() != client.SpanContext().SpanID() {
		t.Fatalf("the server span should be a child of the client's: %v / %v", client, server)
	}
	if server.Name() != "GET /v1/orders" {
		t.Errorf("name = %s", server.Name())
	}
}

func TestShutdownFinishesARequestInFlightThenStopsListening(t *testing.T) {
	r := setup(t, nil)
	release := make(chan struct{})
	entered := make(chan struct{})
	must(t, r.http.Get("/orders", func(context.Context, *weft.HTTPInput) (any, error) {
		close(entered)
		<-release
		return []any{}, nil
	}))
	r.start(t)
	status := make(chan int, 1)
	go func() { s, _, _ := raw(t, "GET", r.url+"/v1/orders", ""); status <- s }()
	<-entered
	stopped := make(chan weft.StopResult, 1)
	go func() { stopped <- r.weft.Stop(context.Background()) }()
	time.Sleep(50 * time.Millisecond)
	close(release)
	if s := <-status; s != 200 {
		t.Errorf("the in-flight request got %d", s)
	}
	if res := <-stopped; res != (weft.StopResult{Drained: true}) {
		t.Errorf("result = %+v", res)
	}
	if _, err := http.Get(r.url + "/v1/orders"); err == nil {
		t.Error("it should not be listening any more")
	}
}
