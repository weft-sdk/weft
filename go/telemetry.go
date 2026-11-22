package weft

import (
	"context"
	"sync"

	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/codes"
	"go.opentelemetry.io/otel/metric"
	"go.opentelemetry.io/otel/propagation"
	"go.opentelemetry.io/otel/trace"
)

// Weft emits OpenTelemetry through the API only. With no SDK installed every call here does nothing;
// install and configure an SDK (and an exporter) in the application to see the spans and metrics.

const instrumentation = "github.com/weft-sdk/weft/go"

func tracer() trace.Tracer {
	return otel.GetTracerProvider().Tracer(instrumentation, trace.WithInstrumentationVersion(Version))
}

func meter() metric.Meter {
	return otel.GetMeterProvider().Meter(instrumentation, metric.WithInstrumentationVersion(Version))
}

// InjectTraceContext writes the span in ctx into headers as traceparent (and tracestate), W3C Trace Context.
func InjectTraceContext(ctx context.Context, headers map[string]string) {
	otel.GetTextMapPropagator().Inject(ctx, propagation.MapCarrier(headers))
}

// ExtractTraceContext returns ctx with the trace context carried in headers, to be the parent of a span.
func ExtractTraceContext(ctx context.Context, headers map[string]string) context.Context {
	return otel.GetTextMapPropagator().Extract(ctx, propagation.MapCarrier(headers))
}

func injectTraceContext(ctx context.Context, headers map[string]string) {
	InjectTraceContext(ctx, headers)
}

// RecordError marks a span as failed and records why.
func RecordError(span trace.Span, err error) {
	span.RecordError(err)
	if e, ok := err.(*Error); ok {
		span.SetAttributes(attribute.String("weft.error.category", string(e.Category)))
	}
	span.SetStatus(codes.Error, err.Error())
}

type messagingInstruments struct {
	sent, received, errors metric.Int64Counter
	duration, size         metric.Float64Histogram
}

var (
	instruments    messagingInstruments
	instrumentsFor metric.MeterProvider
	instrumentsMu  sync.Mutex
)

// metrics creates the instruments on first use, and again if the global meter provider has changed.
func metrics() *messagingInstruments {
	instrumentsMu.Lock()
	defer instrumentsMu.Unlock()
	if instrumentsFor != otel.GetMeterProvider() {
		m := meter()
		instruments.sent, _ = m.Int64Counter("weft.messaging.messages.sent", metric.WithDescription("Messages sent, by event and outcome"))
		instruments.received, _ = m.Int64Counter("weft.messaging.messages.received", metric.WithDescription("Messages received, by event and outcome (ok, retry, dead_letter, dropped, not_acked)"))
		instruments.errors, _ = m.Int64Counter("weft.messaging.errors", metric.WithDescription("Failures, by event and error category"))
		instruments.duration, _ = m.Float64Histogram("weft.messaging.process.duration", metric.WithUnit("ms"), metric.WithDescription("Time a handler took, per attempt"))
		instruments.size, _ = m.Float64Histogram("weft.messaging.message.size", metric.WithUnit("By"), metric.WithDescription("Size of a message body"))
		instrumentsFor = otel.GetMeterProvider()
	}
	return &instruments
}

func messagingAttributes(operation, system, destination, id string, bytes int, event string) []attribute.KeyValue {
	return []attribute.KeyValue{
		attribute.String("messaging.system", system),
		attribute.String("messaging.destination.name", destination),
		attribute.String("messaging.operation.type", operation),
		attribute.String("messaging.message.id", id),
		attribute.Int("messaging.message.body.size", bytes),
		attribute.String("weft.event", event),
	}
}

func recordReceived(event, transport, outcome string) {
	metrics().received.Add(context.Background(), 1, metric.WithAttributes(
		attribute.String("event", event), attribute.String("transport", transport), attribute.String("outcome", outcome)))
}

