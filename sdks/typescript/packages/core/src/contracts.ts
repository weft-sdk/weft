import { WeftError, type Issue } from "./errors.js"
import { checkFormat, isRealDate } from "./formats.js"
import type { ErrorDef, EventDef, FieldDef, Ir, PrimitiveName, RouteDef, Rule, TypeDef, TypeRef } from "./ir.js"

export interface ContractsOptions {
  /** How `int` and `int64` values are decoded. `"bigint"` always gives a bigint. Default `"number"`. */
  readonly int64?: "number" | "bigint"
}

/** Shorthand for a reference to a named type. */
export function named(pkg: string, name: string, args: readonly TypeRef[] = []): TypeRef {
  return args.length === 0 ? { kind: "named", package: pkg, name } : { kind: "named", package: pkg, name, args }
}

type Mode = "encode" | "decode"
type Env = ReadonlyMap<string, TypeRef>

const NO_ENV: Env = new Map()
const UUID = /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/
const DECIMAL = /^-?\d+(\.\d+)?$/
const WHOLE = /^-?\d+$/
const BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/
const TIMESTAMP = /^(\d{4})-(\d{2})-(\d{2})[Tt](\d{2}):(\d{2}):(\d{2})(\.\d+)?([Zz]|[+-]\d{2}:\d{2})$/

const INT_RANGE: Readonly<Record<"int32" | "int" | "int64", readonly [bigint, bigint]>> = {
  int32: [-(2n ** 31n), 2n ** 31n - 1n],
  int: [-(2n ** 63n), 2n ** 63n - 1n],
  int64: [-(2n ** 63n), 2n ** 63n - 1n],
}

const regexCache = new Map<string, RegExp>()

function regex(source: string): RegExp {
  let re = regexCache.get(source)
  if (re === undefined) {
    re = new RegExp(source, "u")
    regexCache.set(source, re)
  }
  return re
}

function join(path: string, key: string): string {
  return path === "" ? key : `${path}.${key}`
}

function isObject(value: unknown): value is Record<string, unknown> {
  return Object.prototype.toString.call(value) === "[object Object]"
}

function own(obj: Record<string, unknown>, key: string): unknown {
  return Object.hasOwn(obj, key) ? obj[key] : undefined
}

function setOwn(obj: Record<string, unknown>, key: string, value: unknown): void {
  Object.defineProperty(obj, key, { value, enumerable: true, writable: true, configurable: true })
}

function describe(value: unknown): string {
  if (value === null) return "null"
  if (Array.isArray(value)) return "an array"
  if (value instanceof Date) return "a Date"
  if (value instanceof Uint8Array) return "bytes"
  return `a ${typeof value}`
}

function toBase64(bytes: Uint8Array): string {
  let binary = ""
  for (let i = 0; i < bytes.length; i += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000))
  }
  return btoa(binary)
}

function fromBase64(text: string): Uint8Array {
  const binary = atob(text)
  const out = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i++) out[i] = binary.charCodeAt(i)
  return out
}

function summary(issues: readonly Issue[]): string {
  const first = issues[0]
  if (first === undefined) return "Invalid input"
  const where = first.path === "" ? "" : `${first.path} `
  const more = issues.length > 1 ? ` (and ${issues.length - 1} more)` : ""
  return `Invalid input: ${where}${first.message}${more}`
}

/**
 * The checked contracts of one or more packages, and the one validator and codec that read them
 * ([ADR 0021](../../../../docs/decisions/0021-generated-code-embeds-the-ir.md)). `encode` turns a value as a handler holds
 * it into the JSON-compatible form that travels; `decode` does the reverse. Both validate, and both
 * collect every problem before failing with an `InvalidInput` error. The format is in `spec/behaviour.md`.
 */
export class Contracts {
  readonly packages: ReadonlyMap<string, Ir>
  private readonly types = new Map<string, TypeDef>()
  private readonly errorDefs = new Map<string, ErrorDef>()
  private readonly options: Required<ContractsOptions>

  constructor(irs: Readonly<Record<string, Ir>>, options: ContractsOptions = {}) {
    this.options = { int64: options.int64 ?? "number" }
    const packages = new Map<string, Ir>()
    for (const ir of Object.values(irs)) {
      packages.set(ir.package, ir)
      for (const t of ir.types ?? []) this.types.set(`${ir.package}.${t.name}`, t)
      for (const e of ir.errors ?? []) this.errorDefs.set(`${ir.package}.${e.name}`, e)
    }
    this.packages = packages
  }

