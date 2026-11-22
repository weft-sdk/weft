package weft

import (
	"context"
	"crypto/rand"
	"encoding/json"
	"fmt"
	"log/slog"
	"strings"
	"time"
)

// MessagingCapabilities says what a library can do. Core rejects an option the adapter cannot
// honour, with a clear error.
type MessagingCapabilities struct {
	Ordering, Keys, Headers, Transactions, RequestReply, Delay, ConsumerGroups bool
}

// Delivery is what an adapter hands to core for one message.
type Delivery struct {
	Envelope Envelope
	// DeliveryCount is how many times the broker has delivered this message, when it knows.
	DeliveryCount int
}

// MessagingAdapter is the messaging library underneath, as core needs it. Adapters only translate:
// encoding, envelopes, validation, middleware, retries and dead letters are core's. A handler that
// returns nil means "acknowledge"; an error means "do not acknowledge" (shutdown, or a crash), and the
// broker will deliver the message again.
type MessagingAdapter interface {
	Transport
	Raw() any
	Capabilities() MessagingCapabilities
	Publish(ctx context.Context, destination string, e Envelope, options map[string]any) error
	Subscribe(ctx context.Context, destination string, handler func(context.Context, Delivery) error, options map[string]any) error
}

// AdapterConfigurer is implemented by adapters that read their settings from weft.yaml.
type AdapterConfigurer interface {
	Configure(settings map[string]any, config *Config)
}

// AdapterReadier is implemented by adapters that must know every subscription before they start consuming.
type AdapterReadier interface {
	Ready(ctx context.Context) error
}

// MessageContext is what a messaging handler sees besides the payload.
type MessageContext struct {
	*Call
	Event    string
	Envelope Envelope
	Key      string
	// Attempt is 1 for the first try, then 2, and so on.
	Attempt   int
	Timestamp time.Time
}

// MessageHandler handles one event. payload is in code form (see Contracts).
type MessageHandler func(ctx context.Context, payload any, mc *MessageContext) error

// MessageMiddleware wraps a messaging handler.
type MessageMiddleware func(ctx context.Context, mc *MessageContext, next Next) error

// PublishOptions are the options of Publish.
type PublishOptions struct {
	// Key overrides the key taken from the event's key:.
	Key     string
	Headers map[string]string
	// IdempotencyKey sets weft-idempotency-key, which Dedupe uses on the receiving side.
	IdempotencyKey string
	// Delay delays delivery. Needs the Delay capability.
	Delay time.Duration
	// Package is for the case where two packages declare the same event name.
	Package string
	// Extra holds options of the adapter's own.
	Extra map[string]any
}

// SubscribeOptions are the options of On.
type SubscribeOptions struct {
	// Group is the consumer group. Needs the ConsumerGroups capability.
	Group         string
	FromBeginning bool
	// Attempts before the message is dead-lettered. Default 5.
	Attempts int
	// DeadLetter is where failed messages go. Default <destination>.dlq.
	DeadLetter string
	// NoDeadLetter drops failed messages instead (with a log line).
	NoDeadLetter bool
	Backoff      Backoff
	Package      string
	// Extra holds options of the adapter's own.
	Extra map[string]any
}

// MessagingOptions configures a Messaging.
type MessagingOptions struct {
	Adapter MessagingAdapter
	// Name is the transport's name in weft.yaml and in contexts, such as kafka. Default: the adapter's.
	Name   string
	Logger *slog.Logger
	// Sleep waits between attempts; for tests. Default: Sleep.
	Sleep func(ctx context.Context, d time.Duration)
	// Random is the source of jitter; for tests. Default: a random number in [0, 1).
	Random func() float64
}

type subscription struct {
	event   string
	pkg     string
	def     *EventDef
	options SubscribeOptions
	handler MessageHandler
}

// Messaging is messaging over one adapter: Publish encodes and sends an event, On receives it.
// Everything the libraries have in common is here: encoding, CloudEvents envelopes, validation,
// middleware, retries with backoff, dead letters. It is a transport of the Weft it is built on.
type Messaging struct {
	weft    *Weft
	adapter MessagingAdapter
	name    string
	logger  *slog.Logger
	sleep   func(ctx context.Context, d time.Duration)
	random  func() float64

	subs       []subscription
	middleware []MessageMiddleware
	stopCtx    context.Context
	stop       context.CancelFunc
	started    bool
}

