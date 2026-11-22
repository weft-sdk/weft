import type { Context } from "../middleware.js"
import type { HttpMethod, RouteDef } from "../ir.js"
import type { Transport } from "../weft.js"

/** A request, as an adapter hands it to core. Nothing is decoded yet. */
export interface HttpRequest {
  readonly method: HttpMethod
  /** The route template that matched, such as `/v1/orders/{id}`. */
  readonly template: string
  readonly params: Readonly<Record<string, string>>
  readonly query: Readonly<Record<string, string | readonly string[]>>
  /** Header names in lower case. */
  readonly headers: Readonly<Record<string, string>>
  /** The raw body, if there was one. Core parses it. */
  readonly bodyText?: string
  /** Aborted when the client goes away. */
  readonly signal: AbortSignal
}

/** What core wants written back. */
export interface HttpResponse {
  readonly status: number
  readonly headers: Readonly<Record<string, string>>
  readonly body?: string
}

/**
 * The HTTP library underneath, as core needs it. It owns the socket, the router and the parsing of
 * the URL; core does the contracts, the middleware, the errors and the tracing.
 */
export interface HttpAdapter<Raw = unknown> extends Transport {
  readonly raw: Raw
  /** Settings from `transports.http` in weft.yaml, such as `port`. Called once, before `route`. */
  configure?(settings: Readonly<Record<string, unknown>>): void
  /** Serve `template` (with `{name}` parameters) for `method`. Called before `start()`. */
  route(method: HttpMethod, template: string, handler: (req: HttpRequest) => Promise<HttpResponse>): void
}

/** What an HTTP handler sees besides its input. */
export interface HttpContext extends Context {
  readonly route: RouteDef
  readonly method: HttpMethod
  readonly template: string
  /** Set `status` and `headers` here to change the response. `status` defaults to 200. */
  readonly response: { status: number; headers: Record<string, string> }
}

export interface HttpInput<B = unknown, P = Record<string, unknown>, Q = Record<string, unknown>, H = Record<string, unknown>> {
  readonly body: B
  readonly params: P
  readonly query: Q
  readonly headers: H
  readonly ctx: HttpContext
}

export type HttpHandler<I = HttpInput, R = unknown> = (input: I) => Promise<R> | R

export interface CallOptions {
  readonly body?: unknown
  /** Values for the `{name}` parts of the path. */
  readonly params?: Readonly<Record<string, unknown>>
  readonly query?: Readonly<Record<string, unknown>>
  /** The route's `headers` record, in code form. Without one in the contract, plain text values. */
  readonly headers?: Readonly<Record<string, unknown>>
  /** Overrides the 10 s call timeout. */
  readonly timeoutMs?: number
  /** Sent as `Idempotency-Key`; makes a POST safe to retry. */
  readonly idempotencyKey?: string
  /** Another package, if two declare the same route. */
  readonly package?: string
}