  /** A declared type. Throws `Internal` when it does not exist: that is a bug, not bad input. */
  type(pkg: string, name: string): TypeDef {
    const t = this.types.get(`${pkg}.${name}`)
    if (t === undefined) throw WeftError.internal(`The contracts have no type ${pkg}.${name}`)
    return t
  }

  /** A declared error. */
  error(pkg: string, name: string): ErrorDef {
    const e = this.errorDefs.get(`${pkg}.${name}`)
    if (e === undefined) throw WeftError.internal(`The contracts have no error ${pkg}.${name}`)
    return e
  }

  /** An event by name, such as `order.placed`. Pass `pkg` if two packages declare the same name. */
  event(name: string, pkg?: string): { readonly package: string; readonly def: EventDef } {
    const found: { package: string; def: EventDef }[] = []
    for (const ir of this.packages.values()) {
      if (pkg !== undefined && ir.package !== pkg) continue
      for (const def of ir.events ?? []) if (def.name === name) found.push({ package: ir.package, def })
    }
    if (found.length === 0) throw WeftError.internal(`The contracts have no event ${name}`)
    if (found.length > 1) throw WeftError.internal(`The event ${name} is in more than one package: pass the package`)
    return found[0] as { package: string; def: EventDef }
  }

  /** A route by operation name, such as `createOrder`. */
  route(name: string, pkg?: string): { readonly package: string; readonly def: RouteDef } {
    const found: { package: string; def: RouteDef }[] = []
    for (const ir of this.packages.values()) {
      if (pkg !== undefined && ir.package !== pkg) continue
      for (const def of ir.routes ?? []) if (def.name === name) found.push({ package: ir.package, def })
    }
    if (found.length === 0) throw WeftError.internal(`The contracts have no route ${name}`)
    if (found.length > 1) throw WeftError.internal(`The route ${name} is in more than one package: pass the package`)
    return found[0] as { package: string; def: RouteDef }
  }

  /** Problems with a value in code form. Empty when it is valid. */
  validate(type: TypeRef, value: unknown): Issue[] {
    const issues: Issue[] = []
    this.convert(type, value, "", "encode", NO_ENV, issues)
    return issues
  }

  /** Code form to wire form. Throws `InvalidInput` listing every problem. */
  encode(type: TypeRef, value: unknown): unknown {
    const issues: Issue[] = []
    const out = this.convert(type, value, "", "encode", NO_ENV, issues)
    if (issues.length > 0) throw WeftError.invalidInput(summary(issues), { issues })
    return out
  }

  /** Wire form to code form. Throws `InvalidInput` listing every problem. */
  decode(type: TypeRef, wire: unknown): unknown {
    const issues: Issue[] = []
    const out = this.convert(type, wire, "", "decode", NO_ENV, issues)
    if (issues.length > 0) throw WeftError.invalidInput(summary(issues), { issues })
    return out
  }

  /** Like `decode`, but returns the problems instead of throwing. */
  tryDecode(type: TypeRef, wire: unknown): { ok: true; value: unknown } | { ok: false; issues: Issue[] } {
    const issues: Issue[] = []
    const value = this.convert(type, wire, "", "decode", NO_ENV, issues)
    return issues.length > 0 ? { ok: false, issues } : { ok: true, value }
  }

  /** Code form to a JSON string. */
  encodeJson(type: TypeRef, value: unknown): string {
    return JSON.stringify(this.encode(type, value))
  }

  /** A JSON string to code form. Text that is not JSON is `InvalidInput`. */
  decodeJson(type: TypeRef, text: string): unknown {
    let wire: unknown
    try {
      wire = JSON.parse(text)
    } catch (cause) {
      throw WeftError.invalidInput("Invalid input: the body is not valid JSON", { cause })
    }
    return this.decode(type, wire)
  }

  /** The fields of a declared error, in wire form. */
  encodeError(pkg: string, name: string, value: unknown): unknown {
    const def = this.error(pkg, name)
    const issues: Issue[] = []
    const out = this.convertFields(def.fields, value, "", "encode", NO_ENV, issues)
    if (issues.length > 0) throw WeftError.invalidInput(summary(issues), { issues })
    return out
  }

  /** The fields of a declared error, from wire form. */
  decodeError(pkg: string, name: string, wire: unknown): unknown {
    const def = this.error(pkg, name)
    const issues: Issue[] = []
    const out = this.convertFields(def.fields, wire, "", "decode", NO_ENV, issues)
    if (issues.length > 0) throw WeftError.invalidInput(summary(issues), { issues })
    return out
  }

