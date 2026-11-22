package weft

import (
	"context"
	"time"
)

// Call is what every handler of every transport can see about one call. Transports add their own
// fields in types that embed it.
type Call struct {
	Transport string
	// Operation is an event name, a route name or a method.
	Operation string
	Headers   map[string]string
	// State is scratch space for middleware to pass values along.
	State map[string]any
}

// NewCall makes a Call with its maps ready to use.
func NewCall(transport, operation string) *Call {
	return &Call{Transport: transport, Operation: operation, Headers: map[string]string{}, State: map[string]any{}}
}

// Next continues to the next middleware, and finally to the handler.
type Next func(ctx context.Context) error

// Middleware wraps a handler. It continues by calling next once; not calling it stops the chain, and
// calling it twice is an error.
type Middleware func(ctx context.Context, call *Call, next Next) error

// Handler is what a transport runs for one call.
type Handler func(ctx context.Context, call *Call) error

// Compose joins middleware in front of a handler, outermost first.
func Compose(middleware []Middleware, handler Handler) Handler {
	return func(ctx context.Context, call *Call) error {
		last := -1
		var run func(i int) Next
		run = func(i int) Next {
			return func(ctx context.Context) error {
				if i <= last {
					return NewError(Internal, "next() was called more than once in the same middleware")
				}
				last = i
				if i == len(middleware) {
					return handler(ctx, call)
				}
				return middleware[i](ctx, call, run(i+1))
			}
		}
		return run(0)(ctx)
	}
}

// Remaining is the time until ctx's deadline, and false when it has none. Never negative.
func Remaining(ctx context.Context, now time.Time) (time.Duration, bool) {
	d, ok := ctx.Deadline()
	if !ok {
		return 0, false
	}
	if left := d.Sub(now); left > 0 {
		return left, true
	}
	return 0, true
}
