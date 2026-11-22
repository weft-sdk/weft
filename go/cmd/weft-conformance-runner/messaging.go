package main

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"log/slog"
	"sync"
	"time"

	weft "github.com/weft-sdk/weft/go"
)

// messagingRequest is the `messaging` operation: one scripted scenario on an in-memory broker, with a
// fixed jitter and a recorded sleep, so every language can be held to the same retries, dead letters
// and envelope.
type messagingRequest struct {
	Event   string          `json:"event"`
	Payload json.RawMessage `json:"payload"`
	RawBody *string         `json:"rawBody"`
	Topic   string          `json:"topic"`
	Handler struct {
		Fail *struct {
			Times    json.RawMessage `json:"times"`
			Category string          `json:"category"`
		} `json:"fail"`
	} `json:"handler"`
	Subscribe struct {
		Attempts   int             `json:"attempts"`
		DeadLetter json.RawMessage `json:"deadLetter"`
	} `json:"subscribe"`
	Publish struct {
		Key            string            `json:"key"`
		IdempotencyKey string            `json:"idempotencyKey"`
		Headers        map[string]string `json:"headers"`
		DelayMs        int               `json:"delayMs"`
		Times          int               `json:"times"`
	} `json:"publish"`
	Dedupe            bool `json:"dedupe"`
	StopDuringBackoff bool `json:"stopDuringBackoff"`
}

func failure(category string) error {
	if category == "Plain" {
		return errors.New("a plain exception")
	}
	return weft.NewError(weft.Category(category), "failed with "+category)
}

func messaging(contracts *weft.Contracts, req messagingRequest) (map[string]any, error) {
	yaml := "service: conformance\n"
	if req.Topic != "" {
		yaml += "topics:\n  " + req.Event + ": " + req.Topic + "\n"
	}
	cfg, err := weft.ParseConfig(yaml, nil)
	if err != nil {
		return nil, err
	}
	w := weft.New(weft.Options{Contracts: contracts, Config: cfg, ShutdownGrace: 2 * time.Second})
	adapter := weft.NewMemoryAdapter("kafka", nil)
	broker := adapter.Broker()

	var mu sync.Mutex
	var sleeps []int64
	m, err := weft.NewMessaging(w, weft.MessagingOptions{
		Adapter: adapter,
		Logger:  slog.New(slog.NewTextHandler(io.Discard, nil)),
		Random:  func() float64 { return 0.5 },
		Sleep: func(ctx context.Context, d time.Duration) {
			mu.Lock()
			sleeps = append(sleeps, d.Milliseconds())
			mu.Unlock()
			if req.StopDuringBackoff {
				<-ctx.Done()
			}
		},
	})
	if err != nil {
		return nil, err
	}
	if req.Dedupe {
		m.Use(weft.Dedupe(0))
	}

	_, def, err := contracts.Event(req.Event, "")
	if err != nil {
		return nil, err
	}
	var attempts []int
	var received any
	failures := 0
	sub := weft.SubscribeOptions{Attempts: req.Subscribe.Attempts}
	if dl := req.Subscribe.DeadLetter; len(dl) > 0 {
		var name string
		if json.Unmarshal(dl, &name) == nil {
			sub.DeadLetter = name
		} else {
			sub.NoDeadLetter = true // `false`
		}
	}
	err = m.On(req.Event, func(ctx context.Context, payload any, mc *weft.MessageContext) error {
		mu.Lock()
		attempts = append(attempts, mc.Attempt)
		mu.Unlock()
		if f := req.Handler.Fail; f != nil {
			always := string(f.Times) == `"always"`
			var times int
			_ = json.Unmarshal(f.Times, &times)
			if always || failures < times {
				failures++
				return failure(f.Category)
			}
		}
		wire, err := contracts.Encode(def.Payload, payload)
		if err != nil {
			return err
		}
		mu.Lock()
		received = wire
		mu.Unlock()
		return nil
	}, sub)
	if err != nil {
		return nil, err
	}
	ctx := context.Background()
	if err := w.Start(ctx); err != nil {
		return nil, err
	}

	destination := req.Event
	if req.Topic != "" {
		destination = req.Topic
	}
	var publishError any
	if req.RawBody != nil {
		_ = adapter.Publish(ctx, destination, weft.Envelope{ID: "raw-1", Type: req.Event, Source: "elsewhere", Time: time.Now().UTC().Format(time.RFC3339), DataContentType: "application/json", Headers: map[string]string{}, Body: []byte(*req.RawBody)}, nil)
		broker.Idle()
	} else {
		wire, err := weft.ParseJSON(req.Payload)
		if err != nil {
			return nil, err
		}
		payload, err := contracts.Decode(def.Payload, wire)
		if err != nil {
			publishError = "InvalidInput"
		} else {
			times := req.Publish.Times
			if times == 0 {
				times = 1
			}
			for i := 0; i < times; i++ {
				err := m.Publish(ctx, req.Event, payload, weft.PublishOptions{
					Key: req.Publish.Key, IdempotencyKey: req.Publish.IdempotencyKey, Headers: req.Publish.Headers,
					Delay: time.Duration(req.Publish.DelayMs) * time.Millisecond,
				})
				if err != nil {
					publishError = string(weft.AsError(err).Category)
					break
				}
				if req.StopDuringBackoff {
					for wait := 0; wait < 400; wait++ {
						mu.Lock()
						n := len(attempts)
						mu.Unlock()
						if n >= 1 {
							break
						}
						time.Sleep(5 * time.Millisecond)
					}
				} else {
					broker.Idle()
				}
			}
		}
	}
	if req.StopDuringBackoff {
		w.Stop(ctx)
	}
	broker.Idle()
	if !req.StopDuringBackoff {
		w.Stop(ctx)
	}

	var sent *weft.Envelope
	var destinations []string
	dlq := []map[string]any{}
	for _, p := range broker.Published() {
		destinations = append(destinations, p.Destination)
		if sent == nil && p.Destination == destination && p.Envelope.ID != "raw-1" {
			e := p.Envelope
			sent = &e
		}
	}
	for _, p := range broker.Published() {
		if cat, ok := p.Envelope.Headers["weft-error-category"]; ok {
			dlq = append(dlq, map[string]any{
				"destination": p.Destination, "category": cat,
				"attempts":            p.Envelope.Headers["weft-attempts"],
				"originalDestination": p.Envelope.Headers["weft-original-destination"],
				"sameId":              p.Envelope.ID == "raw-1" || (sent != nil && p.Envelope.ID == sent.ID),
			})
		}
	}
	var envelope any
	if sent != nil {
		var key any
		if sent.Key != "" {
			key = sent.Key
		}
		_, perr := time.Parse(time.RFC3339, sent.Time)
		envelope = map[string]any{
			"type": sent.Type, "source": sent.Source, "datacontenttype": sent.DataContentType,
			"key": key, "hasId": sent.ID != "", "hasTime": perr == nil, "headers": sent.Headers,
		}
	}
	mu.Lock()
	defer mu.Unlock()
	if attempts == nil {
		attempts = []int{}
	}
	if sleeps == nil {
		sleeps = []int64{}
	}
	if destinations == nil {
		destinations = []string{}
	}
	return map[string]any{
		"publishError": publishError, "calls": len(attempts), "attempts": attempts, "sleeps": sleeps,
		"received": received, "destinations": destinations, "envelope": envelope, "dlq": dlq,
		"unacked": len(broker.Unacked()),
	}, nil
}
