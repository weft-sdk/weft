package weft

import (
	"context"
	"errors"
	"fmt"
	"time"
)

// WithTimeout runs fn with a context that is cancelled after d, or when parent is. If the time runs
// out first the error is Timeout; if the parent was cancelled, Unavailable. fn should pass the
// context on to whatever it calls. A deadline only ever gets shorter: if parent already has an
// earlier one, that one stays.
func WithTimeout[T any](parent context.Context, d time.Duration, fn func(ctx context.Context) (T, error)) (T, error) {
	var zero T
	if err := parent.Err(); err != nil {
		return zero, NewError(Unavailable, "The call was cancelled")
	}
	ctx, cancel := context.WithTimeout(parent, d)
	defer cancel()

	type result struct {
		v   T
		err error
	}
	done := make(chan result, 1)
	go func() {
		v, err := fn(ctx)
		done <- result{v, err}
	}()
	select {
	case r := <-done:
		return r.v, r.err
	case <-ctx.Done():
		if errors.Is(ctx.Err(), context.DeadlineExceeded) && parent.Err() == nil {
			return zero, NewError(Timeout, fmt.Sprintf("Timed out after %d ms", d.Milliseconds()))
		}
		if parent.Err() != nil {
			return zero, NewError(Unavailable, "The call was cancelled")
		}
		return zero, NewError(Timeout, fmt.Sprintf("Timed out after %d ms", d.Milliseconds()))
	}
}

// DeadlineAfter is the earlier of "d from now" and an existing deadline.
func DeadlineAfter(d time.Duration, existing time.Time, now time.Time) time.Time {
	mine := now.Add(d)
	if existing.IsZero() || mine.Before(existing) {
		return mine
	}
	return existing
}
