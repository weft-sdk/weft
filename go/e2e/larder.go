package e2e

import (
	"context"
	"sync"
	"time"

	weft "github.com/weft-sdk/weft/go"
)

// The Larder example: an orders service and a notifications service that talk through one contract
// (spec/examples/orders.weft). They use nothing but the generated code, and are what the end to end
// tests run, over real HTTP and a real Kafka broker.

// OrdersService serves orders over HTTP and announces each new one on Kafka.
type OrdersService struct {
	W *Weft

	mu    sync.Mutex
	store map[UUID]OrdersOrder
}

var stock = map[string]int64{"apple": 100, "pear": 0}

// NewOrdersService registers the handlers of the orders service on w.
func NewOrdersService(w *Weft) (*OrdersService, error) {
	s := &OrdersService{W: w, store: map[UUID]OrdersOrder{}}
	if err := w.HTTP.HandleCreateOrder(s.create); err != nil {
		return nil, err
	}
	if err := w.HTTP.HandleGetOrder(s.get); err != nil {
		return nil, err
	}
	if err := w.HTTP.HandleListOrders(s.list); err != nil {
		return nil, err
	}
	return s, nil
}

func (s *OrdersService) create(ctx context.Context, in CreateOrderInput) (OrdersOrder, error) {
	var total int64
	for _, item := range in.Body.Items {
		if available := stock[item.Sku]; available < item.Qty {
			return OrdersOrder{}, NewOrdersOutOfStock(OrdersOutOfStock{Sku: item.Sku, Available: available}, "Not enough "+item.Sku)
		}
		total += item.Qty * 150
	}
	order := OrdersOrder{
		ID: newUUID(), CustomerID: in.Body.CustomerID, Items: in.Body.Items,
		Total: OrdersMoney{Amount: total, Currency: OrdersCurrencyEUR}, PlacedAt: time.Now().UTC(),
	}
	s.mu.Lock()
	s.store[order.ID] = order
	s.mu.Unlock()
	if err := s.W.Kafka.PublishOrderPlaced(ctx, order, weft.PublishOptions{}); err != nil {
		return OrdersOrder{}, err
	}
	return order, nil
}

func (s *OrdersService) get(_ context.Context, in GetOrderInput) (*OrdersOrder, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if order, ok := s.store[in.Params.ID]; ok {
		return &order, nil
	}
	return nil, nil
}

func (s *OrdersService) list(_ context.Context, in ListOrdersInput) ([]OrdersOrder, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	out := make([]OrdersOrder, 0, len(s.store))
	for _, o := range s.store {
		out = append(out, o)
	}
	if in.Query.Page != nil && int(*in.Query.Page)*10 < len(out) {
		out = out[:*in.Query.Page*10]
	}
	return out, nil
}

// NotificationsService sends a confirmation for every order placed. Fail makes it fail, to see the dead letters.
type NotificationsService struct {
	mu   sync.Mutex
	sent []OrdersOrder
	Fail func() bool
}

// NewNotificationsService registers the handler of the notifications service on w.
func NewNotificationsService(w *Weft) (*NotificationsService, error) {
	s := &NotificationsService{}
	err := w.Kafka.OnOrderPlaced(func(_ context.Context, order OrdersOrder, _ *weft.MessageContext) error {
		if s.Fail != nil && s.Fail() {
			return weft.NewError(weft.Internal, "the mail server is down")
		}
		s.mu.Lock()
		s.sent = append(s.sent, order)
		s.mu.Unlock()
		return nil
	}, weft.SubscribeOptions{FromBeginning: true})
	return s, err
}

// Sent is the orders it has sent a confirmation for.
func (s *NotificationsService) Sent() []OrdersOrder {
	s.mu.Lock()
	defer s.mu.Unlock()
	return append([]OrdersOrder(nil), s.sent...)
}
