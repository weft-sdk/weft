package weft

import (
	"container/list"
	"context"
	"sync"
)

// IdempotencyHeader carries the idempotency key of a message.
const IdempotencyHeader = "weft-idempotency-key"

// Dedupe returns middleware that skips a message whose idempotency key (else its CloudEvents id) was
// already handled successfully. It remembers up to max keys (10 000 if max is 0), the oldest forgotten
// first. Memory only: it covers redelivery to this process, not a restart. A key is remembered only
// after the handler succeeds, so a failed message is retried.
func Dedupe(max int) MessageMiddleware {
	if max <= 0 {
		max = 10_000
	}
	var mu sync.Mutex
	seen := map[string]*list.Element{}
	order := list.New()
	return func(ctx context.Context, mc *MessageContext, next Next) error {
		key := mc.Envelope.Headers[IdempotencyHeader]
		if key == "" {
			key = mc.Envelope.ID
		}
		mu.Lock()
		_, dup := seen[key]
		mu.Unlock()
		if dup {
			return nil
		}
		if err := next(ctx); err != nil {
			return err
		}
		mu.Lock()
		defer mu.Unlock()
		if _, ok := seen[key]; !ok {
			seen[key] = order.PushBack(key)
			if order.Len() > max {
				oldest := order.Front()
				delete(seen, oldest.Value.(string))
				order.Remove(oldest)
			}
		}
		return nil
	}
}
