import { SpanKind, extractTraceContext, withSpan } from "../telemetry.js"
import { httpMetrics } from "../telemetry.js"
import type { Contracts } from "../contracts.js"
import { WeftError, type Issue } from "../errors.js"
import type { HttpMethod, RouteDef } from "../ir.js"
import type { Logger } from "../messaging/messaging.js"
import { consoleLogger } from "../messaging/messaging.js"
import type { Middleware } from "../middleware.js"
import type { Transport, Weft } from "../weft.js"
import { callService } from "./client.js"
import { coerceText, decodeTextRecord } from "./coerce.js"
import { fromProblem, toProblem } from "./problem.js"
import { findRoute, fullPath, summarise } from "./routes.js"
import type { CallOptions, HttpAdapter, HttpContext, HttpHandler, HttpInput, HttpRequest, HttpResponse } from "./types.js"

export interface HttpOptions {
  /** What serves requests. Leave it out to use `http` only as a client. */
  readonly adapter?: HttpAdapter
  /** For calls to other services. Defaults to the global `fetch`. */
  readonly fetch?: typeof fetch
  readonly logger?: Logger
  /** For tests: how to wait between attempts. */
  readonly sleep?: (ms: number, signal: AbortSignal) => Promise<void>
  /** For tests: the source of randomness for backoff jitter. */
  readonly random?: () => number
}

interface Registered {
  readonly package: string
  readonly route: RouteDef
  readonly handler: HttpHandler<HttpInput>
}

/** A client for one other service: `weft.http.to("orders").post("/orders", { body })`. */
export interface ServiceClient {
  get(path: string, options?: CallOptions): Promise<unknown>
  post(path: string, options?: CallOptions): Promise<unknown>
  put(path: string, options?: CallOptions): Promise<unknown>
  patch(path: string, options?: CallOptions): Promise<unknown>
  delete(path: string, options?: CallOptions): Promise<unknown>
}

/**
 * `weft.http`: serve the routes of the contracts, and call other services. Serving is core's work
 * over an adapter (the library that owns the socket); calling uses `fetch`.
 */
export class Http implements Transport {
  readonly name = "http"
  readonly weft: Weft
  readonly adapter: HttpAdapter | undefined
  readonly options: HttpOptions
  readonly logger: Logger
  private readonly registered: Registered[] = []
  private readonly middleware: Middleware<HttpContext>[] = []
  private started = false

  constructor(weft: Weft, options: HttpOptions = {}) {
    this.weft = weft
    this.options = options
    this.adapter = options.adapter
    this.logger = options.logger ?? consoleLogger
    this.adapter?.configure?.(weft.config.transports.http ?? {})
    weft.register(this)
  }

  /** The library underneath, if serving. Weft's tracing and middleware do not apply to calls made through it. */
  get raw(): unknown {
    return this.adapter?.raw
  }

  get contracts(): Contracts {
    return this.weft.contracts
  }

  /** Middleware for HTTP only. It runs after the middleware added to `Weft`. */
  use(middleware: Middleware<HttpContext>): this {
    this.middleware.push(middleware)
    return this
  }

  /** The error to return for "nothing here": `return found ?? weft.http.notFound()`. */
  notFound(message = "Not found"): never {
    throw WeftError.notFound(message)
  }

  /** Serves a route of the contracts. Register before `weft.start()`. */
  route<I = HttpInput, R = unknown>(method: HttpMethod, path: string, handler: HttpHandler<I, R>, options: { package?: string } = {}): void {
    if (this.started) throw WeftError.internal(`Register the handler for ${method} ${path} before start()`)
    const found = findRoute(this.contracts, method, path, options.package)
    if (this.registered.some((r) => r.route === found.def)) {
      throw WeftError.internal(`${method} ${fullPath(found.def)} already has a handler`)
    }
    this.registered.push({ package: found.package, route: found.def, handler: handler as HttpHandler<HttpInput> })
  }

  get<I = HttpInput, R = unknown>(path: string, handler: HttpHandler<I, R>, options?: { package?: string }): void {
    this.route("GET", path, handler, options)
  }
  post<I = HttpInput, R = unknown>(path: string, handler: HttpHandler<I, R>, options?: { package?: string }): void {
    this.route("POST", path, handler, options)
  }
  put<I = HttpInput, R = unknown>(path: string, handler: HttpHandler<I, R>, options?: { package?: string }): void {
    this.route("PUT", path, handler, options)
  }
  patch<I = HttpInput, R = unknown>(path: string, handler: HttpHandler<I, R>, options?: { package?: string }): void {
    this.route("PATCH", path, handler, options)
  }
  delete<I = HttpInput, R = unknown>(path: string, handler: HttpHandler<I, R>, options?: { package?: string }): void {
    this.route("DELETE", path, handler, options)
  }

  /** Calls another service, whose base URL is under `services:` in weft.yaml. */
  to(service: string): ServiceClient {
    const call = (method: HttpMethod) => (path: string, options: CallOptions = {}) => callService(this, service, method, path, options)
    return { get: call("GET"), post: call("POST"), put: call("PUT"), patch: call("PATCH"), delete: call("DELETE") }
  }

