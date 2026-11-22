import type { Contracts } from "../contracts.js"
import type { Issue } from "../errors.js"
import type { TypeRef } from "../ir.js"

/** Query strings, path parameters and headers are text. This turns the text into the wire form its type expects. */
export function coerceText(type: TypeRef, value: unknown): unknown {
  if (Array.isArray(value)) return value.map((v) => coerceText(type.kind === "list" ? type.item : type, v))
  if (typeof value !== "string") return value
  if (type.kind === "list") return [coerceText(type.item, value)]
  if (type.kind !== "primitive") return value
  switch (type.name) {
    case "bool":
      return value === "true" ? true : value === "false" ? false : value
    case "float":
    case "float32":
    case "float64": {
      const n = Number(value)
      return value.trim() === "" || Number.isNaN(n) ? value : n
    }
    default:
      return value
  }
}

/**
 * Decodes a record whose values came from text (a query string, headers). `lookup` finds a field's
 * raw value by its wire name. Issues are returned with `prefix` in front of their paths.
 */
export function decodeTextRecord(
  contracts: Contracts,
  type: TypeRef,
  lookup: (wireName: string) => unknown,
  prefix: string,
): { value: unknown; issues: Issue[] } {
  if (type.kind !== "named") return { value: undefined, issues: [] }
  const def = contracts.type(type.package, type.name)
  const wire: Record<string, unknown> = {}
  if (def.kind === "record") {
    for (const f of def.fields) {
      const raw = lookup(f.wireName)
      if (raw !== undefined) wire[f.wireName] = coerceText(f.type, raw)
    }
  }
  const result = contracts.tryDecode(type, wire)
  if (result.ok) return { value: result.value, issues: [] }
  return { value: undefined, issues: result.issues.map((i) => ({ ...i, path: i.path === "" ? prefix : `${prefix}.${i.path}` })) }
}

/** The inverse, for a client: a record in code form to the text values of a query string or headers. */
export function encodeTextRecord(contracts: Contracts, type: TypeRef, value: unknown): Record<string, string[]> {
  const wire = contracts.encode(type, value) as Record<string, unknown>
  const out: Record<string, string[]> = {}
  for (const [k, v] of Object.entries(wire)) {
    if (v === undefined || v === null) continue
    out[k] = (Array.isArray(v) ? v : [v]).map((x) => (typeof x === "string" ? x : JSON.stringify(x)))
  }
  return out
}
