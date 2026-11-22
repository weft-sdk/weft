package weft_test

import (
	"context"
	"errors"
	"slices"
	"sync"
	"testing"
	"time"

	weft "github.com/weft-sdk/weft/go"
)

type log struct {
	mu     sync.Mutex
	events []string
}

func (l *log) add(e string) {
	l.mu.Lock()
	defer l.mu.Unlock()
	l.events = append(l.events, e)
}

func (l *log) all() []string {
	l.mu.Lock()
	defer l.mu.Unlock()
	return slices.Clone(l.events)
}

type fake struct {
	name      string
	log       *log
	failStart bool
	failClose bool
}

func (f *fake) Name() string { return f.name }
func (f *fake) Start(context.Context) error {
	f.log.add(f.name + " start")
	if f.failStart {
		return errors.New("no broker")
	}
	return nil
}
func (f *fake) StopIntake(context.Context) error { f.log.add(f.name + " stopIntake"); return nil }
func (f *fake) Close(context.Context) error {
	f.log.add(f.name + " close")
	if f.failClose {
		return errors.New("stuck")
	}
	return nil
}

func newWeft(t *testing.T, grace time.Duration) *weft.Weft {
	t.Helper()
	cfg, err := weft.ParseConfig("service: test", nil)
	if err != nil {
		t.Fatal(err)
	}
	return weft.New(weft.Options{Contracts: weft.NewContracts(), Config: cfg, ShutdownGrace: grace})
}

func must(t *testing.T, err error) {
	t.Helper()
	if err != nil {
		t.Fatal(err)
	}
}

func TestStartsInOrderAndStopsInReverse(t *testing.T) {
	var l log
	w := newWeft(t, time.Second)
	must(t, w.Register(&fake{name: "http", log: &l}))
	must(t, w.Register(&fake{name: "kafka", log: &l}))
	must(t, w.Start(context.Background()))
	if w.State() != weft.Running {
		t.Fatal(w.State())
	}
	w.Stop(context.Background())
	want := []string{"http start", "kafka start", "kafka stopIntake", "http stopIntake", "kafka close", "http close"}
	got := l.all()
	// The two stopIntakes run together, so their order is not fixed; the rest is.
	if len(got) != len(want) || got[0] != want[0] || got[1] != want[1] || got[4] != want[4] || got[5] != want[5] {
		t.Errorf("events = %v", got)
	}
	if w.State() != weft.Stopped {
		t.Error(w.State())
	}
}

func TestDispatchRunsGlobalThenTransportMiddlewareThenTheHandler(t *testing.T) {
	w := newWeft(t, time.Second)
	var order []string
	w.Use(func(ctx context.Context, c *weft.Call, next weft.Next) error {
		order = append(order, "global")
		return next(ctx)
	})
	must(t, w.Start(context.Background()))
	err := w.Dispatch(context.Background(), weft.NewCall("t", "op"),
		func(context.Context, *weft.Call) error { order = append(order, "handler"); return nil },
		func(ctx context.Context, c *weft.Call, next weft.Next) error {
			order = append(order, "transport")
			return next(ctx)
		})
	must(t, err)
	if !slices.Equal(order, []string{"global", "transport", "handler"}) {
		t.Errorf("order = %v", order)
	}
	w.Stop(context.Background())
}

func TestStopWaitsForInFlightWorkAndRefusesNewWork(t *testing.T) {
	var l log
	w := newWeft(t, 2*time.Second)
	must(t, w.Register(&fake{name: "http", log: &l}))
	must(t, w.Start(context.Background()))

	gate := make(chan struct{})
	started := make(chan struct{})
	handled := make(chan error, 1)
	go func() {
		handled <- w.Dispatch(context.Background(), weft.NewCall("t", "op"), func(context.Context, *weft.Call) error {
			close(started)
			<-gate
			l.add("handler done")
			return nil
		})
	}()
	<-started

	stopped := make(chan weft.StopResult, 1)
	go func() { stopped <- w.Stop(context.Background()) }()
	time.Sleep(50 * time.Millisecond)

	err := w.Dispatch(context.Background(), weft.NewCall("t", "late"), func(context.Context, *weft.Call) error { return nil })
	if weft.AsError(err).Category != weft.Unavailable {
		t.Errorf("late work: %v", err)
	}
	if slices.Contains(l.all(), "http close") {
		t.Error("closed before the handler finished")
	}
	close(gate)
	must(t, <-handled)
	if r := <-stopped; r != (weft.StopResult{Drained: true}) {
		t.Errorf("result = %+v", r)
	}
	events := l.all()
	if events[len(events)-2] != "handler done" || events[len(events)-1] != "http close" {
		t.Errorf("events = %v", events)
	}
}