// NewMessaging makes a Messaging and registers it with w.
func NewMessaging(w *Weft, o MessagingOptions) (*Messaging, error) {
	m := &Messaging{weft: w, adapter: o.Adapter, name: o.Name, logger: o.Logger, sleep: o.Sleep, random: o.Random}
	if m.name == "" {
		m.name = o.Adapter.Name()
	}
	if m.logger == nil {
		m.logger = slog.Default()
	}
	if m.sleep == nil {
		m.sleep = Sleep
	}
	if m.random == nil {
		m.random = randomFloat
	}
	m.stopCtx, m.stop = context.WithCancel(context.Background())
	if c, ok := o.Adapter.(AdapterConfigurer); ok {
		c.Configure(w.Config.Transports[m.name], w.Config)
	}
	if err := w.Register(m); err != nil {
		return nil, err
	}
	return m, nil
}

func randomFloat() float64 {
	var b [8]byte
	_, _ = rand.Read(b[:])
	return float64(uint64(b[0])<<48|uint64(b[1])<<40|uint64(b[2])<<32|uint64(b[3])<<24|uint64(b[4])<<16|uint64(b[5])<<8|uint64(b[6])) / float64(uint64(1)<<56)
}

// Raw is the library underneath. Weft's tracing and middleware do not apply to calls made through it.
func (m *Messaging) Raw() any { return m.adapter.Raw() }

// Use adds middleware for this transport only. It runs after the middleware added to Weft.
func (m *Messaging) Use(mw MessageMiddleware) { m.middleware = append(m.middleware, mw) }

// Event finds an event in the contracts by name.
func (c *Contracts) Event(name, pkg string) (*Ir, *EventDef, error) {
	var found []*EventDef
	var in []*Ir
	for i := range c.packages {
		if pkg != "" && c.packages[i].Package != pkg {
			continue
		}
		for j := range c.packages[i].Events {
			if c.packages[i].Events[j].Name == name {
				found = append(found, &c.packages[i].Events[j])
				in = append(in, &c.packages[i])
			}
		}
	}
	switch len(found) {
	case 0:
		return nil, nil, errorf(Internal, "The contracts have no event %s", name)
	case 1:
		return in[0], found[0], nil
	}
	return nil, nil, errorf(Internal, "The event %s is in more than one package: pass the package", name)
}

// On receives an event. Register before Weft.Start.
func (m *Messaging) On(event string, handler MessageHandler, o SubscribeOptions) error {
	if m.started {
		return errorf(Internal, "Register the handler for %s before Start()", event)
	}
	ir, def, err := m.weft.Contracts.Event(event, o.Package)
	if err != nil {
		return err
	}
	if o.Group != "" && !m.adapter.Capabilities().ConsumerGroups {
		return errorf(Internal, "%s does not support consumer groups, so the group option is not allowed", m.name)
	}
	m.subs = append(m.subs, subscription{event: event, pkg: ir.Package, def: def, options: o, handler: handler})
	return nil
}

// Publish validates, encodes and sends an event. It retries Unavailable and Timeout failures with backoff.
func (m *Messaging) Publish(ctx context.Context, event string, payload any, o PublishOptions) error {
	if s := m.weft.State(); s != Running && s != Stopping {
		return NewError(Unavailable, "Weft is not running: call Start() before publishing")
	}
	_, def, err := m.weft.Contracts.Event(event, o.Package)
	if err != nil {
		return err
	}
	caps := m.adapter.Capabilities()
	if o.Delay > 0 && !caps.Delay {
		return errorf(Internal, "%s does not support delayed delivery (Delay)", m.name)
	}
	if len(o.Headers) > 0 && !caps.Headers {
		return errorf(Internal, "%s does not support message headers", m.name)
	}
	wire, err := m.weft.Contracts.Encode(def.Payload, payload)
	if err != nil {
		return err
	}
	body, err := json.Marshal(wire)
	if err != nil {
		return &Error{Category: Internal, Message: "the payload cannot be written as JSON", Cause: err}
	}
	key := o.Key
	if key == "" {
		key = keyOf(def.Key, payload)
	}
	id := newID()
	destination := m.weft.Config.TopicName(event)
	options := map[string]any{}
	for k, v := range o.Extra {
		options[k] = v
	}
	if o.Delay > 0 {
		options["delay"] = o.Delay
	}
	return m.instrumentedPublish(ctx, destination, event, id, len(body), func(ctx context.Context) error {
		headers := map[string]string{}
		for k, v := range o.Headers {
			headers[k] = v
		}
		if o.IdempotencyKey != "" {
			headers[IdempotencyHeader] = o.IdempotencyKey
		}
		injectTraceContext(ctx, headers)
		return m.send(ctx, destination, Envelope{
			ID: id, Type: event, Source: m.weft.Config.Service, Time: time.Now().UTC().Format("2006-01-02T15:04:05.000Z"),
			DataContentType: "application/json", Key: key, Headers: headers, Body: body,
		}, options)
	})
}

