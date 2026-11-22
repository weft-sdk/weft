import { describe, expect, it } from "vitest"
import { checkFormat, type FormatName } from "../src/index.js"

const cases: Record<FormatName, { ok: string[]; bad: string[] }> = {
  email: { ok: ["a@example.com", "first.last+tag@sub.example.co"], bad: ["", "a@b", "a b@example.com", "@example.com", "a@@example.com", "a@.com"] },
  url: { ok: ["https://example.com", "http://example.com:8080/a?b=c#d"], bad: ["example.com", "ftp://example.com", "mailto:a@b.co", "https://", ""] },
  uri: { ok: ["urn:isbn:0451450523", "mailto:a@b.co", "https://example.com"], bad: ["example.com", "://x", "a b:c", ""] },
  hostname: { ok: ["example.com", "a", "a-b.example.org", "xn--nxasmq6b.example"], bad: ["", "-a.com", "a-.com", "a..com", "a_b.com", "a".repeat(64) + ".com"] },
  ipv4: { ok: ["0.0.0.0", "192.168.1.255", "10.0.0.1"], bad: ["256.1.1.1", "1.2.3", "1.2.3.4.5", "01.2.3.4", "a.b.c.d", ""] },
  ipv6: {
    ok: ["::1", "::", "2001:db8::8a2e:370:7334", "1:2:3:4:5:6:7:8", "::ffff:192.0.2.1", "1::"],
    bad: ["", ":::", "1:2:3:4:5:6:7", "1:2:3:4:5:6:7:8:9", "12345::", "1::2::3", "::1.2.3", "1.2.3.4::", "g::1"],
  },
  date: { ok: ["2026-11-23", "2024-02-29"], bad: ["2025-02-29", "2026-13-01", "2026-00-10", "2026-11-31", "26-11-23", "2026-1-1", ""] },
  time: { ok: ["10:00:00", "23:59:59", "10:00:00.123", "10:00:00Z", "10:00:00+02:00"], bad: ["24:00:00", "10:60:00", "10:00", "10:00:00+2", ""] },
}

describe("formats", () => {
  for (const [format, { ok, bad }] of Object.entries(cases) as [FormatName, { ok: string[]; bad: string[] }][]) {
    it(`${format} accepts valid values`, () => {
      for (const v of ok) expect(checkFormat(format, v), v).toBe(true)
    })
    it(`${format} rejects invalid values`, () => {
      for (const v of bad) expect(checkFormat(format, v), v).toBe(false)
    })
  }
})