// instrumentedPublish runs send inside a producer span and records the metrics.
func (m *Messaging) instrumentedPublish(ctx context.Context, destination, event, id string, size int, send func(context.Context) error) error {
	ctx, span := tracer().Start(ctx, destination+" publish", trace.WithSpanKind(trace.SpanKindProducer),
		trace.WithAttributes(messagingAttributes("publish", m.name, destination, id, size, event)...))
	defer span.End()
	err := send(ctx)
	attrs := metric.WithAttributes(attribute.String("event", event), attribute.String("transport", m.name))
	in := metrics()
	if err != nil {
		RecordError(span, err)
		in.sent.Add(ctx, 1, attrs, metric.WithAttributes(attribute.String("outcome", "error")))
		in.errors.Add(ctx, 1, attrs, metric.WithAttributes(attribute.String("category", string(AsError(err).Category))))
		return err
	}
	in.sent.Add(ctx, 1, attrs, metric.WithAttributes(attribute.String("outcome", "ok")))
	in.size.Record(ctx, float64(size), attrs, metric.WithAttributes(attribute.String("direction", "send")))
	return nil
}

// instrumentedProcess runs one handler attempt inside a consumer span whose parent is the publisher's.
func (m *Messaging) instrumentedProcess(ctx context.Context, destination string, mc *MessageContext, run func(context.Context) error) error {
	parent := ExtractTraceContext(ctx, mc.Envelope.Headers)
	attrs := append(messagingAttributes("process", m.name, destination, mc.Envelope.ID, len(mc.Envelope.Body), mc.Event), attribute.Int("weft.attempt", mc.Attempt))
	ctx, span := tracer().Start(parent, destination+" process", trace.WithSpanKind(trace.SpanKindConsumer), trace.WithAttributes(attrs...))
	defer span.End()
	started := nowMs()
	err := run(ctx)
	in := metrics()
	labels := metric.WithAttributes(attribute.String("event", mc.Event), attribute.String("transport", m.name))
	if err != nil {
		RecordError(span, err)
		in.duration.Record(ctx, nowMs()-started, labels, metric.WithAttributes(attribute.String("outcome", "error")))
		in.errors.Add(ctx, 1, labels, metric.WithAttributes(attribute.String("category", string(AsError(err).Category))))
		return err
	}
	in.duration.Record(ctx, nowMs()-started, labels, metric.WithAttributes(attribute.String("outcome", "ok")))
	in.received.Add(ctx, 1, labels, metric.WithAttributes(attribute.String("outcome", "ok")))
	in.size.Record(ctx, float64(len(mc.Envelope.Body)), labels, metric.WithAttributes(attribute.String("direction", "receive")))
	return nil
}

// instrumentedDeadLetter sends to the dead-letter destination in the original message's trace.
func (m *Messaging) instrumentedDeadLetter(ctx context.Context, target string, e Envelope, event string, send func(context.Context) error) error {
	parent := ExtractTraceContext(ctx, e.Headers)
	_, span := tracer().Start(parent, target+" publish", trace.WithSpanKind(trace.SpanKindProducer),
		trace.WithAttributes(messagingAttributes("publish", m.name, target, e.ID, len(e.Body), event)...))
	defer span.End()
	err := send(trace.ContextWithSpan(ctx, span))
	if err != nil {
		RecordError(span, err)
	}
	return err
}

func nowMs() float64 { return float64(timeNow().UnixNano()) / 1e6 }

type httpInstruments struct {
	serverDuration, clientDuration metric.Float64Histogram
}

var (
	httpInstrumentsMu  sync.Mutex
	httpInstrumentsVal httpInstruments
	httpInstrumentsFor metric.MeterProvider
)

// httpMetrics creates the HTTP instruments on first use, and again if the global meter provider has changed.
func httpMetrics() *httpInstruments {
	httpInstrumentsMu.Lock()
	defer httpInstrumentsMu.Unlock()
	if httpInstrumentsFor != otel.GetMeterProvider() {
		m := meter()
		httpInstrumentsVal.serverDuration, _ = m.Float64Histogram("weft.http.server.duration", metric.WithUnit("ms"), metric.WithDescription("Time to answer a request, by method, route and status"))
		httpInstrumentsVal.clientDuration, _ = m.Float64Histogram("weft.http.client.duration", metric.WithUnit("ms"), metric.WithDescription("Time for a call to another service, including retries"))
		httpInstrumentsFor = otel.GetMeterProvider()
	}
	return &httpInstrumentsVal
}
