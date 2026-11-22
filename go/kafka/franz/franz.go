// Package franz is Weft's Kafka adapter on franz-go. franz-go stays in charge of the connection, the
// partitions and the rebalancing; Weft does the envelopes, validation, retries and dead letters.
//
// A message is acknowledged (its offset committed) when the handler returns. Retries happen inside
// the handler, so a message that is being retried holds up its partition, and its consumer must keep
// its group membership: keep the retry time well under the session timeout.
package franz

import (
	"context"
	"errors"
	"fmt"
	"strings"
	"sync"
	"time"

	"github.com/twmb/franz-go/pkg/kadm"
	"github.com/twmb/franz-go/pkg/kgo"

	weft "github.com/weft-sdk/weft/go"
)

// prefix is where CloudEvents binary mode puts the attributes on Kafka.
const prefix = "ce_"

// Options configure an Adapter.
type Options struct {
	// Brokers, or transports.kafka.brokers in weft.yaml (a list, or a comma-separated string).
	Brokers []string
	// ClientID defaults to transports.kafka.clientId, then the service name.
	ClientID string
	// Group is the consumer group used when On gives none. Or transports.kafka.group. Default: the service name.
	Group string
	// SkipCreateTopics turns off creating the topics this service consumes. By default they are created
	// first, because a consumer cannot join for a topic that does not exist yet. A failure to create,
	// such as no permission, is ignored: the topic may already be there.
	SkipCreateTopics bool
	// ExtraOptions are passed to every franz-go client (TLS, SASL, compression, ...). They come last, so
	// they override what the adapter sets.
	ExtraOptions []kgo.Opt
}

type subscription struct {
	topic         string
	group         string
	fromBeginning bool
	handler       func(context.Context, weft.Delivery) error
}

// Adapter is a weft.MessagingAdapter on franz-go. Raw is the producing *kgo.Client.
type Adapter struct {
	opts     Options
	settings map[string]any
	service  string

	mu        sync.Mutex
	producer  *kgo.Client
	subs      []subscription
	consumers []*kgo.Client
	cancel    context.CancelFunc
	loops     sync.WaitGroup
}

// New makes an Adapter.
func New(o Options) *Adapter { return &Adapter{opts: o, service: "weft"} }

func (a *Adapter) Name() string { return "kafka" }

// Configure reads transports.kafka from weft.yaml.
func (a *Adapter) Configure(settings map[string]any, config *weft.Config) {
	a.settings = settings
	a.service = config.Service
}

func (a *Adapter) Capabilities() weft.MessagingCapabilities {
	return weft.MessagingCapabilities{Ordering: true, Keys: true, Headers: true, ConsumerGroups: true}
}

func (a *Adapter) brokers() []string {
	if len(a.opts.Brokers) > 0 {
		return a.opts.Brokers
	}
	var out []string
	switch b := a.settings["brokers"].(type) {
	case string:
		for _, s := range strings.Split(b, ",") {
			if s = strings.TrimSpace(s); s != "" {
				out = append(out, s)
			}
		}
	case []any:
		for _, s := range b {
			out = append(out, strings.TrimSpace(fmt.Sprint(s)))
		}
	}
	return out
}

func (a *Adapter) clientID() string {
	if a.opts.ClientID != "" {
		return a.opts.ClientID
	}
	if id, ok := a.settings["clientId"].(string); ok {
		return id
	}
	return a.service
}

func (a *Adapter) defaultGroup() string {
	if a.opts.Group != "" {
		return a.opts.Group
	}
	if g, ok := a.settings["group"].(string); ok {
		return g
	}
	return a.service
}

func (a *Adapter) base() ([]kgo.Opt, error) {
	brokers := a.brokers()
	if len(brokers) == 0 {
		return nil, weft.NewError(weft.Internal, "There are no Kafka brokers: set brokers under transports.kafka in weft.yaml (or WEFT_KAFKA_BROKERS)")
	}
	// Compression is off unless you ask for it with ExtraOptions: franz-go compresses with Snappy by
	// default, which a consumer on KafkaJS (and others) cannot read without a plugin, so a Go service
	// would send messages that a TypeScript service cannot receive.
	opts := []kgo.Opt{kgo.SeedBrokers(brokers...), kgo.ClientID(a.clientID()), kgo.ProducerBatchCompression(kgo.NoCompression())}
	return append(opts, a.opts.ExtraOptions...), nil
}

