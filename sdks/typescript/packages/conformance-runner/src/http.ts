/**
 * The `http` operation: one request to one route, through core's HTTP handling, with a capturing
 * adapter instead of a socket, so every language is held to the same decoding, problems and statuses.
 */
import {
  Contracts,
  DeclaredError,
  Http,
  Weft,
  WeftError,
  fullPath,
  parseConfig,
  type ErrorCategory,
  type HttpAdapter,
  type HttpRequest,
  type HttpResponse,
  type RouteDef,
} from "@weft-sdk/core"

interface Request {
  route: string
  request?: { params?: Record<string, string>; query?: Record<string, string | string[]>; headers?: Record<string, string>; body?: string | null }
  handler?: {
    returns?: unknown
    returnsRaw?: unknown
    throws?: { category?: string; plain?: boolean; declared?: { package?: string; name: string; fields: unknown; message?: string } }
    status?: number
    headers?: Record<string, string>
  }
}

class Capture implements HttpAdapter {
  readonly name = "http"
  readonly raw = {}
  readonly handlers = new Map<string, (req: HttpRequest) => Promise<HttpResponse>>()
  route(method: string, template: string, handler: (req: HttpRequest) => Promise<HttpResponse>): void {
    this.handlers.set(`${method} ${template}`, handler)
  }
  async start(): Promise<void> {}
  async stopIntake(): Promise<void> {}
  async close(): Promise<void> {}
}

const quiet = { warn: () => undefined, error: () => undefined }

export async function http(contracts: Contracts, req: Request): Promise<Record<string, unknown>> {
  const { package: pkg, def: route } = contracts.route(req.route)
  const weft = new Weft({ contracts, config: parseConfig("service: conformance", {}) })
  const adapter = new Capture()
  const h = new Http(weft, { adapter, logger: quiet })

  let calls = 0
  let seen: unknown = null
  h.route(route.method, fullPath(route), (input) => {
    calls += 1
    seen = encodeInput(contracts, route, input as unknown as Record<string, unknown>)
    const script = req.handler ?? {}
    if (script.status !== undefined) input.ctx.response.status = script.status
    Object.assign(input.ctx.response.headers, script.headers ?? {})
    if (script.throws !== undefined) throw thrown(contracts, pkg, script.throws)
    if (script.returnsRaw !== undefined) return script.returnsRaw
    if (script.returns === undefined || script.returns === null) return undefined
    return contracts.decode(route.result.value, script.returns)
  }, { package: pkg })
  await weft.start()

  const r = req.request ?? {}
  const response = await adapter.handlers.get(`${route.method} ${fullPath(route)}`)!({
    method: route.method,
    template: fullPath(route),
    params: r.params ?? {},
    query: r.query ?? {},
    headers: Object.fromEntries(Object.entries(r.headers ?? {}).map(([k, v]) => [k.toLowerCase(), v])),
    ...(r.body === undefined || r.body === null ? {} : { bodyText: r.body }),
    signal: new AbortController().signal,
  })
  await weft.stop()

  const { "content-type": contentType, ...headers } = response.headers
  let body: unknown = null
  if (response.body !== undefined && response.body !== "") body = JSON.parse(response.body)
  const issues = (body as { issues?: { path: string }[] } | null)?.issues
  return {
    status: response.status,
    contentType: (contentType ?? "").split(";")[0],
    body,
    issues: issues === undefined ? null : issues.map((i) => i.path).sort(),
    headers,
    seen,
    calls,
    leaksSecret: (response.body ?? "").includes("hunter2"),
  }
}

function thrown(contracts: Contracts, pkg: string, t: NonNullable<NonNullable<Request["handler"]>["throws"]>): Error {
  if (t.plain) return new Error("a plain exception with a secret: hunter2")
  if (t.declared !== undefined) {
    const d = t.declared
    const fields = contracts.decodeError(d.package ?? pkg, d.name, d.fields) as Record<string, unknown>
    return new DeclaredError(d.package ?? pkg, d.name, fields, d.message)
  }
  return new WeftError(t.category as ErrorCategory, `failed with ${t.category}`)
}

/** What the handler was given, in wire form, so it can be compared across languages. */
function encodeInput(contracts: Contracts, route: RouteDef, input: Record<string, unknown>): unknown {
  const out: Record<string, unknown> = {}
  const params = (input.params ?? {}) as Record<string, unknown>
  out.params = Object.fromEntries((route.pathParams ?? []).map((p) => [p.name, contracts.encode(p.type, params[p.name])]))
  out.query = route.query === undefined ? null : contracts.encode(route.query, input.query)
  out.headers = route.headers === undefined ? null : contracts.encode(route.headers, input.headers)
  out.body = route.body === undefined ? null : contracts.encode(route.body, input.body)
  return out
}
