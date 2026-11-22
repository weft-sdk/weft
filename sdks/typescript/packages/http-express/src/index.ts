import type { Server } from "node:http"
import type { AddressInfo } from "node:net"
import express, { type Express, type NextFunction, type Request, type RequestHandler, type Response } from "express"
import type { HttpAdapter, HttpMethod, HttpRequest, HttpResponse } from "@weft-sdk/core"

export interface ExpressAdapterOptions {
  /** Your own Express app. Without one, the adapter makes its own and answers unknown paths with a 404 problem. */
  readonly app?: Express
  /** Default 3000, or `transports.http.port` in weft.yaml. Use 0 for any free port. */
  readonly port?: number
  /** Default: all interfaces. Or `transports.http.host`. */
  readonly host?: string
  /** Largest request body, in bytes. Default 1 MiB. */
  readonly bodyLimitBytes?: number
  /** After in-flight handlers have finished, how long to let responses finish being written before closing connections. Default 2000. */
  readonly closeGraceMs?: number
}

const PARAM = /\{([A-Za-z_][A-Za-z0-9_]*)\}/g

function problem(status: number, title: string, detail: string): string {
  return JSON.stringify({ type: `urn:weft:error:${status === 413 || status < 500 ? "InvalidInput" : "Internal"}`, title, status, detail })
}

/**
 * Serves Weft's HTTP routes with Express. Express stays in charge of the socket and the router; Weft
 * does the contracts, middleware, errors and tracing. `raw` is the Express app.
 */
export class ExpressAdapter implements HttpAdapter<Express> {
  readonly name = "http"
  readonly raw: Express
  private readonly ownsApp: boolean
  private readonly options: ExpressAdapterOptions
  private settings: Readonly<Record<string, unknown>> = {}
  private server: Server | undefined
  private closed: Promise<void> | undefined
  private intake = true

  constructor(options: ExpressAdapterOptions = {}) {
    this.options = options
    this.ownsApp = options.app === undefined
    this.raw = options.app ?? express()
    if (this.ownsApp) this.raw.disable("x-powered-by")
  }

  configure(settings: Readonly<Record<string, unknown>>): void {
    this.settings = settings
  }

  /** The port it is listening on, once started. Useful with `port: 0`. */
  get port(): number | undefined {
    const address = this.server?.address()
    return address !== null && typeof address === "object" ? (address as AddressInfo).port : undefined
  }

  route(method: HttpMethod, template: string, handler: (req: HttpRequest) => Promise<HttpResponse>): void {
    const path = template.replace(PARAM, ":$1")
    const limit = this.options.bodyLimitBytes ?? 1024 * 1024
    const parse = express.text({ type: () => true, limit })
    const run: RequestHandler = async (req: Request, res: Response) => {
      const controller = new AbortController()
      res.on("close", () => {
        if (!res.writableFinished) controller.abort()
      })
      if (!this.intake) res.set("Connection", "close")
      const query: Record<string, string | string[]> = {}
      for (const [k, v] of Object.entries(req.query)) {
        if (typeof v === "string") query[k] = v
        else if (Array.isArray(v)) query[k] = v.filter((x): x is string => typeof x === "string")
      }
      const headers: Record<string, string> = {}
      for (const [k, v] of Object.entries(req.headers)) {
        if (typeof v === "string") headers[k] = v
        else if (Array.isArray(v)) headers[k] = v.join(", ")
      }
      const params: Record<string, string> = {}
      for (const [k, v] of Object.entries(req.params)) if (typeof v === "string") params[k] = v
      const bodyText =
        typeof req.body === "string" ? req.body : req.body !== null && typeof req.body === "object" && Object.keys(req.body as object).length > 0 ? JSON.stringify(req.body) : undefined
      try {
        const out = await handler({ method, template, params, query, headers, ...(bodyText === undefined ? {} : { bodyText }), signal: controller.signal })
        res.status(out.status)
        for (const [k, v] of Object.entries(out.headers)) res.set(k, v)
        res.send(out.body ?? "")
      } catch (error) {
        // Core answers every error itself; this is only for a bug in an adapter or in core.
        res.status(500).type("application/problem+json").send(problem(500, "Internal", "Internal server error"))
        console.error(error)
      }
    }
    const register = this.raw[method.toLowerCase() as Lowercase<HttpMethod>] as (path: string, ...handlers: RequestHandler[]) => void
    register.call(this.raw, path, parse, run)
  }

  async start(): Promise<void> {
    if (this.ownsApp) {
      this.raw.use((_req: Request, res: Response) => {
        res.status(404).type("application/problem+json").send(problem(404, "NotFound", "No such route"))
      })
      this.raw.use((error: { status?: number; type?: string }, _req: Request, res: Response, _next: NextFunction) => {
        const status = typeof error.status === "number" && error.status >= 400 && error.status < 600 ? error.status : 500
        const detail = error.type === "entity.too.large" ? "The request body is too large" : status >= 500 ? "Internal server error" : "The request could not be read"
        res.status(status).type("application/problem+json").send(problem(status, status === 413 ? "InvalidInput" : "Error", detail))
      })
    }
    const port = this.options.port ?? (typeof this.settings.port === "number" ? this.settings.port : 3000)
    const host = this.options.host ?? (typeof this.settings.host === "string" ? this.settings.host : undefined)
    this.intake = true
    await new Promise<void>((resolve, reject) => {
      const onError = (error: Error): void => reject(error)
      const server = host === undefined ? this.raw.listen(port) : this.raw.listen(port, host)
      server.once("error", onError)
      server.once("listening", () => {
        server.off("error", onError)
        resolve()
      })
      this.server = server
    })
  }

  async stopIntake(): Promise<void> {
    this.intake = false
    const server = this.server
    if (server === undefined) return
    this.closed = new Promise<void>((resolve) => server.close(() => resolve()))
    server.closeIdleConnections()
  }

  async close(): Promise<void> {
    const server = this.server
    if (server === undefined) return
    if (this.closed === undefined) this.closed = new Promise<void>((resolve) => server.close(() => resolve()))
    // A handler that has finished may still be writing its response: close connections as they go idle,
    // and only cut what is left after the grace.
    let done = false
    void this.closed.then(() => {
      done = true
    })
    const until = Date.now() + (this.options.closeGraceMs ?? 2000)
    while (!done && Date.now() < until) {
      server.closeIdleConnections()
      await new Promise((resolve) => setTimeout(resolve, 5))
    }
    server.closeAllConnections()
    await this.closed
    this.server = undefined
  }
}

/** `new ExpressAdapter(options)`, for symmetry with the other adapters. */
export function expressAdapter(options: ExpressAdapterOptions = {}): ExpressAdapter {
  return new ExpressAdapter(options)
}