// Raw is the franz-go client that produces. It is made from the configured brokers the first time it is needed.
func (a *Adapter) Raw() any {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.producer == nil {
		if client, err := a.newProducer(); err == nil {
			a.producer = client
		}
	}
	return a.producer
}

func (a *Adapter) newProducer() (*kgo.Client, error) {
	opts, err := a.base()
	if err != nil {
		return nil, err
	}
	return kgo.NewClient(append(opts, kgo.AllowAutoTopicCreation(), kgo.RecordPartitioner(kgo.StickyKeyPartitioner(nil)))...)
}

func (a *Adapter) Start(ctx context.Context) error {
	a.mu.Lock()
	defer a.mu.Unlock()
	if a.producer == nil {
		client, err := a.newProducer()
		if err != nil {
			return err
		}
		a.producer = client
	}
	return a.producer.Ping(ctx)
}

func (a *Adapter) Subscribe(_ context.Context, destination string, handler func(context.Context, weft.Delivery) error, options map[string]any) error {
	group := a.defaultGroup()
	if g, ok := options["group"].(string); ok && g != "" {
		group = g
	}
	fromBeginning, _ := options["fromBeginning"].(bool)
	a.mu.Lock()
	a.subs = append(a.subs, subscription{topic: destination, group: group, fromBeginning: fromBeginning, handler: handler})
	a.mu.Unlock()
	return nil
}

// Ready starts consuming, once every subscription is known.
func (a *Adapter) Ready(ctx context.Context) error {
	a.mu.Lock()
	subs := append([]subscription(nil), a.subs...)
	a.mu.Unlock()
	if len(subs) == 0 {
		return nil
	}
	if !a.opts.SkipCreateTopics {
		a.createTopics(ctx, subs)
	}

	runCtx, cancel := context.WithCancel(context.Background())
	a.mu.Lock()
	a.cancel = cancel
	a.mu.Unlock()

	// One client per consumer group and per reset policy, so that each group's topics are its own.
	type key struct {
		group         string
		fromBeginning bool
	}
	groups := map[key][]subscription{}
	var order []key
	for _, s := range subs {
		k := key{s.group, s.fromBeginning}
		if _, ok := groups[k]; !ok {
			order = append(order, k)
		}
		groups[k] = append(groups[k], s)
	}
	for _, k := range order {
		members := groups[k]
		topics := make([]string, len(members))
		byTopic := map[string]subscription{}
		for i, m := range members {
			topics[i] = m.topic
			byTopic[m.topic] = m
		}
		opts, err := a.base()
		if err != nil {
			return err
		}
		reset := kgo.NewOffset().AtEnd()
		if k.fromBeginning {
			reset = kgo.NewOffset().AtStart()
		}
		client, err := kgo.NewClient(append(opts,
			kgo.ConsumerGroup(k.group), kgo.ConsumeTopics(topics...), kgo.ConsumeResetOffset(reset),
			kgo.DisableAutoCommit(), kgo.BlockRebalanceOnPoll(),
		)...)
		if err != nil {
			return err
		}
		a.mu.Lock()
		a.consumers = append(a.consumers, client)
		a.mu.Unlock()
		a.loops.Add(1)
		go a.consume(runCtx, client, byTopic)
	}
	return nil
}

func (a *Adapter) createTopics(ctx context.Context, subs []subscription) {
	opts, err := a.base()
	if err != nil {
		return
	}
	client, err := kgo.NewClient(opts...)
	if err != nil {
		return
	}
	defer client.Close()
	seen := map[string]bool{}
	var topics []string
	for _, s := range subs {
		if !seen[s.topic] {
			seen[s.topic] = true
			topics = append(topics, s.topic)
		}
	}
	// A topic that already exists, or that we may not create, is not a problem here.
	_, _ = kadm.NewClient(client).CreateTopics(ctx, 1, 1, nil, topics...)
}

