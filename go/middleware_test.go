package weft_test

import (
	"context"
	"errors"
	"slices"
	"testing"
	"time"

	weft "github.com/weft-sdk/weft/go"
)

func TestComposeRunsOutermostFirst(t *testing.T) {
	var order []string
	mw := func(name string) weft.Middleware {
		return func(ctx context.Context, c *weft.Call, next weft.Next) error {
			order = append(order, name+" in")
			err := next(ctx)
			order = append(order, name+" out")
			return err
		}
	}
	h := weft.Compose([]weft.Middleware{mw("a"), mw("b")}, func(context.Context, *weft.Call) error { order = append(order, "handler"); return nil })
	if err := h(context.Background(), weft.NewCall("t", "op")); err != nil {
		t.Fatal(err)
	}
	if !slices.Equal(order, []string{"a in", "b in", "handler", "b out", "a out"}) {
		t.Errorf("order = %v", order)
	}
}

func TestAMiddlewareThatDoesNotCallNextStopsTheChain(t *testing.T) {
	called := false
	h := weft.Compose([]weft.Middleware{func(context.Context, *weft.Call, weft.Next) error { return nil }},
		func(context.Context, *weft.Call) error { called = true; return nil })
	_ = h(context.Background(), weft.NewCall("t", "op"))
	if called {
		t.Error("the handler should not run")
	}
}

func TestErrorsAndStatePassThrough(t *testing.T) {
	boom := errors.New("boom")
	h := weft.Compose([]weft.Middleware{func(ctx context.Context, c *weft.Call, next weft.Next) error {
		c.State["user"] = "u1"
		return next(ctx)
	}}, func(_ context.Context, c *weft.Call) error {
		if c.State["user"] != "u1" {
			t.Error("state was not shared")
		}
		return boom
	})
	if err := h(context.Background(), weft.NewCall("t", "op")); !errors.Is(err, boom) {
		t.Errorf("err = %v", err)
	}
}

func TestNextTwiceIsAnError(t *testing.T) {
	twice := func(ctx context.Context, c *weft.Call, next weft.Next) error {
		if err := next(ctx); err != nil {
			return err
		}
		return next(ctx)
	}
	err := weft.Compose([]weft.Middleware{twice}, func(context.Context, *weft.Call) error { return nil })(context.Background(), weft.NewCall("t", "op"))
	if e := weft.AsError(err); e.Category != weft.Internal {
		t.Errorf("err = %v", err)
	}
}

func TestDeadlinesOnlyShorten(t *testing.T) {
	now := time.Unix(1000, 0)
	if got := weft.DeadlineAfter(time.Second, time.Time{}, now); !got.Equal(now.Add(time.Second)) {
		t.Error(got)
	}
	if got := weft.DeadlineAfter(time.Second, now.Add(400*time.Millisecond), now); !got.Equal(now.Add(400 * time.Millisecond)) {
		t.Error(got)
	}
	if got := weft.DeadlineAfter(time.Second, now.Add(5*time.Second), now); !got.Equal(now.Add(time.Second)) {
		t.Error(got)
	}
	ctx, cancel := context.WithDeadline(context.Background(), now.Add(500*time.Millisecond))
	defer cancel()
	if left, ok := weft.Remaining(ctx, now); !ok || left != 500*time.Millisecond {
		t.Errorf("left = %v %v", left, ok)
	}
	if left, ok := weft.Remaining(ctx, now.Add(time.Hour)); !ok || left != 0 {
		t.Errorf("never negative: %v %v", left, ok)
	}
	if _, ok := weft.Remaining(context.Background(), now); ok {
		t.Error("no deadline")
	}
}

func TestWithTimeout(t *testing.T) {
	ctx := context.Background()
	if v, err := weft.WithTimeout(ctx, time.Second, func(context.Context) (int, error) { return 7, nil }); err != nil || v != 7 {
		t.Errorf("%v %v", v, err)
	}

	seen := make(chan context.Context, 1)
	_, err := weft.WithTimeout(ctx, 20*time.Millisecond, func(c context.Context) (int, error) {
		seen <- c
		<-c.Done()
		time.Sleep(10 * time.Millisecond)
		return 0, c.Err()
	})
	if e := weft.AsError(err); e.Category != weft.Timeout {
		t.Errorf("err = %v", err)
	}
	if (<-seen).Err() == nil {
		t.Error("the context should be cancelled")
	}

	parent, cancel := context.WithCancel(ctx)
	go func() { time.Sleep(10 * time.Millisecond); cancel() }()
	_, err = weft.WithTimeout(parent, 5*time.Second, func(c context.Context) (int, error) { <-c.Done(); return 0, c.Err() })
	if e := weft.AsError(err); e.Category != weft.Unavailable {
		t.Errorf("err = %v", err)
	}
	_, err = weft.WithTimeout(parent, 5*time.Second, func(context.Context) (int, error) { return 1, nil })
	if e := weft.AsError(err); e.Category != weft.Unavailable {
		t.Errorf("an already-cancelled parent: %v", err)
	}

	boom := weft.NewError(weft.NotFound, "x")
	if _, err := weft.WithTimeout(ctx, time.Second, func(context.Context) (int, error) { return 0, boom }); err != boom {
		t.Errorf("the function's own error passes through: %v", err)
	}
}
