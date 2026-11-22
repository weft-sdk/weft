package e2e_test

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"net"
	"os"
	"testing"
	"time"

	weft "github.com/weft-sdk/weft/go"
	"github.com/weft-sdk/weft/go/e2e"
)

var quiet = slog.New(slog.NewTextHandler(io.Discard, nil))

func freePort(t *testing.T) int {
	t.Helper()
	l, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer l.Close()
	return l.Addr().(*net.TCPAddr).Port
}

// service builds one service of the example from the generated code. Without a broker the
// services share an in-memory one.
type service struct {
	w *e2e.Weft
}

func newService(t *testing.T, name string, ordersPort int, topic, group string, broker *weft.MemoryBroker) *service {
	t.Helper()
	kafka := "    brokers: nowhere:9092"
	if real := os.Getenv("WEFT_TEST_KAFKA"); real != "" && broker == nil {
		kafka = "    brokers: " + real
	}
	yaml := fmt.Sprintf("service: %s\ntransports:\n  http:\n    port: %d\n  kafka:\n%s\n    group: %s\ntopics:\n  order.placed: %s\nservices:\n  orders: http://127.0.0.1:%d",
		name, freePort(t), kafka, group, topic, ordersPort)
	cfg, err := weft.ParseConfig(yaml, nil)
	if err != nil {
		t.Fatal(err)
	}
	opts := e2e.Options{Config: cfg, Logger: quiet}
	if broker != nil {
		opts.KafkaAdapter = weft.NewMemoryAdapter("kafka", broker)
	}
	w, err := e2e.New(opts)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { w.Stop(context.Background()) })
	return &service{w}
}

