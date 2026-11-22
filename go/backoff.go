package weft

import (
	"context"
	"math"
	"time"
)

// Backoff bounds the wait between attempts. Zero values mean the defaults (200 ms to 30 s).
type Backoff struct {
	Min time.Duration
	Max time.Duration
}

// BackoffDelay is exponential backoff with jitter: the delay doubles from Min up to Max, then a
// random amount between half of it and all of it is used. attempt is the one that just failed (1, 2, ...).
// random returns a number in [0, 1).
func BackoffDelay(attempt int, b Backoff, random func() float64) time.Duration {
	min, max := b.Min, b.Max
	if min == 0 {
		min = BackoffMin
	}
	if max == 0 {
		max = BackoffMax
	}
	exp := attempt - 1
	if exp < 0 {
		exp = 0
	}
	base := math.Min(float64(max.Milliseconds()), float64(min.Milliseconds())*math.Pow(2, float64(exp)))
	return time.Duration(math.Round(base/2+base/2*random())) * time.Millisecond
}

// Sleep waits d, or less if ctx is cancelled. It returns either way; the caller checks ctx.
func Sleep(ctx context.Context, d time.Duration) {
	t := time.NewTimer(d)
	defer t.Stop()
	select {
	case <-t.C:
	case <-ctx.Done():
	}
}
