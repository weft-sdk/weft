import { DEFAULTS } from "../defaults.js"
import { WeftError, isWeftError } from "../errors.js"
import type { HttpMethod } from "../ir.js"
import { backoffMs, sleep as realSleep } from "../messaging/backoff.js"
import { SpanKind, httpMetrics, injectTraceContext, withSpan } from "../telemetry.js"
import { withTimeout } from "../timeout.js"
import { encodeTextRecord } from "./coerce.js"
import type { Http } from "./http.js"
import { fromProblem } from "./problem.js"
import { findRoute, fullPath } from "./routes.js"
import type { CallOptions } from "./types.js"

const IDEMPOTENT = new Set<HttpMethod>(["GET", "HEAD", "PUT", "DELETE", "OPTIONS"])

/** Calls a route of another service, as the contracts describe it. */
export async function callService(
  http: Http,
  service: string,
  method: HttpMethod,
  path: string,
  options: CallOptions,
): Promise<unknown> {
  const contracts = http.contracts
  const base = http.weft.config.services[service]
  if (base === undefined) {
    throw WeftError.internal(`There is no URL for the service "${service}": add it under services: in weft.yaml`)
  }
  const found = findRoute(contracts, method, path, options.package)
  const route = found.def
  const template = fullPath(route)

  // Build the request from the contract. Mistakes here are the caller's, so they are InvalidInput.
  let url = template
  for (const p of route.pathParams ?? []) {
    const value = options.params?.[p.name]
    if (value === undefined) throw WeftError.invalidInput(`Invalid input: the path parameter ${p.name} is required`)
    url = url.replace(`{${p.name}}`, encodeURIComponent(String(contracts.encode(p.type, value))))
  }
  const search = new URLSearchParams()
  if (route.query !== undefined) {
    for (const [k, values] of Object.entries(encodeTextRecord(contracts, route.query, options.query ?? {}))) {
      for (const v of values) search.append(k, v)
    }
  }
  const headers: Record<string, string> = {}
  if (route.headers !== undefined) {
    for (const [k, values] of Object.entries(encodeTextRecord(contracts, route.headers, options.headers ?? {}))) {
      headers[k] = values.join(",")
    }
  } else {
    for (const [k, v] of Object.entries(options.headers ?? {})) headers[k] = String(v)
  }
  let body: string | undefined
  if (route.body !== undefined) {
    body = JSON.stringify(contracts.encode(route.body, options.body))
    headers["content-type"] = "application/json"
  }
  if (options.idempotencyKey !== undefined) headers["idempotency-key"] = options.idempotencyKey
  headers.accept = "application/json, application/problem+json"
  const target = `${base.replace(/\/+$/, "")}${url}${search.size > 0 ? `?${search}` : ""}`

  const timeoutMs = options.timeoutMs ?? DEFAULTS.callTimeoutMs
  const safeToRetry = IDEMPOTENT.has(method) || options.idempotencyKey !== undefined
  const doFetch = http.options.fetch ?? fetch
  const sleep = http.options.sleep ?? realSleep
  const random = http.options.random ?? Math.random
  const started = performance.now()
  let status = 0
  try {
    for (let attempt = 1; ; attempt++) {
      try {
        const result = await withSpan(
          `${method} ${template}`,
          {
            kind: SpanKind.CLIENT,
            attributes: { "http.request.method": method, "http.route": template, "server.address": service, "weft.operation": route.name },
          },
          async (span) => {
            const sent = { ...headers }
            injectTraceContext(sent)
            sent["weft-deadline"] = String(Date.now() + timeoutMs)
            const response = await withTimeout(timeoutMs, async (signal) => {
              try {
                const res = await doFetch(target, { method, headers: sent, ...(body === undefined ? {} : { body }), signal })
                return { status: res.status, text: await res.text() }
              } catch (cause) {
                if (isWeftError(cause) || signal.aborted) throw cause
                throw WeftError.unavailable(`Could not reach ${service}: ${cause instanceof Error ? cause.message : String(cause)}`, { cause })
              }
            })
            status = response.status
            span.setAttribute("http.response.status_code", response.status)
            return response
          },
        )
        if (result.status >= 200 && result.status < 300) {
          if (result.text.trim() === "") {
            if (route.result.optional === true) return undefined
            throw WeftError.internal(`${service} answered ${route.name} with no body`)
          }
          return contracts.decodeJson(route.result.value, result.text)
        }
        let parsed: unknown
        try {
          parsed = JSON.parse(result.text)
        } catch {
          parsed = undefined
        }
        const error = fromProblem(result.status, parsed, contracts)
        if (route.result.optional === true && isWeftError(error) && error.category === "NotFound") return undefined
        throw error
      } catch (error) {
        const retryable = isWeftError(error) && (error.category === "Unavailable" || error.category === "Timeout")
        if (!retryable || !safeToRetry || attempt >= DEFAULTS.calls.attempts) throw error
        await sleep(backoffMs(attempt, {}, random), http.weft.shutdownSignal)
      }
    }
  } finally {
    httpMetrics().clientDuration.record(performance.now() - started, { method, route: template, service, status })
  }
}