// consume polls until ctx ends. A record is committed only after its handler returned nil. A handler
// that returns an error (shutdown, or a crash) stops this loop, leaving the record uncommitted so
// the group's next consumer receives it.
func (a *Adapter) consume(ctx context.Context, client *kgo.Client, byTopic map[string]subscription) {
	defer a.loops.Done()
	// Polling blocks rebalances until we say so; never leave the loop with one blocked.
	defer client.AllowRebalance()
	for ctx.Err() == nil {
		fetches := client.PollFetches(ctx)
		if fetches.IsClientClosed() || ctx.Err() != nil {
			return
		}
		failed := false
		fetches.EachPartition(func(p kgo.FetchTopicPartition) {
			if failed {
				return
			}
			sub, ok := byTopic[p.Topic]
			if !ok {
				return
			}
			for _, rec := range p.Records {
				if err := sub.handler(ctx, delivery(rec)); err != nil {
					failed = true
					return
				}
				if err := client.CommitRecords(ctx, rec); err != nil {
					failed = true
					return
				}
			}
		})
		client.AllowRebalance()
		if failed {
			return
		}
	}
}

func delivery(rec *kgo.Record) weft.Delivery {
	headers := make(map[string]string, len(rec.Headers))
	for _, h := range rec.Headers {
		headers[h.Key] = string(h.Value)
	}
	key := string(rec.Key)
	e, missing := weft.FromHeaders(headers, prefix, rec.Value, key)
	if missing != nil {
		// Not sent by Weft: give it an identity of its own.
		e = weft.Envelope{
			ID: fmt.Sprintf("%s:%d:%d", rec.Topic, rec.Partition, rec.Offset), Type: rec.Topic, Source: "unknown",
			Time: rec.Timestamp.UTC().Format("2006-01-02T15:04:05.000Z"), DataContentType: "application/json",
			Key: key, Headers: headers, Body: rec.Value,
		}
	}
	return weft.Delivery{Envelope: e}
}

func (a *Adapter) Publish(ctx context.Context, destination string, e weft.Envelope, options map[string]any) error {
	a.mu.Lock()
	producer := a.producer
	a.mu.Unlock()
	if producer == nil {
		return weft.NewError(weft.Unavailable, "The Kafka producer is not connected")
	}
	headers := weft.ToHeaders(e, prefix)
	rec := &kgo.Record{Topic: destination, Value: e.Body}
	if e.Key != "" {
		rec.Key = []byte(e.Key)
	}
	for k, v := range headers {
		rec.Headers = append(rec.Headers, kgo.RecordHeader{Key: k, Value: []byte(v)})
	}
	if t, err := time.Parse(time.RFC3339, e.Time); err == nil {
		rec.Timestamp = t
	}
	if p, ok := options["partition"].(int); ok {
		rec.Partition = int32(p)
	}
	if err := producer.ProduceSync(ctx, rec).FirstErr(); err != nil {
		retriable := true
		var kerr interface{ Retriable() bool }
		if errors.As(err, &kerr) {
			retriable = kerr.Retriable()
		}
		category := weft.Unavailable
		if !retriable {
			category = weft.Internal
		}
		return &weft.Error{Category: category, Message: fmt.Sprintf("Kafka could not send to %s: %v", destination, err), Cause: err}
	}
	return nil
}

// StopIntake stops consuming. A handler that is running finishes first; Weft has already told it to give up.
func (a *Adapter) StopIntake(context.Context) error {
	a.mu.Lock()
	cancel := a.cancel
	a.mu.Unlock()
	if cancel != nil {
		cancel()
	}
	a.loops.Wait()
	return nil
}

// Close leaves the groups and closes the connections.
func (a *Adapter) Close(context.Context) error {
	a.mu.Lock()
	consumers := a.consumers
	producer := a.producer
	a.consumers, a.producer = nil, nil
	a.mu.Unlock()
	for _, c := range consumers {
		c.CloseAllowingRebalance()
	}
	if producer != nil {
		producer.Close()
	}
	return nil
}
