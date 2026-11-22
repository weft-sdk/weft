package franz_test

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"os"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/twmb/franz-go/pkg/kadm"
	"github.com/twmb/franz-go/pkg/kgo"

	weft "github.com/weft-sdk/weft/go"
	"github.com/weft-sdk/weft/go/kafka/franz"
)

// The integration tests need a broker: WEFT_TEST_KAFKA=127.0.0.1:9092 (see `make ts-kafka-up`).
var brokers = os.Getenv("WEFT_TEST_KAFKA")

const id = "123e4567-e89b-42d3-a456-426614174000"

func field(name, t string) weft.FieldDef {
	return weft.FieldDef{Name: name, WireName: name, Type: weft.Primitive(t)}
}

var contracts = weft.NewContracts(weft.Ir{
	IrVersion: "0.1", Package: "t",
	Types:  []weft.TypeDef{{Name: "Order", Kind: "record", Fields: []weft.FieldDef{field("id", "uuid"), field("customerId", "string")}}},
	Events: []weft.EventDef{{Name: "order.placed", Payload: weft.Named("t", "Order"), Key: []string{"id"}}},
})

var order = map[string]any{"id": id, "customerId": "c1"}

var quiet = slog.New(slog.NewTextHandler(io.Discard, nil))

func unique(prefix string) string { return fmt.Sprintf("%s-%d", prefix, time.Now().UnixNano()) }

type node struct {
	weft    *weft.Weft
	m       *weft.Messaging
	adapter *franz.Adapter
}

// newNode makes a service that uses the topic and group it is given, so tests never see each other's messages.
func newNode(t *testing.T, topic, group string, sleep func(context.Context, time.Duration)) *node {
	t.Helper()
	yaml := fmt.Sprintf("service: orders-svc\ntransports:\n  kafka:\n    brokers: %s\n    group: %s\ntopics:\n  order.placed: %s", brokers, group, topic)
	cfg, err := weft.ParseConfig(yaml, nil)
	if err != nil {
		t.Fatal(err)
	}
	w := weft.New(weft.Options{Contracts: contracts, Config: cfg, ShutdownGrace: 5 * time.Second})
	a := franz.New(franz.Options{})
	if sleep == nil {
		sleep = func(context.Context, time.Duration) {}
	}
	m, err := weft.NewMessaging(w, weft.MessagingOptions{Adapter: a, Logger: quiet, Random: func() float64 { return 0.5 }, Sleep: sleep})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { w.Stop(context.Background()) })
	return &node{w, m, a}
}

func eventually(t *testing.T, what string, within time.Duration, cond func() bool) {
	t.Helper()
	deadline := time.Now().Add(within)
	for time.Now().Before(deadline) {
		if cond() {
			return
		}
		time.Sleep(50 * time.Millisecond)
	}
	t.Fatalf("timed out waiting for %s", what)
}

func TestWithoutBrokersItSaysWhichSettingIsMissing(t *testing.T) {
	cfg, _ := weft.ParseConfig("service: s", nil)
	w := weft.New(weft.Options{Contracts: contracts, Config: cfg})
	if _, err := weft.NewMessaging(w, weft.MessagingOptions{Adapter: franz.New(franz.Options{})}); err != nil {
		t.Fatal(err)
	}
	err := w.Start(context.Background())
	if err == nil || !strings.Contains(err.Error(), "no Kafka brokers") || !strings.Contains(err.Error(), "transports.kafka") {
		t.Errorf("err = %v", err)
	}
}

func TestPublishAndConsumeThroughTheContract(t *testing.T) {
	if brokers == "" {
		t.Skip("needs WEFT_TEST_KAFKA")
	}
	topic := unique("placed")
	n := newNode(t, topic, unique("g"), nil)
	var mu sync.Mutex
	var got []*weft.MessageContext
	var payloads []any
	_ = n.m.On("order.placed", func(_ context.Context, p any, mc *weft.MessageContext) error {
		mu.Lock()
		got, payloads = append(got, mc), append(payloads, p)
		mu.Unlock()
		return nil
	}, weft.SubscribeOptions{FromBeginning: true})
	if err := n.weft.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	if err := n.m.Publish(context.Background(), "order.placed", order, weft.PublishOptions{Headers: map[string]string{"tenant": "t1"}}); err != nil {
		t.Fatal(err)
	}
	eventually(t, "the message", 30*time.Second, func() bool { mu.Lock(); defer mu.Unlock(); return len(got) == 1 })
	mu.Lock()
	defer mu.Unlock()
	if payloads[0].(map[string]any)["customerId"] != "c1" {
		t.Errorf("payload = %v", payloads[0])
	}
	mc := got[0]
	if mc.Key != id || mc.Envelope.Type != "order.placed" || mc.Envelope.Source != "orders-svc" || mc.Envelope.Headers["tenant"] != "t1" || mc.Attempt != 1 {
		t.Errorf("context = %+v", mc)
	}
}