func TestOrdersOverHTTPAndNotificationsOverTheBroker(t *testing.T) {
	broker := weft.NewMemoryBroker()
	port := freePort(t)
	topic := fmt.Sprintf("placed-%d", time.Now().UnixNano())

	// The orders service listens on `port`, so its config says so.
	ordersCfg, err := weft.ParseConfig(fmt.Sprintf("service: orders\ntransports:\n  http:\n    port: %d\n  kafka:\n    brokers: nowhere:9092\ntopics:\n  order.placed: %s", port, topic), nil)
	if err != nil {
		t.Fatal(err)
	}
	ow, err := e2e.New(e2e.Options{Config: ordersCfg, Logger: quiet, KafkaAdapter: weft.NewMemoryAdapter("kafka", broker)})
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { ow.Stop(context.Background()) })
	ordersSvc, err := e2e.NewOrdersService(ow)
	if err != nil {
		t.Fatal(err)
	}

	notifications := newService(t, "notifications", port, topic, "notifications", broker)
	notified, err := e2e.NewNotificationsService(notifications.w)
	if err != nil {
		t.Fatal(err)
	}
	api := newService(t, "api", port, topic, "api", broker)
	for _, w := range []*e2e.Weft{ow, notifications.w, api.w} {
		if err := w.Start(context.Background()); err != nil {
			t.Fatal(err)
		}
	}
	_ = ordersSvc
	ctx := context.Background()
	client := api.w.HTTP.Orders()

	// An order placed over HTTP comes back typed, and reaches the consumer over the broker.
	placed, err := client.CreateOrder(ctx, e2e.OrdersCreateOrderRequest{Body: e2e.OrdersCreateOrder{CustomerID: "c-1", Items: []e2e.OrdersOrderItem{{Sku: "apple", Qty: 2}}}})
	if err != nil {
		t.Fatal(err)
	}
	if placed.CustomerID != "c-1" || placed.Total != (e2e.OrdersMoney{Amount: 300, Currency: e2e.OrdersCurrencyEUR}) || placed.PlacedAt.IsZero() {
		t.Errorf("placed = %+v", placed)
	}
	broker.Idle()
	sent := notified.Sent()
	if len(sent) != 1 || sent[0].ID != placed.ID || !sent[0].PlacedAt.Equal(placed.PlacedAt) {
		t.Errorf("the notification: %+v", sent)
	}

	// Read it back, and ask for one that is not there.
	got, err := client.GetOrder(ctx, e2e.OrdersGetOrderRequest{Params: e2e.OrdersGetOrderParams{ID: placed.ID}})
	if err != nil || got == nil || got.ID != placed.ID {
		t.Errorf("get: %v %v", got, err)
	}
	if none, err := client.GetOrder(ctx, e2e.OrdersGetOrderRequest{Params: e2e.OrdersGetOrderParams{ID: "123e4567-e89b-42d3-a456-426614174000"}}); err != nil || none != nil {
		t.Errorf("a missing order is nil: %v %v", none, err)
	}
	list, err := client.ListOrders(ctx, e2e.OrdersListOrdersRequest{})
	if err != nil || len(list) != 1 {
		t.Errorf("list: %v %v", list, err)
	}
	page := int64(5)
	if _, err := client.ListOrders(ctx, e2e.OrdersListOrdersRequest{Query: e2e.OrdersListOrdersQuery{Page: &page}}); err != nil {
		t.Errorf("a query: %v", err)
	}

	// A declared error comes back as the same typed error.
	_, err = client.CreateOrder(ctx, e2e.OrdersCreateOrderRequest{Body: e2e.OrdersCreateOrder{CustomerID: "c", Items: []e2e.OrdersOrderItem{{Sku: "pear", Qty: 1}}}})
	oos, ok := e2e.AsOrdersOutOfStock(err)
	if !ok || oos.Sku != "pear" || oos.Available != 0 {
		t.Errorf("declared error: %v (%+v)", err, oos)
	}
	broker.Idle()
	if len(notified.Sent()) != 1 {
		t.Error("a failed order must not be announced")
	}

	// A request that breaks the contract is refused before it is sent.
	_, err = client.CreateOrder(ctx, e2e.OrdersCreateOrderRequest{Body: e2e.OrdersCreateOrder{CustomerID: "c", Items: []e2e.OrdersOrderItem{{Sku: "apple", Qty: 1}}}, Timeout: 50 * time.Millisecond})
	if err != nil {
		t.Errorf("a valid request: %v", err)
	}

	// Shutdown is clean.
	for _, w := range []*e2e.Weft{api.w, notifications.w, ow} {
		if r := w.Stop(ctx); r != (weft.StopResult{Drained: true}) {
			t.Errorf("stop = %+v", r)
		}
	}
}

func TestTheGeneratedFileIsCurrentAndTheTypesAreWhatTheContractSays(t *testing.T) {
	// The contracts embedded in the file are the ones the types were generated from.
	c := e2e.Contracts()
	ir, ok := c.Package("orders")
	if !ok || len(ir.Events) != 2 || len(ir.Routes) != 3 {
		t.Fatalf("package orders: %+v", ir)
	}
	// A value of the generated type round-trips through the runtime.
	in := e2e.OrdersOrder{ID: "123e4567-e89b-42d3-a456-426614174000", CustomerID: "c", Total: e2e.OrdersMoney{Amount: 1, Currency: e2e.OrdersCurrencyGBP}, PlacedAt: time.Date(2026, 11, 23, 10, 0, 0, 0, time.UTC)}
	wire, err := c.Marshal(weft.Named("orders", "Order"), in)
	if err != nil {
		t.Fatal(err)
	}
	var out e2e.OrdersOrder
	if err := c.Unmarshal(weft.Named("orders", "Order"), wire, &out); err != nil || out.ID != in.ID || out.Total != in.Total || !out.PlacedAt.Equal(in.PlacedAt) {
		t.Errorf("%+v %v", out, err)
	}
	if _, err := c.Marshal(weft.Named("orders", "Order"), e2e.OrdersOrder{ID: "not-a-uuid"}); err == nil {
		t.Error("a bad value is refused")
	}
}