func TestStopGivesUpAfterTheGracePeriodAndStillCloses(t *testing.T) {
	var l log
	w := newWeft(t, 30*time.Millisecond)
	must(t, w.Register(&fake{name: "http", log: &l}))
	must(t, w.Start(context.Background()))
	block := make(chan struct{})
	defer close(block)
	started := make(chan struct{})
	go w.Dispatch(context.Background(), weft.NewCall("t", "op"), func(context.Context, *weft.Call) error {
		close(started)
		<-block
		return nil
	})
	<-started
	r := w.Stop(context.Background())
	if r.Drained || r.Abandoned != 1 {
		t.Errorf("result = %+v", r)
	}
	if w.ShutdownContext().Err() == nil {
		t.Error("the shutdown context should be cancelled")
	}
	if !slices.Contains(l.all(), "http close") {
		t.Error("it should still close")
	}
}

func TestAFailingHandlerCountsAsFinished(t *testing.T) {
	w := newWeft(t, time.Second)
	must(t, w.Start(context.Background()))
	boom := errors.New("x")
	if err := w.Dispatch(context.Background(), weft.NewCall("t", "op"), func(context.Context, *weft.Call) error { return boom }); !errors.Is(err, boom) {
		t.Errorf("err = %v", err)
	}
	if r := w.Stop(context.Background()); r != (weft.StopResult{Drained: true}) {
		t.Errorf("result = %+v", r)
	}
}

func TestAFailedStartClosesWhatAlreadyStarted(t *testing.T) {
	var l log
	w := newWeft(t, time.Second)
	must(t, w.Register(&fake{name: "http", log: &l}))
	must(t, w.Register(&fake{name: "kafka", log: &l, failStart: true}))
	err := w.Start(context.Background())
	if e := weft.AsError(err); e.Category != weft.Unavailable || e.Message != "Could not start kafka: no broker" {
		t.Errorf("err = %v", err)
	}
	want := []string{"http start", "kafka start", "http stopIntake", "http close"}
	if !slices.Equal(l.all(), want) {
		t.Errorf("events = %v", l.all())
	}
	if w.State() != weft.Stopped {
		t.Error(w.State())
	}
}

func TestATransportThatWillNotCloseDoesNotKeepTheOthersOpen(t *testing.T) {
	var l log
	w := newWeft(t, time.Second)
	must(t, w.Register(&fake{name: "http", log: &l}))
	must(t, w.Register(&fake{name: "kafka", log: &l, failClose: true}))
	must(t, w.Start(context.Background()))
	w.Stop(context.Background())
	if !slices.Contains(l.all(), "http close") {
		t.Errorf("events = %v", l.all())
	}
}

func TestStopIsSafeToCallTwiceOrBeforeStart(t *testing.T) {
	if r := newWeft(t, time.Second).Stop(context.Background()); r != (weft.StopResult{Drained: true}) {
		t.Errorf("result = %+v", r)
	}
	w := newWeft(t, time.Second)
	must(t, w.Start(context.Background()))
	var wg sync.WaitGroup
	results := make([]weft.StopResult, 3)
	for i := range results {
		wg.Add(1)
		go func() { defer wg.Done(); results[i] = w.Stop(context.Background()) }()
	}
	wg.Wait()
	for _, r := range results {
		if r != (weft.StopResult{Drained: true}) {
			t.Errorf("results = %+v", results)
		}
	}
}

func TestRegisterAndStartAreChecked(t *testing.T) {
	var l log
	w := newWeft(t, time.Second)
	must(t, w.Register(&fake{name: "http", log: &l}))
	if err := w.Register(&fake{name: "http", log: &l}); err == nil {
		t.Error("a name twice")
	}
	must(t, w.Start(context.Background()))
	if err := w.Start(context.Background()); err == nil {
		t.Error("start twice")
	}
	if err := w.Register(&fake{name: "kafka", log: &l}); err == nil {
		t.Error("register after start")
	}
	w.Stop(context.Background())
}