func keyOf(fields []string, payload any) string {
	rec, ok := payload.(map[string]any)
	if !ok || len(fields) == 0 {
		return ""
	}
	parts := make([]string, 0, len(fields))
	for _, f := range fields {
		v, ok := rec[f]
		if !ok || v == nil {
			return ""
		}
		if t, isTime := v.(time.Time); isTime {
			parts = append(parts, t.UTC().Format("2006-01-02T15:04:05.000Z"))
		} else {
			parts = append(parts, fmt.Sprint(v))
		}
	}
	return strings.Join(parts, ":")
}

func newID() string {
	var b [16]byte
	_, _ = rand.Read(b[:])
	b[6] = b[6]&0x0f | 0x40
	b[8] = b[8]&0x3f | 0x80
	return fmt.Sprintf("%x-%x-%x-%x-%x", b[0:4], b[4:6], b[6:8], b[8:10], b[10:])
}

// ---- Transport ----

func (m *Messaging) Name() string { return m.name }

func (m *Messaging) Start(ctx context.Context) error {
	if err := m.adapter.Start(ctx); err != nil {
		return err
	}
	for i := range m.subs {
		sub := &m.subs[i]
		destination := m.weft.Config.TopicName(sub.event)
		options := map[string]any{}
		for k, v := range sub.options.Extra {
			options[k] = v
		}
		if sub.options.Group != "" {
			options["group"] = sub.options.Group
		}
		if sub.options.FromBeginning {
			options["fromBeginning"] = true
		}
		err := m.adapter.Subscribe(ctx, destination, func(ctx context.Context, d Delivery) error {
			return m.receive(ctx, sub, destination, d)
		}, options)
		if err != nil {
			return err
		}
	}
	if r, ok := m.adapter.(AdapterReadier); ok {
		if err := r.Ready(ctx); err != nil {
			return err
		}
	}
	m.started = true
	return nil
}

func (m *Messaging) StopIntake(ctx context.Context) error {
	m.stop()
	return m.adapter.StopIntake(ctx)
}

func (m *Messaging) Close(ctx context.Context) error { return m.adapter.Close(ctx) }

// ---- internals ----

func (m *Messaging) stopping() bool {
	if m.stopCtx.Err() != nil {
		return true
	}
	s := m.weft.State()
	return s == Stopping || s == Stopped
}

func (m *Messaging) send(ctx context.Context, destination string, e Envelope, options map[string]any) error {
	for attempt := 1; ; attempt++ {
		err := m.adapter.Publish(ctx, destination, e, options)
		if err == nil {
			return nil
		}
		we := AsError(err)
		if _, isWeft := err.(*Error); !isWeft {
			we = &Error{Category: Unavailable, Message: fmt.Sprintf("%s could not send to %s: %v", m.name, destination, err), Cause: err}
		}
		retryable := we.Category == Unavailable || we.Category == Timeout
		if !retryable || attempt >= MessagingAttempts {
			return we
		}
		m.sleep(m.stopCtx, BackoffDelay(attempt, Backoff{}, m.random))
		if m.stopCtx.Err() != nil && m.weft.State() != Running {
			return we
		}
	}
}