  // ---- the walker ----

  private subst(ref: TypeRef, env: Env): TypeRef {
    switch (ref.kind) {
      case "primitive":
        return ref
      case "param":
        return env.get(ref.name) ?? ref
      case "list":
        return { kind: "list", item: this.subst(ref.item, env) }
      case "map":
        return { kind: "map", key: this.subst(ref.key, env), value: this.subst(ref.value, env) }
      case "named":
        return ref.args === undefined || ref.args.length === 0
          ? ref
          : { ...ref, args: ref.args.map((a) => this.subst(a, env)) }
    }
  }

  private convert(ref: TypeRef, value: unknown, path: string, mode: Mode, env: Env, issues: Issue[]): unknown {
    const fail = (message: string): undefined => {
      issues.push({ path, message })
      return undefined
    }
    switch (ref.kind) {
      case "primitive":
        return this.primitive(ref.name, value, mode, fail)
      case "param": {
        const bound = env.get(ref.name)
        if (bound === undefined) throw WeftError.internal(`The type parameter ${ref.name} has no argument`)
        return this.convert(bound, value, path, mode, NO_ENV, issues)
      }
      case "list": {
        if (!Array.isArray(value)) return fail(`must be an array, not ${describe(value)}`)
        return value.map((item, i) => this.convert(ref.item, item, `${path}[${i}]`, mode, env, issues))
      }
      case "map": {
        if (!isObject(value)) return fail(`must be an object, not ${describe(value)}`)
        const keyRef = this.subst(ref.key, env)
        const out: [string, unknown][] = []
        for (const [k, v] of Object.entries(value)) {
          const keyPath = `${path}[${k}]`
          this.convert(keyRef, k, keyPath, "decode", env, issues)
          out.push([k, this.convert(ref.value, v, keyPath, mode, env, issues)])
        }
        return Object.fromEntries(out)
      }
      case "named": {
        const def = this.type(ref.package, ref.name)
        const args = (ref.args ?? []).map((a) => this.subst(a, env))
        const inner: Env = new Map((def.params ?? []).map((p, i) => [p, args[i] as TypeRef]))
        if (def.kind === "enum") {
          if (typeof value !== "string") return fail(`must be one of ${def.variants.map((v) => v.name).join(", ")}`)
          if (!def.variants.some((v) => v.name === value)) {
            return fail(`${JSON.stringify(value)} is not one of ${def.variants.map((v) => v.name).join(", ")}`)
          }
          return value
        }
        return this.convertFields(def.fields, value, path, mode, inner, issues)
      }
    }
  }

  private convertFields(
    fields: readonly FieldDef[],
    value: unknown,
    path: string,
    mode: Mode,
    env: Env,
    issues: Issue[],
  ): unknown {
    if (!isObject(value)) {
      issues.push({ path, message: `must be an object, not ${describe(value)}` })
      return undefined
    }
    const out: Record<string, unknown> = {}
    for (const f of fields) {
      const from = mode === "decode" ? f.wireName : f.name
      const to = mode === "decode" ? f.name : f.wireName
      const fieldPath = join(path, from)
      const raw = own(value, from)
      if (raw === undefined || raw === null) {
        if (!f.optional) issues.push({ path: fieldPath, message: "is required" })
        else if (raw === null && mode === "decode") setOwn(out, to, null)
        continue
      }
      const before = issues.length
      const converted = this.convert(this.subst(f.type, env), raw, fieldPath, mode, env, issues)
      // Rules read the code form: what was decoded, or what the caller gave to encode.
      if (issues.length === before) this.applyRules(f.rules ?? [], mode === "decode" ? converted : raw, fieldPath, issues)
      setOwn(out, to, converted)
    }
    return out
  }

  private primitive(name: PrimitiveName, value: unknown, mode: Mode, fail: (message: string) => undefined): unknown {
    switch (name) {
      case "string":
        return typeof value === "string" ? value : fail(`must be a string, not ${describe(value)}`)
      case "bool":
        return typeof value === "boolean" ? value : fail(`must be a boolean, not ${describe(value)}`)
      case "float":
      case "float32":
      case "float64":
        return typeof value === "number" && Number.isFinite(value) ? value : fail(`must be a finite number, not ${describe(value)}`)
      case "decimal":
        return typeof value === "string" && DECIMAL.test(value)
          ? value
          : fail('must be a decimal string such as "12.50"')
      case "uuid":
        return typeof value === "string" && UUID.test(value) ? value : fail("must be a UUID")
      case "int":
      case "int32":
      case "int64":
        return this.integer(name, value, mode, fail)
      case "timestamp":
        return this.timestamp(value, mode, fail)
      case "bytes":
        if (mode === "encode") {
          return value instanceof Uint8Array ? toBase64(value) : fail(`must be a Uint8Array, not ${describe(value)}`)
        }
        return typeof value === "string" && BASE64.test(value) ? fromBase64(value) : fail("must be a base64 string")
    }
  }

