package weft_test

import (
	"context"
	"strings"
	"testing"

	"go.opentelemetry.io/otel"
	"go.opentelemetry.io/otel/propagation"
	sdkmetric "go.opentelemetry.io/otel/sdk/metric"
	"go.opentelemetry.io/otel/sdk/metric/metricdata"
	sdktrace "go.opentelemetry.io/otel/sdk/trace"
	"go.opentelemetry.io/otel/sdk/trace/tracetest"
	"go.opentelemetry.io/otel/trace"

	weft "github.com/weft-sdk/weft/go"
)

// withTelemetry installs a recording tracer and meter for one test.
func withTelemetry(t *testing.T) (*tracetest.SpanRecorder, *sdkmetric.ManualReader) {
	t.Helper()
	recorder := tracetest.NewSpanRecorder()
	tp := sdktrace.NewTracerProvider(sdktrace.WithSpanProcessor(recorder))
	reader := sdkmetric.NewManualReader()
	mp := sdkmetric.NewMeterProvider(sdkmetric.WithReader(reader))
	oldTP, oldMP, oldProp := otel.GetTracerProvider(), otel.GetMeterProvider(), otel.GetTextMapPropagator()
	otel.SetTracerProvider(tp)
	otel.SetMeterProvider(mp)
	otel.SetTextMapPropagator(propagation.TraceContext{})
	t.Cleanup(func() {
		otel.SetTracerProvider(oldTP)
		otel.SetMeterProvider(oldMP)
		otel.SetTextMapPropagator(oldProp)
	})
	return recorder, reader
}

func spanNamed(spans []sdktrace.ReadOnlySpan, name string) []sdktrace.ReadOnlySpan {
	var out []sdktrace.ReadOnlySpan
	for _, s := range spans {
		if s.Name() == name {
			out = append(out, s)
		}
	}
	return out
}

func TestAPublishAndItsConsumeAreOneTrace(t *testing.T) {
	recorder, _ := withTelemetry(t)
	r := newRig(t, "", nil, false)
	failed := false
	_ = r.m.On("order.placed", func(context.Context, any, *weft.MessageContext) error {
		if !failed {
			failed = true
			return weft.NewError(weft.Unavailable, "down")
		}
		return nil
	}, weft.SubscribeOptions{})
	r.start(t)
	if err := r.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{}); err != nil {
		t.Fatal(err)
	}
	r.broker.Idle()

	spans := recorder.Ended()
	publish := spanNamed(spans, "order.placed publish")
	process := spanNamed(spans, "order.placed process")
	if len(publish) != 1 || len(process) != 2 {
		t.Fatalf("publish %d, process %d", len(publish), len(process))
	}
	if publish[0].SpanKind() != trace.SpanKindProducer || process[0].SpanKind() != trace.SpanKindConsumer {
		t.Error("kinds")
	}
	for _, p := range process {
		if p.SpanContext().TraceID() != publish[0].SpanContext().TraceID() || p.Parent().SpanID() != publish[0].SpanContext().SpanID() {
			t.Errorf("the consumer is not a child of the publisher: %v", p.Parent())
		}
	}
	attrs := map[string]string{}
	for _, a := range process[0].Attributes() {
		attrs[string(a.Key)] = a.Value.Emit()
	}
	if attrs["messaging.system"] != "kafka" || attrs["messaging.operation.type"] != "process" || attrs["weft.attempt"] != "1" || attrs["weft.event"] != "order.placed" {
		t.Errorf("attributes = %v", attrs)
	}
	if process[0].Status().Code.String() != "Error" || process[1].Status().Code.String() != "Unset" {
		t.Errorf("status %v then %v", process[0].Status(), process[1].Status())
	}
	if len(process[0].Events()) == 0 {
		t.Error("the failed attempt should record the exception")
	}
}

