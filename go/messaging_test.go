package weft_test

import (
	"context"
	"errors"
	"io"
	"log/slog"
	"slices"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	weft "github.com/weft-sdk/weft/go"
)

var placed = map[string]any{
	"id": id, "customerId": "c1", "total": map[string]any{"amount": int64(450), "currency": "EUR"},
	"placedAt": time.Date(2026, 11, 23, 10, 0, 0, 0, time.UTC), "blob": []byte{1, 2, 3}, "big": int64(1),
}

type rig struct {
	weft    *weft.Weft
	m       *weft.Messaging
	adapter *weft.MemoryAdapter
	broker  *weft.MemoryBroker
	mu      sync.Mutex
	sleeps  []time.Duration
}

func newRig(t *testing.T, yaml string, broker *weft.MemoryBroker, hold bool) *rig {
	t.Helper()
	if yaml == "" {
		yaml = "service: orders-svc"
	}
	cfg, err := weft.ParseConfig(yaml, nil)
	if err != nil {
		t.Fatal(err)
	}
	r := &rig{}
	r.weft = weft.New(weft.Options{Contracts: contracts, Config: cfg, ShutdownGrace: 300 * time.Millisecond})
	r.adapter = weft.NewMemoryAdapter("kafka", broker)
	r.broker = r.adapter.Broker()
	r.m, err = weft.NewMessaging(r.weft, weft.MessagingOptions{
		Adapter: r.adapter,
		Logger:  slog.New(slog.NewTextHandler(io.Discard, nil)),
		Random:  func() float64 { return 0.5 },
		Sleep: func(ctx context.Context, d time.Duration) {
			r.mu.Lock()
			r.sleeps = append(r.sleeps, d)
			r.mu.Unlock()
			if hold {
				<-ctx.Done()
			}
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	return r
}

func (r *rig) start(t *testing.T) {
	t.Helper()
	if err := r.weft.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { r.weft.Stop(context.Background()) })
}

func (r *rig) slept() []time.Duration {
	r.mu.Lock()
	defer r.mu.Unlock()
	return slices.Clone(r.sleeps)
}

func TestPublishAndReceive(t *testing.T) {
	r := newRig(t, "", nil, false)
	var got any
	var gotCtx *weft.MessageContext
	if err := r.m.On("order.placed", func(_ context.Context, p any, mc *weft.MessageContext) error {
		got, gotCtx = p, mc
		return nil
	}, weft.SubscribeOptions{}); err != nil {
		t.Fatal(err)
	}
	r.start(t)
	if err := r.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{Headers: map[string]string{"tenant": "t1"}, IdempotencyKey: "k1"}); err != nil {
		t.Fatal(err)
	}
	r.broker.Idle()
	rec := got.(map[string]any)
	if rec["customerId"] != "c1" || !rec["placedAt"].(time.Time).Equal(placed["placedAt"].(time.Time)) {
		t.Errorf("payload = %v", rec)
	}
	if gotCtx.Key != id || gotCtx.Attempt != 1 || gotCtx.Event != "order.placed" || gotCtx.Transport != "kafka" {
		t.Errorf("ctx = %+v", gotCtx)
	}
	e := gotCtx.Envelope
	if e.Type != "order.placed" || e.Source != "orders-svc" || e.DataContentType != "application/json" || len(e.ID) != 36 {
		t.Errorf("envelope = %+v", e)
	}
	if e.Headers["tenant"] != "t1" || e.Headers[weft.IdempotencyHeader] != "k1" {
		t.Errorf("headers = %v", e.Headers)
	}
	if !strings.Contains(string(e.Body), `"customer_id":"c1"`) || !strings.Contains(string(e.Body), `"placedAt":"2026-11-23T10:00:00.000Z"`) {
		t.Errorf("body is the wire form: %s", e.Body)
	}
}

func TestTheTopicNameIsTheDestination(t *testing.T) {
	r := newRig(t, "service: s\ntopics:\n  order.placed: prod.placed", nil, false)
	r.start(t)
	if err := r.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{Key: "custom"}); err != nil {
		t.Fatal(err)
	}
	if got := r.broker.To("prod.placed"); len(got) != 1 || got[0].Envelope.Key != "custom" {
		t.Errorf("published = %+v", r.broker.Published())
	}
}