func (m *Messaging) receive(ctx context.Context, sub *subscription, destination string, d Delivery) error {
	e := d.Envelope
	max := sub.options.Attempts
	if max == 0 {
		max = MessagingAttempts
	}
	wire, err := ParseJSON(e.Body)
	var payload any
	if err == nil {
		payload, err = m.weft.Contracts.Decode(sub.def.Payload, wire)
	}
	if err != nil {
		we := AsError(err)
		if _, isWeft := err.(*Error); !isWeft {
			we = &Error{Category: InvalidInput, Message: "Invalid input: the message body is not valid JSON", Cause: err}
		}
		return m.deadLetter(ctx, sub, destination, e, we, 1)
	}

	for attempt := 1; ; attempt++ {
		mc := m.context(sub, e, attempt)
		err := m.process(ctx, sub, destination, mc, payload)
		if err == nil {
			return nil
		}
		we := AsError(err)
		// Shutting down: do not acknowledge, so the broker delivers it again after a restart.
		if m.stopping() {
			recordReceived(sub.event, m.name, "not_acked")
			return &Error{Category: Unavailable, Message: "Shutting down before the message was handled", Cause: err}
		}
		switch we.MessagingPolicy() {
		case PolicyNone:
			recordReceived(sub.event, m.name, "dropped")
			m.logger.Warn(sub.event+" handler failed; not retried", "category", string(we.Category), "id", e.ID, "message", we.Message)
			return nil
		case PolicyDeadLetter:
			recordReceived(sub.event, m.name, "dead_letter")
			return m.deadLetter(ctx, sub, destination, e, we, attempt)
		}
		if attempt >= max {
			recordReceived(sub.event, m.name, "dead_letter")
			return m.deadLetter(ctx, sub, destination, e, we, attempt)
		}
		recordReceived(sub.event, m.name, "retry")
		m.sleep(m.stopCtx, BackoffDelay(attempt, sub.options.Backoff, m.random))
		if m.stopping() {
			return &Error{Category: Unavailable, Message: "Shutting down before the message was handled", Cause: err}
		}
	}
}

// process runs one attempt: the middleware, then the handler, inside a consumer span.
func (m *Messaging) process(ctx context.Context, sub *subscription, destination string, mc *MessageContext, payload any) error {
	chain := make([]Middleware, 0, len(m.middleware))
	for _, mw := range m.middleware {
		mw := mw
		chain = append(chain, func(ctx context.Context, _ *Call, next Next) error { return mw(ctx, mc, next) })
	}
	return m.instrumentedProcess(ctx, destination, mc, func(ctx context.Context) error {
		return m.weft.Dispatch(ctx, mc.Call, func(ctx context.Context, _ *Call) error {
			return sub.handler(ctx, payload, mc)
		}, chain...)
	})
}

func (m *Messaging) deadLetter(ctx context.Context, sub *subscription, destination string, e Envelope, err *Error, attempts int) error {
	if sub.options.NoDeadLetter {
		m.logger.Error(sub.event+" message dropped", "attempts", attempts, "message", err.Message, "id", e.ID)
		return nil
	}
	target := sub.options.DeadLetter
	if target == "" {
		target = destination + ".dlq"
	}
	headers := map[string]string{}
	for k, v := range e.Headers {
		headers[k] = v
	}
	headers["weft-original-destination"] = destination
	headers["weft-error-category"] = string(err.Category)
	headers["weft-error-message"] = err.Message
	headers["weft-attempts"] = fmt.Sprint(attempts)
	dead := e
	dead.Headers = headers
	m.logger.Error(sub.event+" message sent to "+target, "attempts", attempts, "message", err.Message, "id", e.ID)
	return m.instrumentedDeadLetter(ctx, target, e, sub.event, func(ctx context.Context) error {
		return m.send(ctx, target, dead, map[string]any{})
	})
}

func (m *Messaging) context(sub *subscription, e Envelope, attempt int) *MessageContext {
	call := NewCall(m.name, sub.event)
	for k, v := range e.Headers {
		call.Headers[k] = v
	}
	t, _ := time.Parse(time.RFC3339, e.Time)
	return &MessageContext{Call: call, Event: sub.event, Envelope: e, Key: e.Key, Attempt: attempt, Timestamp: t}
}
