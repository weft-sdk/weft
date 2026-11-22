import { describe, expect, it } from "vitest"
import { Contracts, WeftError, named, type TypeRef } from "../src/index.js"
import { contracts } from "./fixtures/examples.gen.js"

const c = new Contracts(contracts)
const ID = "123e4567-e89b-42d3-a456-426614174000"
const prim = (name: "int" | "int32" | "int64" | "string" | "bytes" | "timestamp" | "decimal" | "uuid" | "float" | "bool"): TypeRef => ({
  kind: "primitive",
  name,
})

function issuesOf(fn: () => unknown): { path: string; message: string }[] {
  try {
    fn()
  } catch (e) {
    expect(e).toBeInstanceOf(WeftError)
    expect((e as WeftError).category).toBe("InvalidInput")
    return [...(e as WeftError).issues]
  }
  throw new Error("expected an InvalidInput error")
}

describe("orders", () => {
  const Order = named("orders", "Order")
  const order = {
    id: ID,
    customerId: "c-1",
    items: [{ sku: "apple", qty: 2 }],
    total: { amount: 450, currency: "EUR" },
    placedAt: new Date("2026-11-23T10:00:00.000Z"),
  }
  const wire = {
    id: ID,
    customerId: "c-1",
    items: [{ sku: "apple", qty: 2 }],
    total: { amount: 450, currency: "EUR" },
    placedAt: "2026-11-23T10:00:00.000Z",
  }

  it("encodes to the wire form and back", () => {
    expect(c.encode(Order, order)).toEqual(wire)
    expect(c.decode(Order, wire)).toEqual(order)
  })

  it("round-trips through JSON text", () => {
    expect(c.decodeJson(Order, c.encodeJson(Order, order))).toEqual(order)
  })

  it("leaves out an optional field that is missing or null, and keeps a value", () => {
    expect(c.encode(Order, { ...order, note: null })).toEqual(wire)
    expect(c.encode(Order, { ...order, note: "hi" })).toEqual({ ...wire, note: "hi" })
    expect(c.decode(Order, { ...wire, note: null })).toEqual({ ...order, note: null })
  })

  it("ignores fields that are not in the contract", () => {
    expect(c.decode(Order, { ...wire, extra: 1 })).toEqual(order)
    expect(c.encode(Order, { ...order, extra: 1 })).toEqual(wire)
  })

  it("collects every problem with its path", () => {
    const issues = issuesOf(() =>
      c.decode(Order, { id: "nope", items: [{ sku: "a" }, { sku: 1, qty: 1 }], total: { amount: 1.5, currency: "JPY" } }),
    )
    expect(issues.map((i) => i.path).sort()).toEqual(
      ["customerId", "id", "items[0].qty", "items[1].sku", "placedAt", "total.amount", "total.currency"].sort(),
    )
  })

  it("says what is wrong in the error message", () => {
    try {
      c.decode(Order, {})
    } catch (e) {
      expect((e as WeftError).message).toMatch(/^Invalid input: id is required \(and \d+ more\)$/)
    }
  })

  it("rejects non-objects and non-arrays", () => {
    expect(issuesOf(() => c.decode(Order, [1]))[0]?.message).toMatch(/must be an object/)
    expect(issuesOf(() => c.decode(Order, { ...wire, items: {} }))[0]?.message).toMatch(/must be an array/)
    expect(issuesOf(() => c.decode(Order, null))[0]?.message).toMatch(/must be an object, not null/)
  })

  it("validate returns issues without throwing", () => {
    expect(c.validate(Order, order)).toEqual([])
    expect(c.validate(Order, { ...order, id: 5 })).toEqual([{ path: "id", message: expect.stringContaining("UUID") }])
  })

  it("tryDecode returns either the value or the issues", () => {
    expect(c.tryDecode(Order, wire)).toEqual({ ok: true, value: order })
    expect(c.tryDecode(Order, {})).toMatchObject({ ok: false })
  })

  it("rejects text that is not JSON as InvalidInput", () => {
    expect(() => c.decodeJson(Order, "{nope")).toThrow(WeftError)
  })

  it("finds events and routes by name", () => {
    expect(c.event("order.placed", "orders").def.key).toEqual(["id"])
    expect(c.event("page.viewed").package).toBe("kitchen")
    expect(() => c.event("order.placed")).toThrow(/more than one package/)
    expect(c.route("createOrder", "orders").def.method).toBe("POST")
    expect(() => c.event("missing")).toThrow(/no event/)
    expect(() => c.route("missing")).toThrow(/no route/)
  })

  it("codes a declared error's fields", () => {
    expect(c.encodeError("orders", "OutOfStock", { sku: "x", available: 0 })).toEqual({ sku: "x", available: 0 })
    expect(c.decodeError("orders", "OutOfStock", { sku: "x", available: 3 })).toEqual({ sku: "x", available: 3 })
    expect(() => c.decodeError("orders", "OutOfStock", { sku: "x" })).toThrow(WeftError)
  })

  it("throws Internal for a type that does not exist", () => {
    try {
      c.encode(named("orders", "Nope"), {})
    } catch (e) {
      expect((e as WeftError).category).toBe("Internal")
    }
  })
})