func TestWhatIsRefused(t *testing.T) {
	r := newRig(t, "", nil, false)
	if err := r.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{}); weft.AsError(err).Category != weft.Unavailable {
		t.Errorf("before start: %v", err)
	}
	r.start(t)
	bad := map[string]any{"id": "nope"}
	if err := r.m.Publish(context.Background(), "order.placed", bad, weft.PublishOptions{}); weft.AsError(err).Category != weft.InvalidInput {
		t.Errorf("invalid payload: %v", err)
	}
	if len(r.broker.Published()) != 0 {
		t.Error("nothing should be sent for an invalid payload")
	}
	if err := r.m.Publish(context.Background(), "nope", placed, weft.PublishOptions{}); weft.AsError(err).Category != weft.Internal {
		t.Errorf("unknown event: %v", err)
	}
	if err := r.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{Delay: time.Second}); !strings.Contains(err.Error(), "delayed delivery") {
		t.Errorf("delay: %v", err)
	}
	if err := r.m.On("order.placed", nil, weft.SubscribeOptions{}); err == nil || !strings.Contains(err.Error(), "before Start") {
		t.Errorf("late handler: %v", err)
	}
}

func TestEachConsumerGroupGetsEveryMessageOnce(t *testing.T) {
	broker := weft.NewMemoryBroker()
	var mu sync.Mutex
	var got []string
	record := func(who string) weft.MessageHandler {
		return func(context.Context, any, *weft.MessageContext) error {
			mu.Lock()
			got = append(got, who)
			mu.Unlock()
			return nil
		}
	}
	a, b, c := newRig(t, "", broker, false), newRig(t, "", broker, false), newRig(t, "", broker, false)
	_ = a.m.On("order.placed", record("a"), weft.SubscribeOptions{Group: "payments"})
	_ = b.m.On("order.placed", record("b"), weft.SubscribeOptions{Group: "payments"})
	_ = c.m.On("order.placed", record("c"), weft.SubscribeOptions{Group: "emails"})
	a.start(t)
	b.start(t)
	c.start(t)
	for i := 0; i < 2; i++ {
		if err := a.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{}); err != nil {
			t.Fatal(err)
		}
	}
	broker.Idle()
	count := map[string]int{}
	for _, g := range got {
		count[g]++
	}
	if count["c"] != 2 || count["a"] != 1 || count["b"] != 1 {
		t.Errorf("count = %v", count)
	}
}

