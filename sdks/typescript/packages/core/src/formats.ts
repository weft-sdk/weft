import type { FormatName } from "./ir.js"

const EMAIL =
  /^[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?(?:\.[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?)+$/
const URI = /^[A-Za-z][A-Za-z0-9+.-]*:\S*$/
const LABEL = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?$/
const DATE = /^(\d{4})-(\d{2})-(\d{2})$/
const TIME = /^([01]\d|2[0-3]):([0-5]\d):([0-5]\d)(\.\d+)?(Z|z|[+-](?:[01]\d|2[0-3]):[0-5]\d)?$/

/** True when `year-month-day` is a real calendar day. */
export function isRealDate(year: number, month: number, day: number): boolean {
  if (month < 1 || month > 12 || day < 1) return false
  const d = new Date(Date.UTC(2000, month - 1, day))
  d.setUTCFullYear(year)
  return d.getUTCFullYear() === year && d.getUTCMonth() === month - 1 && d.getUTCDate() === day
}

export function isIpv4(s: string): boolean {
  const parts = s.split(".")
  if (parts.length !== 4) return false
  return parts.every((p) => /^(0|[1-9]\d{0,2})$/.test(p) && Number(p) <= 255)
}

export function isIpv6(s: string): boolean {
  if (!/^[0-9A-Fa-f:.]+$/.test(s)) return false
  const halves = s.split("::")
  if (halves.length > 2) return false
  const split = (part: string | undefined): string[] => (part === undefined || part === "" ? [] : part.split(":"))
  const head = split(halves[0])
  const rest = halves.length === 2 ? split(halves[1]) : []
  const compressed = halves.length === 2
  // An embedded IPv4 address is only allowed as the very last group.
  const tail = compressed ? rest : head
  let groups = 0
  for (const list of [head, rest]) {
    for (let i = 0; i < list.length; i++) {
      const g = list[i] as string
      const isLastOfTail = list === tail && i === list.length - 1
      if (g.includes(".")) {
        if (!isLastOfTail || !isIpv4(g)) return false
        groups += 2
      } else {
        if (!/^[0-9A-Fa-f]{1,4}$/.test(g)) return false
        groups += 1
      }
    }
  }
  return compressed ? groups <= 7 : groups === 8
}

function isHostname(s: string): boolean {
  if (s.length === 0 || s.length > 253) return false
  return s.split(".").every((label) => LABEL.test(label))
}

function isUrl(s: string): boolean {
  try {
    const u = new URL(s)
    return (u.protocol === "http:" || u.protocol === "https:") && u.hostname !== ""
  } catch {
    return false
  }
}

function isDate(s: string): boolean {
  const m = DATE.exec(s)
  return m !== null && isRealDate(Number(m[1]), Number(m[2]), Number(m[3]))
}

/** Whether `value` is valid for the `@format` named `format`. See spec/behaviour.md. */
export function checkFormat(format: FormatName, value: string): boolean {
  switch (format) {
    case "email":
      return value.length <= 254 && EMAIL.test(value)
    case "url":
      return isUrl(value)
    case "uri":
      return URI.test(value)
    case "hostname":
      return isHostname(value)
    case "ipv4":
      return isIpv4(value)
    case "ipv6":
      return isIpv6(value)
    case "date":
      return isDate(value)
    case "time":
      return TIME.test(value)
  }
}
