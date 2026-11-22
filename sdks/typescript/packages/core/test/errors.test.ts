import { describe, expect, it } from "vitest"
import { CATEGORIES, WeftError, isWeftError, type ErrorCategory } from "../src/index.js"

describe("error categories", () => {
  const table: [ErrorCategory, number, string, string][] = [
    ["NotFound", 404, "NOT_FOUND", "none"],
    ["InvalidInput", 400, "INVALID_ARGUMENT", "dead-letter"],
    ["Unauthorized", 401, "UNAUTHENTICATED", "none"],
    ["Forbidden", 403, "PERMISSION_DENIED", "none"],
    ["Conflict", 409, "ALREADY_EXISTS", "none"],
    ["Unavailable", 503, "UNAVAILABLE", "retry"],
    ["Timeout", 504, "DEADLINE_EXCEEDED", "retry"],
    ["Internal", 500, "INTERNAL", "retry-then-dead-letter"],
  ]

  it("has the eight categories of spec/behaviour.md", () => {
    expect(Object.keys(CATEGORIES).sort()).toEqual(table.map((r) => r[0]).sort())
  })

  it.each(table)("%s maps to HTTP %i, gRPC %s, and %s in messaging", (category, http, grpc, messaging) => {
    const e = new WeftError(category, "x")
    expect(e.httpStatus).toBe(http)
    expect(e.grpcCode).toBe(grpc)
    expect(e.messagingPolicy).toBe(messaging)
  })

  it("has a factory for each category", () => {
    expect(WeftError.notFound().category).toBe("NotFound")
    expect(WeftError.invalidInput().category).toBe("InvalidInput")
    expect(WeftError.unauthorized().category).toBe("Unauthorized")
    expect(WeftError.forbidden().category).toBe("Forbidden")
    expect(WeftError.conflict().category).toBe("Conflict")
    expect(WeftError.unavailable().category).toBe("Unavailable")
    expect(WeftError.timeout().category).toBe("Timeout")
    expect(WeftError.internal().category).toBe("Internal")
  })

  it("keeps the cause and the issues, and is recognised", () => {
    const cause = new Error("boom")
    const e = WeftError.internal("failed", { cause, issues: [{ path: "a", message: "b" }] })
    expect(e.cause).toBe(cause)
    expect(e.issues).toEqual([{ path: "a", message: "b" }])
    expect(isWeftError(e)).toBe(true)
    expect(isWeftError(new Error("x"))).toBe(false)
    expect(e).toBeInstanceOf(Error)
  })
})
