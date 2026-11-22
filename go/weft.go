package weft

import (
	"context"
	"sync"
	"time"
)

// Transport is one way services talk (HTTP, Kafka, ...), run by an adapter on a real library.
type Transport interface {
	Name() string
	// Start connects and begins listening. Called by Weft.Start.
	Start(ctx context.Context) error
	// StopIntake stops taking new work. In-flight work carries on.
	StopIntake(ctx context.Context) error
	// Close closes connections, after in-flight work has finished or the grace period has run out.
	Close(ctx context.Context) error
}

// State is where a Weft is in its life.
type State string

const (
	Created  State = "created"
	Starting State = "starting"
	Running  State = "running"
	Stopping State = "stopping"
	Stopped  State = "stopped"
)

// StopResult says how a Stop ended.
type StopResult struct {
	// Drained is false when the grace period ran out with handlers still running.
	Drained bool
	// Abandoned is how many handlers were still running then.
	Abandoned int
}

// Options configures a Weft.
type Options struct {
	Contracts *Contracts
	Config    *Config
	// ShutdownGrace is how long Stop waits for in-flight handlers. Default 30 s.
	ShutdownGrace time.Duration
}

// Weft is the lifecycle shared by every transport: register, Start, run handlers through the
// middleware, Stop. Adapters register themselves as transports and send every handler call
// through Dispatch.
type Weft struct {
	Contracts *Contracts
	Config    *Config
	grace     time.Duration

	mu         sync.Mutex
	state      State
	transports []Transport
	started    []Transport
	middleware []Middleware
	inflight   sync.WaitGroup
	count      int
	stopping   chan StopResult
	shutdown   context.Context
	abort      context.CancelFunc
}

// New makes a Weft.
func New(o Options) *Weft {
	grace := o.ShutdownGrace
	if grace == 0 {
		grace = ShutdownGrace
	}
	ctx, cancel := context.WithCancel(context.Background())
	return &Weft{Contracts: o.Contracts, Config: o.Config, grace: grace, state: Created, shutdown: ctx, abort: cancel}
}

// State is where this Weft is in its life.
func (w *Weft) State() State {
	w.mu.Lock()
	defer w.mu.Unlock()
	return w.state
}

// ShutdownContext is cancelled when Stop gives up on handlers that did not finish in the grace period.
func (w *Weft) ShutdownContext() context.Context { return w.shutdown }

// Use adds middleware that runs for every transport, in the order added.
func (w *Weft) Use(m Middleware) {
	w.mu.Lock()
	defer w.mu.Unlock()
	w.middleware = append(w.middleware, m)
}

// Register adds a transport. Only before Start.
func (w *Weft) Register(t Transport) error {
	w.mu.Lock()
	defer w.mu.Unlock()
	if w.state != Created {
		return errorf(Internal, "Cannot register %s: Weft has already started", t.Name())
	}
	for _, existing := range w.transports {
		if existing.Name() == t.Name() {
			return errorf(Internal, "The transport %s is already registered", t.Name())
		}
	}
	w.transports = append(w.transports, t)
	return nil
}

// Start connects every transport, in the order registered. If one fails, the ones already started
// are closed again and the error is Unavailable.
func (w *Weft) Start(ctx context.Context) error {
	w.mu.Lock()
	if w.state != Created {
		s := w.state
		w.mu.Unlock()
		return errorf(Internal, "Cannot start: Weft is %s", s)
	}
	w.state = Starting
	transports := append([]Transport(nil), w.transports...)
	w.mu.Unlock()

	for _, t := range transports {
		if err := t.Start(ctx); err != nil {
			w.unwind(ctx)
			w.setState(Stopped)
			return &Error{Category: Unavailable, Message: "Could not start " + t.Name() + ": " + err.Error(), Cause: err}
		}
		w.mu.Lock()
		w.started = append(w.started, t)
		w.mu.Unlock()
	}
	w.setState(Running)
	return nil
}

func (w *Weft) setState(s State) {
	w.mu.Lock()
	w.state = s
	w.mu.Unlock()
}

func (w *Weft) unwind(ctx context.Context) {
	w.mu.Lock()
	started := append([]Transport(nil), w.started...)
	w.started = nil
	w.mu.Unlock()
	for i := len(started) - 1; i >= 0; i-- {
		_ = started[i].StopIntake(ctx)
		_ = started[i].Close(ctx)
	}
}

// Stop stops taking new work, waits for in-flight handlers (up to the grace period), then closes
// every transport in reverse order. It is safe to call more than once. It does not fail for a
// handler that is still running: check the result.
func (w *Weft) Stop(ctx context.Context) StopResult {
	w.mu.Lock()
	if w.stopping != nil {
		ch := w.stopping
		w.mu.Unlock()
		r := <-ch
		ch <- r // let the next caller read it too
		return r
	}
	if w.state == Created {
		w.state = Stopped
		w.mu.Unlock()
		return StopResult{Drained: true}
	}
	w.state = Stopping
	w.stopping = make(chan StopResult, 1)
	ch := w.stopping
	started := append([]Transport(nil), w.started...)
	w.mu.Unlock()

	var wg sync.WaitGroup
	for _, t := range started {
		wg.Add(1)
		go func() { defer wg.Done(); _ = t.StopIntake(ctx) }()
	}
	wg.Wait()

	result := w.drain()
	if !result.Drained {
		w.abort()
	}
	for i := len(started) - 1; i >= 0; i-- {
		_ = started[i].Close(ctx) // one that fails to close must not keep the others open
	}
	w.setState(Stopped)
	ch <- result
	return result
}

func (w *Weft) drain() StopResult {
	done := make(chan struct{})
	go func() { w.inflight.Wait(); close(done) }()
	timer := time.NewTimer(w.grace)
	defer timer.Stop()
	select {
	case <-done:
		return StopResult{Drained: true}
	case <-timer.C:
		w.mu.Lock()
		n := w.count
		w.mu.Unlock()
		return StopResult{Drained: false, Abandoned: n}
	}
}

// Track runs fn as in-flight work that Stop waits for. It is refused once stopping has begun.
func (w *Weft) Track(fn func() error) error {
	w.mu.Lock()
	if w.state == Stopping || w.state == Stopped {
		w.mu.Unlock()
		return NewError(Unavailable, "Weft is shutting down")
	}
	w.inflight.Add(1)
	w.count++
	w.mu.Unlock()
	defer func() {
		w.mu.Lock()
		w.count--
		w.mu.Unlock()
		w.inflight.Done()
	}()
	return fn()
}

// Dispatch runs handler for one call: the global middleware, then extra (the transport's own), then
// the handler, counted as in-flight work.
func (w *Weft) Dispatch(ctx context.Context, call *Call, handler Handler, extra ...Middleware) error {
	w.mu.Lock()
	chain := append(append([]Middleware(nil), w.middleware...), extra...)
	w.mu.Unlock()
	return w.Track(func() error { return Compose(chain, handler)(ctx, call) })
}
