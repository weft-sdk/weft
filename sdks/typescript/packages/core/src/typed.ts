import type { HttpContext } from "./http/types.js"
import type { CallOptions } from "./http/types.js"
import type { MessageContext, MessageHandler, PublishOptions, SubscribeOptions } from "./messaging/types.js"
import type { Middleware } from "./middleware.js"

/*
 * The types behind the generated `weft` object. `weft gen` writes the maps (which events and routes
 * exist, with their types); these turn a map into a typed API. Nothing here exists at run time.
 */

/** What the generated code says about one route. A part a route does not have is `undefined`. */
export interface RouteShape {
  readonly body: unknown
  readonly params: unknown
  readonly query: unknown
  readonly headers: unknown
  readonly result: unknown
  /** `true` when the contract's result ends in `?`. */
  readonly optional: boolean
}

/** A map of `"METHOD /path"` to the shape of that route. */
export type RouteMap = { readonly [route: string]: RouteShape }

/** Every path of `R` for one HTTP method. */
export type RoutePaths<R, M extends string> = {
  [K in keyof R & string]: K extends `${M} ${infer P}` ? P : never
}[keyof R & string]

type Shape<R, M extends string, P extends string> = R[`${M} ${P}` & keyof R] extends infer S extends RouteShape ? S : never

/** What a handler is given. */
export type ServeInput<S extends RouteShape> = {
  readonly body: S["body"]
  readonly params: S["params"]
  readonly query: S["query"]
  readonly headers: S["headers"]
  readonly ctx: HttpContext
}

/** What a handler may return. A route whose result is optional may return nothing, which answers 404. */
export type ServeResult<S extends RouteShape> = S["optional"] extends true ? S["result"] | null | undefined : S["result"]

type ServeFn<R, M extends string> = <P extends RoutePaths<R, M>>(
  path: P,
  handler: (input: ServeInput<Shape<R, M, P>>) => Promise<ServeResult<Shape<R, M, P>>> | ServeResult<Shape<R, M, P>>,
) => void

type Part<K extends string, T> = [T] extends [undefined]
  ? unknown
  : {} extends T
    ? { readonly [P in K]?: T }
    : { readonly [P in K]: T }

/** The options of a call: exactly what the route takes, and only what it needs to be given. */
export type CallInput<S extends RouteShape> = Part<"body", S["body"]> &
  Part<"params", S["params"]> &
  Part<"query", S["query"]> &
  Part<"headers", S["headers"]> &
  Pick<CallOptions, "timeoutMs" | "idempotencyKey">

type CallArgs<S extends RouteShape> = {} extends CallInput<S> ? [options?: CallInput<S>] : [options: CallInput<S>]

type CallResult<S extends RouteShape> = S["optional"] extends true ? S["result"] | undefined : S["result"]

type CallFn<R, M extends string> = <P extends RoutePaths<R, M>>(
  path: P,
  ...args: CallArgs<Shape<R, M, P>>
) => Promise<CallResult<Shape<R, M, P>>>

/** A client for one other service. */
export interface TypedClient<R> {
  get: CallFn<R, "GET">
  post: CallFn<R, "POST">
  put: CallFn<R, "PUT">
  patch: CallFn<R, "PATCH">
  delete: CallFn<R, "DELETE">
}

/**
 * `weft.http`. `Serve` is the routes this service serves, `Services` the routes of each package by
 * name, for `to()`. `Raw` is the HTTP library's own object.
 */
export interface TypedHttp<Serve, Services extends { readonly [service: string]: RouteMap }, Raw = unknown> {
  get: ServeFn<Serve, "GET">
  post: ServeFn<Serve, "POST">
  put: ServeFn<Serve, "PUT">
  patch: ServeFn<Serve, "PATCH">
  delete: ServeFn<Serve, "DELETE">
  to<N extends keyof Services & string>(service: N): TypedClient<Services[N]>
  use(middleware: Middleware<HttpContext>): unknown
  /** The error to return for "nothing here": `return found ?? weft.http.notFound()`. */
  notFound(message?: string): never
  readonly raw: Raw
}

/** `weft.kafka` and the other messaging transports. `Events` maps an event name to its payload. */
export interface TypedMessaging<Events extends { readonly [event: string]: unknown }, Raw = unknown> {
  publish<E extends keyof Events & string>(event: E, payload: Events[E], options?: PublishOptions): Promise<void>
  on<E extends keyof Events & string>(event: E, handler: MessageHandler<Events[E]>, options?: SubscribeOptions): void
  use(middleware: Middleware<MessageContext>): unknown
  readonly raw: Raw
}
