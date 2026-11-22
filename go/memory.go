package weft

import (
	"context"
	"fmt"
	"sync"
)

// Published is a message as the in-memory broker saw it.
type Published struct {
	Destination string
	Envelope    Envelope
	Options     map[string]any
}

type memorySubscriber struct {
	group   string
	handler func(context.Context, Delivery) error
	owner   *MemoryAdapter
}

// MemoryBroker is an in-process broker for tests and examples. Each consumer group gets every message
// once, handed to one of its subscribers in turn; a subscriber that returns an error leaves the message
// in Unacked.
type MemoryBroker struct {
	mu          sync.Mutex
	published   []Published
	unacked     []Published
	subscribers map[string][]memorySubscriber
	turn        map[string]int
	pending     sync.WaitGroup
}

// NewMemoryBroker makes an empty broker.
func NewMemoryBroker() *MemoryBroker {
	return &MemoryBroker{subscribers: map[string][]memorySubscriber{}, turn: map[string]int{}}
}

// Published is every message sent so far.
func (b *MemoryBroker) Published() []Published {
	b.mu.Lock()
	defer b.mu.Unlock()
	return append([]Published(nil), b.published...)
}

// To is the messages sent to one destination.
func (b *MemoryBroker) To(destination string) []Published {
	var out []Published
	for _, p := range b.Published() {
		if p.Destination == destination {
			out = append(out, p)
		}
	}
	return out
}

// Unacked is the messages a subscriber refused to acknowledge.
func (b *MemoryBroker) Unacked() []Published {
	b.mu.Lock()
	defer b.mu.Unlock()
	return append([]Published(nil), b.unacked...)
}

// Idle waits until every delivery started so far has finished.
func (b *MemoryBroker) Idle() { b.pending.Wait() }

func (b *MemoryBroker) publish(destination string, e Envelope, options map[string]any) {
	b.mu.Lock()
	b.published = append(b.published, Published{destination, e, options})
	groups := map[string][]memorySubscriber{}
	var order []string
	for _, s := range b.subscribers[destination] {
		if _, ok := groups[s.group]; !ok {
			order = append(order, s.group)
		}
		groups[s.group] = append(groups[s.group], s)
	}
	var targets []memorySubscriber
	for _, g := range order {
		k := destination + "/" + g
		i := b.turn[k]
		b.turn[k] = i + 1
		members := groups[g]
		targets = append(targets, members[i%len(members)])
	}
	b.mu.Unlock()

	for _, t := range targets {
		b.pending.Add(1)
		go func() {
			defer b.pending.Done()
			if err := t.handler(context.Background(), Delivery{Envelope: e, DeliveryCount: 1}); err != nil {
				b.mu.Lock()
				b.unacked = append(b.unacked, Published{destination, e, options})
				b.mu.Unlock()
			}
		}()
	}
}

// MemoryAdapter is the messaging adapter over a MemoryBroker.
type MemoryAdapter struct {
	name   string
	broker *MemoryBroker

	mu          sync.Mutex
	intake      bool
	failPublish int
}

// NewMemoryAdapter makes an adapter named name over broker (a new one if broker is nil).
func NewMemoryAdapter(name string, broker *MemoryBroker) *MemoryAdapter {
	if broker == nil {
		broker = NewMemoryBroker()
	}
	return &MemoryAdapter{name: name, broker: broker, intake: true}
}

// FailNextPublishes makes the next n calls to Publish fail with Unavailable, to test retries.
func (a *MemoryAdapter) FailNextPublishes(n int) {
	a.mu.Lock()
	a.failPublish = n
	a.mu.Unlock()
}

func (a *MemoryAdapter) Name() string          { return a.name }
func (a *MemoryAdapter) Raw() any              { return a.broker }
func (a *MemoryAdapter) Broker() *MemoryBroker { return a.broker }

func (a *MemoryAdapter) Capabilities() MessagingCapabilities {
	return MessagingCapabilities{Ordering: true, Keys: true, Headers: true, ConsumerGroups: true}
}

func (a *MemoryAdapter) Start(context.Context) error {
	a.mu.Lock()
	a.intake = true
	a.mu.Unlock()
	return nil
}

func (a *MemoryAdapter) StopIntake(context.Context) error {
	a.mu.Lock()
	a.intake = false
	a.mu.Unlock()
	return nil
}

func (a *MemoryAdapter) Close(context.Context) error {
	a.broker.Idle()
	return nil
}

func (a *MemoryAdapter) Publish(_ context.Context, destination string, e Envelope, options map[string]any) error {
	a.mu.Lock()
	if a.failPublish > 0 {
		a.failPublish--
		a.mu.Unlock()
		return NewError(Unavailable, "the memory broker is pretending to be down")
	}
	a.mu.Unlock()
	a.broker.publish(destination, e, options)
	return nil
}

func (a *MemoryAdapter) Subscribe(_ context.Context, destination string, handler func(context.Context, Delivery) error, options map[string]any) error {
	group := "default"
	if g, ok := options["group"].(string); ok {
		group = g
	}
	guarded := func(ctx context.Context, d Delivery) error {
		a.mu.Lock()
		open := a.intake
		a.mu.Unlock()
		if !open {
			return fmt.Errorf("not taking new work")
		}
		return handler(ctx, d)
	}
	a.broker.mu.Lock()
	a.broker.subscribers[destination] = append(a.broker.subscribers[destination], memorySubscriber{group: group, handler: guarded, owner: a})
	a.broker.mu.Unlock()
	return nil
}
