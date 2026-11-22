package weft

// Envelope is one message as it travels: a CloudEvents 1.0 envelope around the encoded payload.
// Adapters carry the attributes in the transport's own headers (binary mode) and the body as the value.
type Envelope struct {
	// ID is the CloudEvents id: unique per message.
	ID string
	// Type is the CloudEvents type: the event name, such as order.placed.
	Type string
	// Source is the service that sent it.
	Source string
	// Time is RFC 3339.
	Time            string
	DataContentType string
	// Key is the ordering and partition key, if the event declares one.
	Key string
	// Headers are the other headers: trace context, the idempotency key, and anything the sender added.
	Headers map[string]string
	// Body is the encoded payload (JSON by default).
	Body []byte
}

var ceAttributes = []string{"id", "type", "source", "time"}

// ToHeaders returns the headers for an envelope in CloudEvents binary mode: <prefix>specversion,
// <prefix>id, ... The prefix depends on the transport (ce_ for Kafka, ce- for HTTP, cloudEvents: for
// AMQP). The key and the body are for the adapter to place natively; other headers are copied.
func ToHeaders(e Envelope, prefix string) map[string]string {
	h := make(map[string]string, len(e.Headers)+6)
	for k, v := range e.Headers {
		h[k] = v
	}
	h[prefix+"specversion"] = "1.0"
	h[prefix+"id"] = e.ID
	h[prefix+"type"] = e.Type
	h[prefix+"source"] = e.Source
	h[prefix+"time"] = e.Time
	h[prefix+"datacontenttype"] = e.DataContentType
	return h
}

// FromHeaders is the reverse. When the message is not a CloudEvent it returns the missing header names.
func FromHeaders(headers map[string]string, prefix string, body []byte, key string) (Envelope, []string) {
	var missing []string
	for _, name := range ceAttributes {
		if _, ok := headers[prefix+name]; !ok {
			missing = append(missing, prefix+name)
		}
	}
	if len(missing) > 0 {
		return Envelope{}, missing
	}
	rest := map[string]string{}
	for k, v := range headers {
		if len(k) < len(prefix) || k[:len(prefix)] != prefix {
			rest[k] = v
		}
	}
	contentType := headers[prefix+"datacontenttype"]
	if contentType == "" {
		contentType = "application/json"
	}
	return Envelope{
		ID: headers[prefix+"id"], Type: headers[prefix+"type"], Source: headers[prefix+"source"], Time: headers[prefix+"time"],
		DataContentType: contentType, Key: key, Headers: rest, Body: body,
	}, nil
}
