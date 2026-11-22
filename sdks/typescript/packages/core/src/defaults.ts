/** The defaults every package shares (docs/04-sdk-design.md). */
export const DEFAULTS = {
  /** A call to another service gives up after this long. */
  callTimeoutMs: 10_000,
  /** `stop()` waits this long for in-flight handlers before it closes connections anyway. */
  shutdownGraceMs: 30_000,
  messaging: {
    /** Attempts before a message goes to the dead-letter destination. */
    attempts: 5,
    backoffMinMs: 200,
    backoffMaxMs: 30_000,
  },
  /** HTTP and gRPC calls: only `Unavailable` and `Timeout`, only for idempotent operations. */
  calls: { attempts: 3 },
} as const
