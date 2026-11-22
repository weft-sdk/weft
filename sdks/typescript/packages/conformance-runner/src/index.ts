/**
 * The TypeScript runner for the conformance harness. It speaks spec/conformance-protocol.md on
 * standard input and output, and uses nothing but `@weft-sdk/core`.
 */
import { createInterface } from "node:readline"
import { ConfigError, Contracts, isWeftError, parseConfig, version, type Ir, type TypeRef } from "@weft-sdk/core"

const PROTOCOL = "0.1"

let contracts: Contracts | undefined

function need(): Contracts {
  if (contracts === undefined) throw new Error("no contracts loaded: send `load` first")
  return contracts
}

type Request = { id: number; op: string; [key: string]: unknown }

function handle(request: Request): unknown {
  switch (request.op) {
    case "hello":
      return { language: "typescript", version, protocol: PROTOCOL }
    case "load": {
      const irs = request.ir as Ir[]
      // Wire values are decoded to `bigint` when they do not fit a number, so every integer survives.
      contracts = new Contracts(Object.fromEntries(irs.map((ir) => [ir.package, ir])), { int64: "bigint" })
      return {}
    }
    case "decode": {
      const c = need()
      const type = request.type as TypeRef
      const decoded = c.tryDecode(type, request.wire)
      if (!decoded.ok) return { valid: false, issues: decoded.issues }
      try {
        return { valid: true, canonical: c.encode(type, decoded.value) }
      } catch (error) {
        if (isWeftError(error)) return { valid: false, issues: error.issues }
        throw error
      }
    }
    case "config": {
      try {
        const config = parseConfig(request.yaml as string, request.env as Record<string, string>)
        return { valid: true, config }
      } catch (error) {
        if (error instanceof ConfigError) return { valid: false, problems: error.problems }
        throw error
      }
    }
    default:
      throw new Error(`unknown op ${request.op}`)
  }
}

const lines = createInterface({ input: process.stdin })
lines.on("line", (line) => {
  if (line.trim() === "") return
  let id = -1
  try {
    const request = JSON.parse(line) as Request
    id = request.id
    process.stdout.write(`${JSON.stringify({ id, ok: true, result: handle(request) })}\n`)
  } catch (error) {
    process.stdout.write(`${JSON.stringify({ id, ok: false, error: error instanceof Error ? error.message : String(error) })}\n`)
  }
})
