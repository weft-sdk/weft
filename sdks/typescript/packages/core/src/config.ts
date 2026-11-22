import { existsSync, readFileSync } from "node:fs"
import { readFile } from "node:fs/promises"
import { dirname, join, resolve } from "node:path"
import { parse } from "yaml"
import { ConfigError } from "./errors.js"

/** The parts of `weft.yaml` the runtime reads (docs/06-cli.md). */
export interface Config {
  readonly service: string
  /** Settings per transport, such as `http: { library: "express", port: 3000 }`. */
  readonly transports: Readonly<Record<string, Readonly<Record<string, unknown>>>>
  /** Event name to real topic or queue name. */
  readonly topics: Readonly<Record<string, string>>
  /** Where other services live, for calls to them. */
  readonly services: Readonly<Record<string, string>>
  readonly telemetry: Readonly<Record<string, string>>
}

export type Env = Readonly<Record<string, string | undefined>>

type Plain = Record<string, unknown>

function isPlain(value: unknown): value is Plain {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

/** Replaces `${NAME}` with the environment variable, collecting a problem for each one that is missing. */
function interpolate(value: unknown, where: string, env: Env, problems: string[]): unknown {
  if (typeof value === "string") {
    return value.replace(/\$\{([A-Za-z_][A-Za-z0-9_]*)\}/g, (_, name: string) => {
      const found = env[name]
      if (found === undefined) {
        problems.push(`the environment variable ${name} is not set (used by ${where})`)
        return ""
      }
      return found
    })
  }
  if (Array.isArray(value)) return value.map((v, i) => interpolate(v, `${where}[${i}]`, env, problems))
  if (isPlain(value)) {
    return Object.fromEntries(
      Object.entries(value).map(([k, v]) => [k, interpolate(v, where === "" ? k : `${where}.${k}`, env, problems)]),
    )
  }
  return value
}

function section(value: unknown, name: string, problems: string[]): Plain {
  if (value === undefined || value === null) return {}
  if (!isPlain(value)) {
    problems.push(`${name} must be a map`)
    return {}
  }
  return value
}

function strings(value: unknown, name: string, problems: string[]): Record<string, string> {
  const out: Record<string, string> = {}
  for (const [k, v] of Object.entries(section(value, name, problems))) {
    if (typeof v === "string") out[k] = v
    else problems.push(`${name}.${k} must be a string`)
  }
  return out
}

const squash = (s: string): string => s.toLowerCase().replaceAll("_", "")

/** Coerces an environment value to the type of the setting it replaces. */
function coerce(existing: unknown, text: string): unknown {
  if (typeof existing === "number") {
    const n = Number(text)
    return Number.isNaN(n) || text.trim() === "" ? text : n
  }
  if (typeof existing === "boolean") return text === "true" || text === "1"
  return text
}

/**
 * Reads `weft.yaml` text. `${VAR}` reads the environment, and `WEFT_<TRANSPORT>_<KEY>` (such as
 * `WEFT_KAFKA_BROKERS`), `WEFT_SERVICES_<NAME>`, `WEFT_TELEMETRY_<KEY>` and `WEFT_SERVICE` override
 * the file. Every problem is reported at once, in a `ConfigError`.
 */
export function parseConfig(text: string, env: Env = {}): Config {
  const problems: string[] = []
  let raw: unknown
  try {
    raw = parse(text)
  } catch (cause) {
    throw new ConfigError([`weft.yaml is not valid YAML: ${cause instanceof Error ? cause.message : String(cause)}`])
  }
  if (raw === null || raw === undefined) raw = {}
  if (!isPlain(raw)) throw new ConfigError(["weft.yaml must be a map at the top level"])

  const doc = interpolate(raw, "", env, problems) as Plain
  let service = typeof doc.service === "string" ? doc.service : ""
  if (doc.service !== undefined && typeof doc.service !== "string") problems.push("service must be a string")

  const transports: Record<string, Plain> = {}
  for (const [name, value] of Object.entries(section(doc.transports, "transports", problems))) {
    if (value === null || value === undefined) transports[name] = {}
    else if (isPlain(value)) transports[name] = { ...value }
    else problems.push(`transports.${name} must be a map`)
  }
  const topics = strings(doc.topics, "topics", problems)
  const services = strings(doc.services, "services", problems)
  const telemetry: Record<string, string> = {}
  for (const [k, v] of Object.entries(section(doc.telemetry, "telemetry", problems))) {
    if (typeof v === "string") telemetry[k] = v
    else if (v !== undefined && v !== null) problems.push(`telemetry.${k} must be a string`)
  }

  // Overrides: the longest transport name that prefixes the variable wins.
  const transportNames = Object.keys(transports).sort((a, b) => b.length - a.length)
  for (const [name, text] of Object.entries(env)) {
    if (text === undefined || !name.startsWith("WEFT_")) continue
    const rest = name.slice("WEFT_".length)
    if (rest === "SERVICE") {
      service = text
    } else if (rest.startsWith("SERVICES_")) {
      services[rest.slice("SERVICES_".length).toLowerCase()] = text
    } else if (rest.startsWith("TELEMETRY_")) {
      const key = rest.slice("TELEMETRY_".length)
      const existing = Object.keys(telemetry).find((k) => squash(k) === squash(key))
      telemetry[existing ?? key.toLowerCase()] = text
    } else {
      const transport = transportNames.find((t) => rest.startsWith(`${t.toUpperCase()}_`))
      if (transport === undefined) continue
      const key = rest.slice(transport.length + 1)
      const settings = transports[transport] as Plain
      const existing = Object.keys(settings).find((k) => squash(k) === squash(key))
      const target = existing ?? key.toLowerCase()
      settings[target] = coerce(settings[target], text)
    }
  }

  if (service === "") problems.push("service is required")
  if (problems.length > 0) throw new ConfigError(problems)
  return { service, transports, topics, services, telemetry }
}

/** Reads `weft.yaml` (or another path) from disk. */
export async function loadConfig(path = "weft.yaml", env: Env = process.env): Promise<Config> {
  let text: string
  try {
    text = await readFile(path, "utf8")
  } catch (cause) {
    throw new ConfigError([`cannot read ${path}: ${cause instanceof Error ? cause.message : String(cause)}`])
  }
  return parseConfig(text, env)
}

/** `weft.yaml` in `from` or the nearest folder above it, or `undefined`. */
export function findConfigFile(from: string = process.cwd()): string | undefined {
  let dir = resolve(from)
  for (;;) {
    const candidate = join(dir, "weft.yaml")
    if (existsSync(candidate)) return candidate
    const parent = dirname(dir)
    if (parent === dir) return undefined
    dir = parent
  }
}

/**
 * Reads `weft.yaml` at once. With no path it looks in the working folder and then each folder
 * above it, as the CLI does. Relative paths are not resolved: they are for the CLI.
 */
export function loadConfigSync(path?: string, env: Env = process.env): Config {
  const file = path ?? findConfigFile()
  if (file === undefined) {
    throw new ConfigError(["no weft.yaml found in the working folder or any folder above it: run `weft init`"])
  }
  let text: string
  try {
    text = readFileSync(file, "utf8")
  } catch (cause) {
    throw new ConfigError([`cannot read ${file}: ${cause instanceof Error ? cause.message : String(cause)}`])
  }
  return parseConfig(text, env)
}

/** The real topic or queue name for an event: the `topics:` entry, else the event name itself. */
export function topicName(config: Config, event: string): string {
  return config.topics[event] ?? event
}

/** The base URL of another service, from `services:`. */
export function serviceUrl(config: Config, service: string): string | undefined {
  return config.services[service]
}
