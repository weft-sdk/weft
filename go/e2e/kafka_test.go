package e2e_test

import (
	"context"
	"fmt"
	"os"
	"sync/atomic"
	"testing"
	"time"

	"github.com/twmb/franz-go/pkg/kgo"
	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/propagation"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
	"go.opentelemetry.io/otel/sdk/trace/tracetest"
	"go.opentelemetry.io/otel/trace"

	weft "github.com/weft-sdk/weft/go"
	"github.com/weft-sdk/weft/go/e2e"
)

// The whole stack, from the generated code down: net/http, franz-go, a real Kafka broker. Needs one:
// WEFT_TEST_KAFKA=127.0.0.1:9092 (see `make ts-kafka-up`).

// larder starts the three services against the real broker: orders (listening), notifications, and an
// api that only calls orders.
func larder(t *testing.T) (orders *e2e.OrdersService, notifications *e2e.NotificationsService, api *e2e.Weft, topic string) {
	t.Helper()
	brokers := os.Getenv("WEFT_TEST_KAFKA")
	if brokers == "" {
		t.Skip("needs WEFT_TEST_KAFKA")
	}
	port := freePort(t)
	id := time.Now().UnixNano()
	topic = fmt.Sprintf("placed-%d", id)
	build := func(name string, listen int) *e2e.Weft {
		cfg, err := weft.ParseConfig(fmt.Sprintf("service: %s\ntransports:\n  http:\n    port: %d\n  kafka:\n    brokers: %s\n    group: %s-%d\ntopics:\n  order.placed: %s\nservices:\n  orders: http://127.0.0.1:%d",
			name, listen, brokers, name, id, topic, port), nil)
		if err != nil {
			t.Fatal(err)
		}
		w, err := e2e.New(e2e.Options{Config: cfg, Logger: quiet})
		if err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { w.Stop(context.Background()) })
		return w
	}
	ow, nw := build("orders", port), build("notifications", freePort(t))
	api = build("api", freePort(t))
	var err error
	if orders, err = e2e.NewOrdersService(ow); err != nil {
		t.Fatal(err)
	}
	if notifications, err = e2e.NewNotificationsService(nw); err != nil {
		t.Fatal(err)
	}
	for _, w := range []*e2e.Weft{ow, nw, api} {
		if err := w.Start(context.Background()); err != nil {
			t.Fatal(err)
		}
	}
	return orders, notifications, api, topic
}

func eventually(t *testing.T, what string, within time.Duration, cond func() bool) {
	t.Helper()
	for deadline := time.Now().Add(within); time.Now().Before(deadline); time.Sleep(50 * time.Millisecond) {
		if cond() {
			return
		}
	}
	t.Fatalf("timed out waiting for %s", what)
}

var item = []e2e.OrdersOrderItem{{Sku: "apple", Qty: 1}}

func TestAnOrderPlacedOverHTTPReachesTheConsumerThroughKafka(t *testing.T) {
	_, notifications, api, _ := larder(t)
	placed, err := api.HTTP.Orders().CreateOrder(context.Background(), e2e.OrdersCreateOrderRequest{Body: e2e.OrdersCreateOrder{CustomerID: "c-1", Items: item}})
	if err != nil {
		t.Fatal(err)
	}
	eventually(t, "the notification", 40*time.Second, func() bool { return len(notifications.Sent()) == 1 })
	if got := notifications.Sent()[0]; got.ID != placed.ID || got.CustomerID != "c-1" || !got.PlacedAt.Equal(placed.PlacedAt) {
		t.Errorf("sent %+v, placed %+v", got, placed)
	}
}

func TestOneTraceSpansHTTPAndKafka(t *testing.T) {
	recorder := tracetest.NewSpanRecorder()
	old, oldProp := otel.GetTracerProvider(), otel.GetTextMapPropagator()
	otel.SetTracerProvider(sdktrace.NewTracerProvider(sdktrace.WithSpanProcessor(recorder)))
	otel.SetTextMapPropagator(propagation.TraceContext{})
	t.Cleanup(func() { otel.SetTracerProvider(old); otel.SetTextMapPropagator(oldProp) })

	_, notifications, api, _ := larder(t)
	ctx, root := otel.Tracer("test").Start(context.Background(), "checkout")
	if _, err := api.HTTP.Orders().CreateOrder(ctx, e2e.OrdersCreateOrderRequest{Body: e2e.OrdersCreateOrder{CustomerID: "c", Items: item}}); err != nil {
		t.Fatal(err)
	}
	root.End()
	eventually(t, "the notification", 40*time.Second, func() bool { return len(notifications.Sent()) == 1 })
	time.Sleep(200 * time.Millisecond)

	kinds := map[string]bool{}
	for _, s := range recorder.Ended() {
		if s.SpanContext().TraceID() == root.SpanContext().TraceID() {
			kinds[s.SpanKind().String()] = true
		}
	}
	for _, want := range []trace.SpanKind{trace.SpanKindClient, trace.SpanKindServer, trace.SpanKindProducer, trace.SpanKindConsumer} {
		if !kinds[want.String()] {
			t.Errorf("no %s span in the trace: %v", want, kinds)
		}
	}
}

func TestAConsumerThatKeepsFailingDeadLettersTheOrder(t *testing.T) {
	_, notifications, api, topic := larder(t)
	var failing atomic.Bool
	failing.Store(true)
	notifications.Fail = failing.Load

	if _, err := api.HTTP.Orders().CreateOrder(context.Background(), e2e.OrdersCreateOrderRequest{Body: e2e.OrdersCreateOrder{CustomerID: "c", Items: item}}); err != nil {
		t.Fatal(err)
	}
	client, err := kgo.NewClient(kgo.SeedBrokers(os.Getenv("WEFT_TEST_KAFKA")), kgo.ConsumeTopics(topic+".dlq"), kgo.ConsumeResetOffset(kgo.NewOffset().AtStart()))
	if err != nil {
		t.Fatal(err)
	}
	defer client.Close()
	var rec *kgo.Record
	eventually(t, "the dead letter", 90*time.Second, func() bool {
		ctx, cancel := context.WithTimeout(context.Background(), time.Second)
		defer cancel()
		client.PollFetches(ctx).EachRecord(func(r *kgo.Record) { rec = r })
		return rec != nil
	})
	headers := map[string]string{}
	for _, h := range rec.Headers {
		headers[h.Key] = string(h.Value)
	}
	if headers["weft-error-category"] != "Internal" || headers["weft-attempts"] != "5" || headers["weft-error-message"] != "the mail server is down" {
		t.Errorf("headers = %v", headers)
	}
	if len(notifications.Sent()) != 0 {
		t.Error("nothing was sent")
	}
}

func TestShutdownIsClean(t *testing.T) {
	orders, notifications, api, _ := larder(t)
	if _, err := api.HTTP.Orders().CreateOrder(context.Background(), e2e.OrdersCreateOrderRequest{Body: e2e.OrdersCreateOrder{CustomerID: "c", Items: item}}); err != nil {
		t.Fatal(err)
	}
	eventually(t, "the notification", 40*time.Second, func() bool { return len(notifications.Sent()) == 1 })
	for name, w := range map[string]*e2e.Weft{"api": api, "orders": orders.W} {
		if r := w.Stop(context.Background()); r != (weft.StopResult{Drained: true}) {
			t.Errorf("%s: %+v", name, r)
		}
	}
}
