/** One thing wrong with a value, found by validation. */
export interface Issue {
  /** Where, such as `items[0].qty`. Empty for the value itself. */
  readonly path: string
  readonly message: string
}

export type ErrorCategory =
  | "NotFound"
  | "InvalidInput"
  | "Unauthorized"
  | "Forbidden"
  | "Conflict"
  | "Unavailable"
  | "Timeout"
  | "Internal"

/** What a receiving messaging consumer does when a handler fails with this category. */
export type MessagingPolicy = "none" | "dead-letter" | "retry" | "retry-then-dead-letter"

export interface CategoryInfo {
  readonly http: number
  readonly grpc: string
  readonly messaging: MessagingPolicy
}

/** The eight categories and how each maps to a transport (spec/behaviour.md). */
export const CATEGORIES: Readonly<Record<ErrorCategory, CategoryInfo>> = {
  NotFound: { http: 404, grpc: "NOT_FOUND", messaging: "none" },
  InvalidInput: { http: 400, grpc: "INVALID_ARGUMENT", messaging: "dead-letter" },
  Unauthorized: { http: 401, grpc: "UNAUTHENTICATED", messaging: "none" },
  Forbidden: { http: 403, grpc: "PERMISSION_DENIED", messaging: "none" },
  Conflict: { http: 409, grpc: "ALREADY_EXISTS", messaging: "none" },
  Unavailable: { http: 503, grpc: "UNAVAILABLE", messaging: "retry" },
  Timeout: { http: 504, grpc: "DEADLINE_EXCEEDED", messaging: "retry" },
  Internal: { http: 500, grpc: "INTERNAL", messaging: "retry-then-dead-letter" },
}

export interface WeftErrorOptions {
  readonly issues?: readonly Issue[]
  readonly cause?: unknown
}

/** An error with one of the eight shared categories. */
export class WeftError extends Error {
  override readonly name = "WeftError"
  readonly category: ErrorCategory
  /** Set for `InvalidInput` raised by validation. */
  readonly issues: readonly Issue[]

  constructor(category: ErrorCategory, message: string, options: WeftErrorOptions = {}) {
    super(message, options.cause === undefined ? undefined : { cause: options.cause })
    this.category = category
    this.issues = options.issues ?? []
  }

  get httpStatus(): number {
    return CATEGORIES[this.category].http
  }

  get grpcCode(): string {
    return CATEGORIES[this.category].grpc
  }

  get messagingPolicy(): MessagingPolicy {
    return CATEGORIES[this.category].messaging
  }

  static notFound(message = "Not found", options?: WeftErrorOptions): WeftError {
    return new WeftError("NotFound", message, options)
  }
  static invalidInput(message = "Invalid input", options?: WeftErrorOptions): WeftError {
    return new WeftError("InvalidInput", message, options)
  }
  static unauthorized(message = "Unauthorized", options?: WeftErrorOptions): WeftError {
    return new WeftError("Unauthorized", message, options)
  }
  static forbidden(message = "Forbidden", options?: WeftErrorOptions): WeftError {
    return new WeftError("Forbidden", message, options)
  }
  static conflict(message = "Conflict", options?: WeftErrorOptions): WeftError {
    return new WeftError("Conflict", message, options)
  }
  static unavailable(message = "Unavailable", options?: WeftErrorOptions): WeftError {
    return new WeftError("Unavailable", message, options)
  }
  static timeout(message = "Timed out", options?: WeftErrorOptions): WeftError {
    return new WeftError("Timeout", message, options)
  }
  static internal(message = "Internal error", options?: WeftErrorOptions): WeftError {
    return new WeftError("Internal", message, options)
  }
}

export function isWeftError(value: unknown): value is WeftError {
  return value instanceof WeftError
}

/** `weft.yaml` could not be used. `problems` lists everything wrong, not only the first. */
export class ConfigError extends Error {
  override readonly name = "ConfigError"
  readonly problems: readonly string[]

  constructor(problems: readonly string[]) {
    super(`Invalid Weft configuration:\n${problems.map((p) => `  - ${p}`).join("\n")}`)
    this.problems = problems
  }
}

/**
 * An error declared in a contract (`error OutOfStock { ... }`). A handler throws it; the receiving
 * side gets the same class back, with the same fields.
 */
export class DeclaredError extends Error {
  override readonly name = "DeclaredError"
  readonly package: string
  readonly errorName: string
  readonly fields: Readonly<Record<string, unknown>>

  constructor(pkg: string, errorName: string, fields: Readonly<Record<string, unknown>> = {}, message?: string) {
    super(message ?? `${pkg}.${errorName}`)
    this.package = pkg
    this.errorName = errorName
    this.fields = fields
  }
}

export function isDeclaredError(value: unknown, pkg?: string, errorName?: string): value is DeclaredError {
  return (
    value instanceof DeclaredError &&
    (pkg === undefined || value.package === pkg) &&
    (errorName === undefined || value.errorName === errorName)
  )
}
