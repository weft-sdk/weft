package weft

import "time"

// Defaults every package shares (docs/04-sdk-design.md).
const (
	// CallTimeout is how long a call to another service waits.
	CallTimeout = 10 * time.Second
	// ShutdownGrace is how long Stop waits for in-flight handlers.
	ShutdownGrace = 30 * time.Second
	// MessagingAttempts is the attempts before a message is dead-lettered.
	MessagingAttempts = 5
	// BackoffMin and BackoffMax bound the retry backoff.
	BackoffMin = 200 * time.Millisecond
	BackoffMax = 30 * time.Second
	// CallAttempts is the attempts of an HTTP or gRPC call.
	CallAttempts = 3
)