  private integer(
    name: "int" | "int32" | "int64",
    value: unknown,
    mode: Mode,
    fail: (message: string) => undefined,
  ): unknown {
    let big: bigint
    if (typeof value === "bigint") {
      big = value
    } else if (typeof value === "number") {
      if (!Number.isSafeInteger(value)) {
        return fail(
          mode === "decode"
            ? "must be a whole number up to 2^53 - 1 (larger values are sent as strings)"
            : "must be a whole number up to 2^53 - 1 (use a bigint for larger values)",
        )
      }
      big = BigInt(value)
    } else if (mode === "decode" && typeof value === "string" && WHOLE.test(value)) {
      big = BigInt(value)
    } else {
      return fail(`must be a whole number, not ${describe(value)}`)
    }
    const [min, max] = INT_RANGE[name]
    if (big < min || big > max) return fail(`must be between ${min} and ${max}`)
    const safe = big >= BigInt(Number.MIN_SAFE_INTEGER) && big <= BigInt(Number.MAX_SAFE_INTEGER)
    if (mode === "encode") return safe ? Number(big) : big.toString()
    if (name !== "int32" && this.options.int64 === "bigint") return big
    return safe ? Number(big) : big
  }

  private timestamp(value: unknown, mode: Mode, fail: (message: string) => undefined): unknown {
    if (mode === "encode") {
      if (!(value instanceof Date)) return fail(`must be a Date, not ${describe(value)}`)
      if (Number.isNaN(value.getTime())) return fail("must be a valid Date")
      return value.toISOString()
    }
    const m = typeof value === "string" ? TIMESTAMP.exec(value) : null
    if (m === null) return fail("must be an RFC 3339 timestamp such as 2026-11-23T10:00:00Z")
    const [year, month, day, hour, minute, second] = [m[1], m[2], m[3], m[4], m[5], m[6]].map(Number) as [
      number, number, number, number, number, number,
    ]
    if (!isRealDate(year, month, day) || hour > 23 || minute > 59 || second > 59) {
      return fail("must be a real date and time")
    }
    const ms = Date.parse(`${m[1]}-${m[2]}-${m[3]}T${m[4]}:${m[5]}:${m[6]}${m[7] ?? ""}${(m[8] as string).toUpperCase()}`)
    return Number.isNaN(ms) ? fail("must be a real date and time") : new Date(ms)
  }

  private applyRules(rules: readonly Rule[], value: unknown, path: string, issues: Issue[]): void {
    for (const rule of rules) {
      const fail = (message: string): void => {
        issues.push({ path, message })
      }
      switch (rule.rule) {
        case "min":
        case "max": {
          const n = typeof value === "number" || typeof value === "bigint" ? Number(value) : typeof value === "string" ? Number(value) : NaN
          if (Number.isNaN(n)) break
          const limit = Number(rule.value)
          if (rule.rule === "min" && n < limit) fail(`must be at least ${rule.value}`)
          if (rule.rule === "max" && n > limit) fail(`must be at most ${rule.value}`)
          break
        }
        case "length": {
          let size: number
          let unit: string
          if (typeof value === "string") {
            size = Array.from(value).length
            unit = "characters"
          } else if (value instanceof Uint8Array) {
            size = value.byteLength
            unit = "bytes"
          } else if (Array.isArray(value)) {
            size = value.length
            unit = "items"
          } else if (isObject(value)) {
            size = Object.keys(value).length
            unit = "entries"
          } else break
          if (size < rule.min || size > rule.max) fail(`must have between ${rule.min} and ${rule.max} ${unit}, not ${size}`)
          break
        }
        case "pattern":
          if (typeof value === "string" && !regex(rule.regex).test(value)) fail(`must match the pattern ${rule.regex}`)
          break
        case "format":
          if (typeof value === "string" && !checkFormat(rule.format, value)) fail(`must be a valid ${rule.format}`)
          break
      }
    }
  }
}