func TestEachConsumerGroupGetsEveryMessageOnce(t *testing.T) {
	if brokers == "" {
		t.Skip("needs WEFT_TEST_KAFKA")
	}
	topic, group := unique("placed"), unique("g")
	var mu sync.Mutex
	counts := map[string]int{}
	record := func(who string) weft.MessageHandler {
		return func(context.Context, any, *weft.MessageContext) error {
			mu.Lock()
			counts[who]++
			mu.Unlock()
			return nil
		}
	}
	a, b := newNode(t, topic, group, nil), newNode(t, topic, group, nil)
	other := newNode(t, topic, group+"-other", nil)
	_ = a.m.On("order.placed", record("a"), weft.SubscribeOptions{FromBeginning: true})
	_ = b.m.On("order.placed", record("b"), weft.SubscribeOptions{FromBeginning: true})
	_ = other.m.On("order.placed", record("other"), weft.SubscribeOptions{FromBeginning: true})
	for _, n := range []*node{a, b, other} {
		if err := n.weft.Start(context.Background()); err != nil {
			t.Fatal(err)
		}
	}
	for i := 0; i < 4; i++ {
		o := map[string]any{"id": id, "customerId": fmt.Sprint("c", i)}
		if err := a.m.Publish(context.Background(), "order.placed", o, weft.PublishOptions{Key: fmt.Sprint(i)}); err != nil {
			t.Fatal(err)
		}
	}
	eventually(t, "every message in both groups", 40*time.Second, func() bool {
		mu.Lock()
		defer mu.Unlock()
		return counts["other"] == 4 && counts["a"]+counts["b"] == 4
	})
}

func TestAMessageThatKeepsFailingIsDeadLetteredWithItsReason(t *testing.T) {
	if brokers == "" {
		t.Skip("needs WEFT_TEST_KAFKA")
	}
	topic := unique("placed")
	n := newNode(t, topic, unique("g"), nil)
	var calls atomic.Int32
	_ = n.m.On("order.placed", func(context.Context, any, *weft.MessageContext) error {
		calls.Add(1)
		return weft.NewError(weft.Unavailable, "down")
	}, weft.SubscribeOptions{FromBeginning: true, Attempts: 3})
	if err := n.weft.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	if err := n.m.Publish(context.Background(), "order.placed", order, weft.PublishOptions{}); err != nil {
		t.Fatal(err)
	}

	// Read the dead-letter topic with a client of our own.
	client, err := kgo.NewClient(kgo.SeedBrokers(brokers), kgo.ConsumeTopics(topic+".dlq"), kgo.ConsumeResetOffset(kgo.NewOffset().AtStart()))
	if err != nil {
		t.Fatal(err)
	}
	defer client.Close()
	var rec *kgo.Record
	eventually(t, "the dead letter", 60*time.Second, func() bool {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		fetches := client.PollFetches(ctx)
		fetches.EachRecord(func(r *kgo.Record) { rec = r })
		return rec != nil
	})
	headers := map[string]string{}
	for _, h := range rec.Headers {
		headers[h.Key] = string(h.Value)
	}
	if calls.Load() != 3 || headers["weft-error-category"] != "Unavailable" || headers["weft-attempts"] != "3" ||
		headers["weft-original-destination"] != topic || headers["ce_type"] != "order.placed" {
		t.Errorf("calls %d, headers %v", calls.Load(), headers)
	}
}

func TestAMessageBeingRetriedAtShutdownIsDeliveredToTheNextConsumer(t *testing.T) {
	if brokers == "" {
		t.Skip("needs WEFT_TEST_KAFKA")
	}
	topic, group := unique("placed"), unique("g")
	first := newNode(t, topic, group, func(ctx context.Context, _ time.Duration) { <-ctx.Done() })
	var tries atomic.Int32
	_ = first.m.On("order.placed", func(context.Context, any, *weft.MessageContext) error {
		tries.Add(1)
		return weft.NewError(weft.Unavailable, "down")
	}, weft.SubscribeOptions{FromBeginning: true})
	if err := first.weft.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	if err := first.m.Publish(context.Background(), "order.placed", order, weft.PublishOptions{}); err != nil {
		t.Fatal(err)
	}
	eventually(t, "the first attempt", 40*time.Second, func() bool { return tries.Load() >= 1 })
	first.weft.Stop(context.Background())

	second := newNode(t, topic, group, nil)
	var mu sync.Mutex
	var received []any
	_ = second.m.On("order.placed", func(_ context.Context, p any, _ *weft.MessageContext) error {
		mu.Lock()
		received = append(received, p)
		mu.Unlock()
		return nil
	}, weft.SubscribeOptions{FromBeginning: true})
	if err := second.weft.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	eventually(t, "the redelivery", 40*time.Second, func() bool { mu.Lock(); defer mu.Unlock(); return len(received) == 1 })
}

func TestTheClientIsTheRawObjectAndTopicsAreCreatedFirst(t *testing.T) {
	if brokers == "" {
		t.Skip("needs WEFT_TEST_KAFKA")
	}
	topic := unique("placed")
	n := newNode(t, topic, unique("g"), nil)
	_ = n.m.On("order.placed", func(context.Context, any, *weft.MessageContext) error { return nil }, weft.SubscribeOptions{})
	if err := n.weft.Start(context.Background()); err != nil {
		t.Fatal(err)
	}
	client, ok := n.m.Raw().(*kgo.Client)
	if !ok {
		t.Fatalf("raw = %T", n.m.Raw())
	}
	details, err := kadm.NewClient(client).ListTopics(context.Background(), topic)
	if err != nil || !details.Has(topic) {
		t.Errorf("the consumed topic should exist: %v %v", details, err)
	}
}
