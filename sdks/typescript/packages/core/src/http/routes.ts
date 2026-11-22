import type { Contracts } from "../contracts.js"
import { WeftError, type Issue } from "../errors.js"
import type { HttpMethod, RouteDef } from "../ir.js"

/** `/v1` + `/orders/{id}`. */
export function fullPath(route: RouteDef): string {
  return `${route.base ?? ""}${route.path}`
}

/**
 * The route a method and path name: by the full path (`/v1/orders`) or, if that is not ambiguous,
 * by the path without its base (`/orders`).
 */
export function findRoute(
  contracts: Contracts,
  method: HttpMethod,
  path: string,
  pkg?: string,
): { readonly package: string; readonly def: RouteDef } {
  const all: { package: string; def: RouteDef }[] = []
  for (const ir of contracts.packages.values()) {
    if (pkg !== undefined && ir.package !== pkg) continue
    for (const def of ir.routes ?? []) if (def.method === method) all.push({ package: ir.package, def })
  }
  const exact = all.filter((r) => fullPath(r.def) === path)
  const found = exact.length > 0 ? exact : all.filter((r) => r.def.path === path)
  if (found.length === 0) throw WeftError.internal(`The contracts have no route ${method} ${path}`)
  if (found.length > 1) throw WeftError.internal(`The route ${method} ${path} is in more than one package: pass the package`)
  return found[0] as { package: string; def: RouteDef }
}

export function summarise(issues: readonly Issue[]): string {
  const first = issues[0]
  if (first === undefined) return "Invalid input"
  const where = first.path === "" ? "" : `${first.path} `
  const more = issues.length > 1 ? ` (and ${issues.length - 1} more)` : ""
  return `Invalid input: ${where}${first.message}${more}`
}