describe("kitchen sink", () => {
  const Order = named("kitchen", "Order")
  const wire = {
    id: ID,
    customer_id: "c1",
    email: "a@example.com",
    code: "ABC-12",
    qty: 5,
    tags: { x: ["a", "b"] },
    sides: { Left: 1 },
    nested: { a: 1, b: "AQID" },
    total: "12.50",
    placedAt: "2026-11-23T10:00:00+02:00",
  }

  it("maps wire names, timestamps with offsets, bytes and maps", () => {
    const value = c.decode(Order, wire) as Record<string, unknown>
    expect(value.customerId).toBe("c1")
    expect(value.customer_id).toBeUndefined()
    expect(value.placedAt).toEqual(new Date("2026-11-23T08:00:00.000Z"))
    expect((value.nested as { b: Uint8Array }).b).toEqual(new Uint8Array([1, 2, 3]))
    const back = c.encode(Order, value) as Record<string, unknown>
    expect(back).toEqual({ ...wire, placedAt: "2026-11-23T08:00:00.000Z" })
  })

  it("applies rules: length, format, pattern, min, max", () => {
    const bad = c.tryDecode(Order, {
      ...wire,
      customer_id: "",
      email: "nope",
      code: "abc",
      qty: 101,
      ratio: 0.1,
    })
    expect(bad).toMatchObject({ ok: false })
    const paths = (bad as { issues: { path: string; message: string }[] }).issues.map((i) => i.path).sort()
    expect(paths).toEqual(["code", "customer_id", "email", "qty", "ratio"])
    expect(c.tryDecode(Order, { ...wire, qty: 1, ratio: 0.5 })).toMatchObject({ ok: true })
    expect(c.tryDecode(Order, { ...wire, qty: 0 })).toMatchObject({ ok: false })
    expect(c.tryDecode(Order, { ...wire, customer_id: "x".repeat(41) })).toMatchObject({ ok: false })
  })

  it("counts characters, not UTF-16 units, for @length", () => {
    expect(c.tryDecode(Order, { ...wire, customer_id: "😀".repeat(40) })).toMatchObject({ ok: true })
    expect(c.tryDecode(Order, { ...wire, customer_id: "😀".repeat(41) })).toMatchObject({ ok: false })
  })

  it("checks enum keys of a map", () => {
    const issues = (c.tryDecode(Order, { ...wire, sides: { Up: 1 } }) as { issues: { path: string }[] }).issues
    expect(issues.map((i) => i.path)).toEqual(["sides[Up]"])
  })

  it("rejects a bad decimal, bytes and timestamp", () => {
    for (const patch of [{ total: "12,5" }, { nested: { a: 1, b: "AQI" } }, { placedAt: "2026-02-30T00:00:00Z" }, { placedAt: "yesterday" }]) {
      expect(c.tryDecode(Order, { ...wire, ...patch })).toMatchObject({ ok: false })
    }
  })

  it("substitutes type arguments in generic types", () => {
    const Page = named("kitchen", "Page", [named("kitchen", "Order")])
    expect(c.tryDecode(Page, { items: [wire] })).toMatchObject({ ok: true })
    const bad = c.tryDecode(Page, { items: [{ ...wire, qty: 0 }] }) as { issues: { path: string }[] }
    expect(bad.issues.map((i) => i.path)).toEqual(["items[0].qty"])
    const Pair = named("kitchen", "Pair", [prim("int"), named("kitchen", "Side")])
    expect(c.decode(Pair, { first: 1, second: "Left" })).toEqual({ first: 1, second: "Left" })
    expect(c.tryDecode(Pair, { first: "1.5", second: "Left" })).toMatchObject({ ok: false })
  })

  it("keeps optional timestamps and null", () => {
    expect(c.decode(Order, { ...wire, when: null })).toMatchObject({ when: null })
  })

  it("never lets a map key reach the prototype", () => {
    const Tags = { kind: "map", key: prim("string"), value: prim("int") } as const
    const out = c.decode(Tags, JSON.parse('{"__proto__": 1, "a": 2}')) as Record<string, number>
    expect(Object.keys(out).sort()).toEqual(["__proto__", "a"])
    expect(Object.getPrototypeOf(out)).toBe(Object.prototype)
  })
})