func TestAPublishContinuesTheCallersTraceAndADeadLetterKeepsIt(t *testing.T) {
	recorder, _ := withTelemetry(t)
	r := newRig(t, "", nil, false)
	_ = r.m.On("order.placed", func(context.Context, any, *weft.MessageContext) error {
		return weft.NewError(weft.InvalidInput, "bad")
	}, weft.SubscribeOptions{})
	r.start(t)
	ctx, root := otel.Tracer("test").Start(context.Background(), "request")
	if err := r.m.Publish(ctx, "order.placed", placed, weft.PublishOptions{}); err != nil {
		t.Fatal(err)
	}
	root.End()
	r.broker.Idle()

	original := r.broker.To("order.placed")[0].Envelope.Headers["traceparent"]
	dead := r.broker.To("order.placed.dlq")[0].Envelope.Headers["traceparent"]
	if !strings.HasPrefix(original, "00-"+root.SpanContext().TraceID().String()) || dead != original {
		t.Errorf("original %q, dead %q", original, dead)
	}
	for _, s := range recorder.Ended() {
		if s.SpanContext().TraceID() != root.SpanContext().TraceID() {
			t.Errorf("%s is in another trace", s.Name())
		}
	}
	if len(spanNamed(recorder.Ended(), "order.placed.dlq publish")) != 1 {
		t.Error("the dead-letter send has a span")
	}
}

func TestInjectAndExtractTraceContext(t *testing.T) {
	withTelemetry(t)
	ctx, span := otel.Tracer("test").Start(context.Background(), "outer")
	defer span.End()
	headers := map[string]string{}
	weft.InjectTraceContext(ctx, headers)
	want := "00-" + span.SpanContext().TraceID().String() + "-" + span.SpanContext().SpanID().String() + "-01"
	if headers["traceparent"] != want {
		t.Errorf("traceparent = %q", headers["traceparent"])
	}
	got := trace.SpanContextFromContext(weft.ExtractTraceContext(context.Background(), headers))
	if got.TraceID() != span.SpanContext().TraceID() || got.SpanID() != span.SpanContext().SpanID() || !got.IsRemote() {
		t.Errorf("extracted = %v", got)
	}
	if trace.SpanContextFromContext(weft.ExtractTraceContext(context.Background(), map[string]string{})).IsValid() {
		t.Error("no headers, no trace")
	}
}

func TestMetrics(t *testing.T) {
	_, reader := withTelemetry(t)
	r := newRig(t, "", nil, false)
	n := 0
	_ = r.m.On("order.placed", func(context.Context, any, *weft.MessageContext) error {
		n++
		if n == 1 {
			return weft.NewError(weft.Timeout, "slow")
		}
		return nil
	}, weft.SubscribeOptions{})
	r.start(t)
	_ = r.m.Publish(context.Background(), "order.placed", placed, weft.PublishOptions{})
	r.broker.Idle()

	var rm metricdata.ResourceMetrics
	if err := reader.Collect(context.Background(), &rm); err != nil {
		t.Fatal(err)
	}
	counts := map[string]int64{}
	for _, sm := range rm.ScopeMetrics {
		for _, m := range sm.Metrics {
			switch data := m.Data.(type) {
			case metricdata.Sum[int64]:
				for _, dp := range data.DataPoints {
					key := m.Name
					for _, kv := range dp.Attributes.ToSlice() {
						if k := string(kv.Key); k == "outcome" || k == "category" {
							key += "/" + kv.Value.Emit()
						}
					}
					counts[key] += dp.Value
				}
			case metricdata.Histogram[float64]:
				for _, dp := range data.DataPoints {
					counts[m.Name] += int64(dp.Count)
				}
			}
		}
	}
	for _, k := range []string{
		"weft.messaging.messages.sent/ok", "weft.messaging.messages.received/ok", "weft.messaging.messages.received/retry",
		"weft.messaging.errors/Timeout", "weft.messaging.process.duration", "weft.messaging.message.size",
	} {
		if counts[k] < 1 {
			t.Errorf("no %s in %v", k, counts)
		}
	}
}