  // ---- Transport ----

  async start(): Promise<void> {
    if (this.adapter === undefined) {
      if (this.registered.length > 0) throw WeftError.internal("Routes are registered, but there is no HTTP adapter to serve them")
      this.started = true
      return
    }
    for (const entry of this.registered) {
      this.adapter.route(entry.route.method, fullPath(entry.route), (req) => this.handle(entry, req))
    }
    await this.adapter.start()
    this.started = true
  }

  async stopIntake(): Promise<void> {
    await this.adapter?.stopIntake()
  }

  async close(): Promise<void> {
    await this.adapter?.close()
  }

  // ---- serving ----

  private async handle(entry: Registered, req: HttpRequest): Promise<HttpResponse> {
    const { route } = entry
    const template = fullPath(route)
    const deadline = Number(req.headers["weft-deadline"])
    const ctx: HttpContext = {
      transport: "http",
      operation: route.name,
      signal: req.signal,
      headers: { ...req.headers },
      state: {},
      ...(Number.isFinite(deadline) && deadline > 0 ? { deadline } : {}),
      route,
      method: route.method,
      template,
      response: { status: 200, headers: {} },
    }
    const started = performance.now()
    const response = await withSpan(
      `${route.method} ${template}`,
      {
        kind: SpanKind.SERVER,
        parent: extractTraceContext(req.headers),
        attributes: { "http.request.method": route.method, "http.route": template, "weft.operation": route.name },
      },
      async (span) => {
        const out = await this.respond(entry, ctx, req)
        span.setAttribute("http.response.status_code", out.status)
        if (out.status >= 500) span.setStatus({ code: 2, message: `HTTP ${out.status}` })
        return out
      },
    )
    httpMetrics().serverDuration.record(performance.now() - started, {
      method: route.method,
      route: template,
      status: response.status,
    })
    return response
  }

  private async respond(entry: Registered, ctx: HttpContext, req: HttpRequest): Promise<HttpResponse> {
    const { route } = entry
    try {
      let returned: unknown
      await this.weft.dispatch(
        ctx,
        async (c) => {
          const input = this.decodeInput(route, req)
          returned = await entry.handler({ ...input, ctx: c })
        },
        this.middleware,
      )
      if (returned === undefined || returned === null) {
        if (route.result.optional === true) throw WeftError.notFound()
        throw WeftError.internal(`The handler of ${route.name} returned nothing`)
      }
      let body: string
      try {
        body = JSON.stringify(this.contracts.encode(route.result.value, returned))
      } catch (cause) {
        // The caller did nothing wrong: the handler returned something the contract does not allow.
        throw WeftError.internal(`The handler of ${route.name} returned a value that does not match the contract`, { cause })
      }
      return { status: ctx.response.status, headers: { "content-type": "application/json", ...ctx.response.headers }, body }
    } catch (error) {
      const { status, problem, unexpected } = toProblem(error, route, this.contracts)
      if (unexpected !== undefined) this.logger.error(`${route.method} ${fullPath(route)} failed`, { error: String(unexpected) })
      return {
        status,
        headers: { "content-type": "application/problem+json", ...ctx.response.headers },
        body: JSON.stringify(problem),
      }
    }
  }

  private decodeInput(route: RouteDef, req: HttpRequest): Omit<HttpInput, "ctx"> {
    const issues: Issue[] = []
    const prefixed = (prefix: string, found: readonly Issue[]): void => {
      for (const i of found) issues.push({ ...i, path: i.path === "" ? prefix : `${prefix}.${i.path}` })
    }

    const params: Record<string, unknown> = {}
    for (const p of route.pathParams ?? []) {
      const raw = req.params[p.name]
      const result = this.contracts.tryDecode(p.type, coerceText(p.type, raw))
      if (result.ok) params[p.name] = result.value
      else prefixed(`params.${p.name}`, result.issues)
    }

    let query: unknown = {}
    if (route.query !== undefined) {
      const r = decodeTextRecord(this.contracts, route.query, (name) => req.query[name], "query")
      query = r.value
      issues.push(...r.issues)
    }

    let headers: unknown = {}
    if (route.headers !== undefined) {
      const r = decodeTextRecord(this.contracts, route.headers, (name) => req.headers[name.toLowerCase()], "headers")
      headers = r.value
      issues.push(...r.issues)
    }

    let body: unknown
    if (route.body !== undefined) {
      if (req.bodyText === undefined || req.bodyText.trim() === "") {
        issues.push({ path: "body", message: "is required" })
      } else {
        let parsed: unknown
        let ok = true
        try {
          parsed = JSON.parse(req.bodyText)
        } catch {
          ok = false
          issues.push({ path: "body", message: "is not valid JSON" })
        }
        if (ok) {
          const r = this.contracts.tryDecode(route.body, parsed)
          if (r.ok) body = r.value
          else prefixed("body", r.issues)
        }
      }
    }

    if (issues.length > 0) throw WeftError.invalidInput(summarise(issues), { issues })
    return { body, params, query: query as Record<string, unknown>, headers: headers as Record<string, unknown> }
  }
}

export { fromProblem }