describe("numbers", () => {
  it("uses a number when the value is safe and a string on the wire when it is not", () => {
    expect(c.encode(prim("int"), 5)).toBe(5)
    expect(c.encode(prim("int64"), 9007199254740993n)).toBe("9007199254740993")
    expect(c.decode(prim("int64"), "9007199254740993")).toBe(9007199254740993n)
    expect(c.decode(prim("int64"), "5")).toBe(5)
    expect(c.encode(prim("int"), 2n ** 63n - 1n)).toBe("9223372036854775807")
  })

  it("decodes int and int64 to bigint when asked", () => {
    const big = new Contracts(contracts, { int64: "bigint" })
    expect(big.decode(prim("int"), 5)).toBe(5n)
    expect(big.decode(prim("int32"), 5)).toBe(5)
  })

  it("checks whole numbers and ranges", () => {
    expect(() => c.encode(prim("int"), 1.5)).toThrow(WeftError)
    expect(() => c.encode(prim("int"), 2 ** 53)).toThrow(WeftError)
    expect(() => c.decode(prim("int"), 2 ** 53)).toThrow(WeftError)
    expect(() => c.encode(prim("int32"), 2 ** 31)).toThrow(WeftError)
    expect(c.encode(prim("int32"), 2 ** 31 - 1)).toBe(2 ** 31 - 1)
    expect(() => c.decode(prim("int"), 2n ** 63n)).toThrow(WeftError)
    expect(() => c.decode(prim("int"), "9223372036854775808")).toThrow(WeftError)
    expect(() => c.encode(prim("int"), "5")).toThrow(WeftError)
  })

  it("accepts finite floats only", () => {
    expect(c.encode(prim("float"), 1.5)).toBe(1.5)
    expect(() => c.encode(prim("float"), Number.NaN)).toThrow(WeftError)
    expect(() => c.encode(prim("float"), Number.POSITIVE_INFINITY)).toThrow(WeftError)
    expect(() => c.decode(prim("float"), "1.5")).toThrow(WeftError)
  })
})

describe("timestamps and bytes", () => {
  it("encodes in UTC with milliseconds", () => {
    expect(c.encode(prim("timestamp"), new Date(Date.UTC(2026, 10, 23, 10)))).toBe("2026-11-23T10:00:00.000Z")
    expect(() => c.encode(prim("timestamp"), new Date("nope"))).toThrow(WeftError)
    expect(() => c.encode(prim("timestamp"), "2026-11-23T10:00:00Z")).toThrow(WeftError)
  })

  it("decodes any fraction length, either case, and rejects impossible times", () => {
    expect((c.decode(prim("timestamp"), "2026-11-23T10:00:00.123456z") as Date).toISOString()).toBe("2026-11-23T10:00:00.123Z")
    expect(() => c.decode(prim("timestamp"), "2026-11-23T24:00:00Z")).toThrow(WeftError)
    expect(() => c.decode(prim("timestamp"), "2026-11-23 10:00:00Z")).toThrow(WeftError)
    expect(() => c.decode(prim("timestamp"), "2026-11-23T10:00:00")).toThrow(WeftError)
  })

  it("round-trips a large byte array", () => {
    const bytes = new Uint8Array(100_000).map((_, i) => i % 251)
    expect(c.decode(prim("bytes"), c.encode(prim("bytes"), bytes))).toEqual(bytes)
    expect(c.encode(prim("bytes"), new Uint8Array([1, 2, 3]))).toBe("AQID")
    expect(() => c.encode(prim("bytes"), [1, 2])).toThrow(WeftError)
  })
})