func TestRetriesDeadLettersAndPolicies(t *testing.T) {
	run := func(category weft.Category, plain bool, o weft.SubscribeOptions) (*rig, int) {
		r := newRig(t, "", nil, false)
		var calls atomic.Int32
		_ = r.m.On("order.placed", func(context.Context, any, *weft.MessageContext) error {
			calls.Add(1)
			if plain {
				return errors.New("bug")
			}
			return weft.NewError(category, "failed")
		}, o)
		r.start(t)
		if err := r.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{}); err != nil {
			t.Fatal(err)
		}
		r.broker.Idle()
		return r, int(calls.Load())
	}
	ms := func(d ...int) []time.Duration {
		var out []time.Duration
		for _, x := range d {
			out = append(out, time.Duration(x)*time.Millisecond)
		}
		return out
	}

	r, calls := run(weft.Timeout, false, weft.SubscribeOptions{})
	if calls != 5 || !slices.Equal(r.slept(), ms(150, 300, 600, 1200)) {
		t.Errorf("calls %d, sleeps %v", calls, r.slept())
	}
	dead := r.broker.To("order.placed.dlq")
	if len(dead) != 1 {
		t.Fatalf("dead letters = %d", len(dead))
	}
	h := dead[0].Envelope.Headers
	if h["weft-error-category"] != "Timeout" || h["weft-attempts"] != "5" || h["weft-original-destination"] != "order.placed" || h["weft-error-message"] != "failed" {
		t.Errorf("headers = %v", h)
	}
	if dead[0].Envelope.ID != r.broker.To("order.placed")[0].Envelope.ID {
		t.Error("a dead letter keeps the message's id")
	}

	if r, calls := run(weft.InvalidInput, false, weft.SubscribeOptions{}); calls != 1 || len(r.slept()) != 0 || len(r.broker.To("order.placed.dlq")) != 1 {
		t.Errorf("InvalidInput: %d %v", calls, r.slept())
	}
	if r, calls := run("", true, weft.SubscribeOptions{}); calls != 5 || r.broker.To("order.placed.dlq")[0].Envelope.Headers["weft-error-category"] != "Internal" {
		t.Errorf("plain error: %d", calls)
	}
	for _, c := range []weft.Category{weft.NotFound, weft.Unauthorized, weft.Forbidden, weft.Conflict} {
		if r, calls := run(c, false, weft.SubscribeOptions{}); calls != 1 || len(r.broker.To("order.placed.dlq")) != 0 {
			t.Errorf("%s: calls %d", c, calls)
		}
	}
	if r, calls := run(weft.Unavailable, false, weft.SubscribeOptions{Attempts: 2, DeadLetter: "failed.orders"}); calls != 2 || len(r.broker.To("failed.orders")) != 1 {
		t.Errorf("options: calls %d", calls)
	}
	if r, calls := run(weft.Internal, false, weft.SubscribeOptions{Attempts: 1, NoDeadLetter: true}); calls != 1 || len(r.broker.Published()) != 1 {
		t.Errorf("no dead letters: calls %d, published %d", calls, len(r.broker.Published()))
	}
}

func TestAMessageThatBreaksTheContractIsDeadLetteredUnseen(t *testing.T) {
	r := newRig(t, "", nil, false)
	var calls atomic.Int32
	_ = r.m.On("order.placed", func(context.Context, any, *weft.MessageContext) error { calls.Add(1); return nil }, weft.SubscribeOptions{})
	r.start(t)
	for _, body := range []string{`{"id": 5}`, "not json"} {
		r.adapter.Publish(context.Background(), "order.placed", weft.Envelope{ID: body, Type: "order.placed", Body: []byte(body), Headers: map[string]string{}}, nil)
	}
	r.broker.Idle()
	if calls.Load() != 0 || len(r.broker.To("order.placed.dlq")) != 2 {
		t.Errorf("calls %d, dead letters %d", calls.Load(), len(r.broker.To("order.placed.dlq")))
	}
}

func TestAMessageBeingRetriedAtShutdownIsNotAcknowledged(t *testing.T) {
	r := newRig(t, "", nil, true)
	var calls atomic.Int32
	_ = r.m.On("order.placed", func(context.Context, any, *weft.MessageContext) error {
		calls.Add(1)
		return weft.NewError(weft.Unavailable, "down")
	}, weft.SubscribeOptions{})
	r.start(t)
	_ = r.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{})
	for i := 0; i < 400 && calls.Load() < 1; i++ {
		time.Sleep(5 * time.Millisecond)
	}
	r.weft.Stop(context.Background())
	if len(r.broker.Unacked()) != 1 || len(r.broker.To("order.placed.dlq")) != 0 || calls.Load() != 1 {
		t.Errorf("unacked %d, dead letters %d, calls %d", len(r.broker.Unacked()), len(r.broker.To("order.placed.dlq")), calls.Load())
	}
}

func TestPublishFailures(t *testing.T) {
	r := newRig(t, "", nil, false)
	r.start(t)
	r.adapter.FailNextPublishes(2)
	if err := r.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{}); err != nil {
		t.Fatal(err)
	}
	if got := r.slept(); len(got) != 2 || got[0] != 150*time.Millisecond || got[1] != 300*time.Millisecond {
		t.Errorf("sleeps = %v", got)
	}
	r2 := newRig(t, "", nil, false)
	r2.start(t)
	r2.adapter.FailNextPublishes(99)
	if err := r2.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{}); weft.AsError(err).Category != weft.Unavailable || len(r2.slept()) != 4 {
		t.Errorf("gives up after five attempts: %v, %v", err, r2.slept())
	}
}

