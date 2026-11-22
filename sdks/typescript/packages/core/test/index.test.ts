import { describe, expect, it } from "vitest"
import { version } from "../src/index.js"

describe("@weft-sdk/core", () => {
  it("exports its version", () => {
    expect(version).toBe("0.0.0")
  })
})