func TestMiddlewareAndDedupe(t *testing.T) {
	r := newRig(t, "", nil, false)
	var mu sync.Mutex
	var log []string
	add := func(s string) { mu.Lock(); log = append(log, s); mu.Unlock() }
	r.weft.Use(func(ctx context.Context, c *weft.Call, next weft.Next) error { add("weft"); return next(ctx) })
	r.m.Use(func(ctx context.Context, mc *weft.MessageContext, next weft.Next) error {
		add("kafka " + mc.Event)
		return next(ctx)
	})
	r.m.Use(weft.Dedupe(0))
	_ = r.m.On("order.placed", func(context.Context, any, *weft.MessageContext) error { add("handler"); return nil }, weft.SubscribeOptions{})
	r.start(t)
	for _, key := range []string{"same", "same", "other"} {
		_ = r.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{IdempotencyKey: key})
		r.broker.Idle()
	}
	want := []string{"weft", "kafka order.placed", "handler", "weft", "kafka order.placed", "weft", "kafka order.placed", "handler"}
	if !slices.Equal(log, want) {
		t.Errorf("log = %v", log)
	}
}

func TestDedupeForgetsTheOldestKeyPastItsLimit(t *testing.T) {
	mw := weft.Dedupe(2)
	run := func(id string) bool {
		ran := false
		_ = mw(context.Background(), &weft.MessageContext{Envelope: weft.Envelope{ID: id}}, func(context.Context) error { ran = true; return nil })
		return ran
	}
	if !run("a") || run("a") {
		t.Fatal("a is handled once")
	}
	run("b")
	run("c")
	if !run("a") {
		t.Error("a was forgotten")
	}
}

func TestBackoff(t *testing.T) {
	r := func(v float64) func() float64 { return func() float64 { return v } }
	cases := []struct {
		attempt int
		random  float64
		b       weft.Backoff
		want    time.Duration
	}{
		{1, 0, weft.Backoff{}, 100 * time.Millisecond},
		{1, 1, weft.Backoff{}, 200 * time.Millisecond},
		{2, 1, weft.Backoff{}, 400 * time.Millisecond},
		{3, 1, weft.Backoff{}, 800 * time.Millisecond},
		{30, 1, weft.Backoff{}, 30 * time.Second},
		{30, 0, weft.Backoff{}, 15 * time.Second},
		{2, 1, weft.Backoff{Min: 10 * time.Millisecond, Max: 15 * time.Millisecond}, 15 * time.Millisecond},
	}
	for _, c := range cases {
		if got := weft.BackoffDelay(c.attempt, c.b, r(c.random)); got != c.want {
			t.Errorf("attempt %d random %v: %v, want %v", c.attempt, c.random, got, c.want)
		}
	}
}

func TestCloudEventsHeaders(t *testing.T) {
	e := weft.Envelope{ID: "1", Type: "order.placed", Source: "svc", Time: "2026-11-23T10:00:00.000Z", DataContentType: "application/json", Key: "k", Headers: map[string]string{"tenant": "t"}, Body: []byte{1}}
	h := weft.ToHeaders(e, "ce_")
	for k, v := range map[string]string{"ce_specversion": "1.0", "ce_id": "1", "ce_type": "order.placed", "ce_source": "svc", "ce_datacontenttype": "application/json", "tenant": "t"} {
		if h[k] != v {
			t.Errorf("%s = %q", k, h[k])
		}
	}
	back, missing := weft.FromHeaders(h, "ce_", e.Body, "k")
	if missing != nil || back.ID != "1" || back.Key != "k" || back.Headers["tenant"] != "t" || len(back.Headers) != 1 {
		t.Errorf("back = %+v, missing %v", back, missing)
	}
	if _, missing := weft.FromHeaders(map[string]string{"tenant": "t"}, "ce_", nil, ""); len(missing) != 4 {
		t.Errorf("missing = %v", missing)
	}
}
