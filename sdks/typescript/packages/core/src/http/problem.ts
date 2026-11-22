import type { Contracts } from "../contracts.js"
import { CATEGORIES, DeclaredError, WeftError, isWeftError, type ErrorCategory, type Issue } from "../errors.js"
import type { RouteDef } from "../ir.js"

/** The RFC 9457 `type` of a built-in category or a declared error. */
export const PROBLEM_TYPE_PREFIX = "urn:weft:error:"

export interface Problem {
  readonly type: string
  readonly title: string
  readonly status: number
  readonly detail?: string
  readonly issues?: readonly Issue[]
  /** The fields of a declared error, in wire form. */
  readonly fields?: unknown
}

export interface ProblemResult {
  readonly status: number
  readonly problem: Problem
  /** Set when this was not a clean, expected error: the server should log it. */
  readonly unexpected?: unknown
}

function category(error: WeftError): ProblemResult {
  const status = error.httpStatus
  return {
    status,
    problem: {
      type: `${PROBLEM_TYPE_PREFIX}${error.category}`,
      title: error.category,
      status,
      // The message of an Internal error may describe the server, so it is not sent.
      detail: error.category === "Internal" ? "Internal server error" : error.message,
      ...(error.issues.length > 0 ? { issues: error.issues } : {}),
    },
  }
}

/** Turns anything a handler threw into the response it deserves. */
export function toProblem(error: unknown, route: RouteDef | undefined, contracts: Contracts): ProblemResult {
  if (isWeftError(error)) {
    const result = category(error)
    return error.category === "Internal" ? { ...result, unexpected: error } : result
  }
  if (error instanceof DeclaredError) {
    const declared = route?.result.errors?.some((e) => e.package === error.package && e.name === error.errorName)
    if (declared === true) {
      try {
        const def = contracts.error(error.package, error.errorName)
        return {
          status: def.status,
          problem: {
            type: `${PROBLEM_TYPE_PREFIX}${error.package}.${error.errorName}`,
            title: error.errorName,
            status: def.status,
            detail: error.message,
            fields: contracts.encodeError(error.package, error.errorName, error.fields),
          },
        }
      } catch (cause) {
        return { ...category(WeftError.internal("A declared error had invalid fields", { cause })), unexpected: cause }
      }
    }
    return {
      ...category(WeftError.internal(`The route does not declare ${error.package}.${error.errorName}`)),
      unexpected: error,
    }
  }
  return { ...category(WeftError.internal("Unexpected error", { cause: error })), unexpected: error }
}

const STATUS_TO_CATEGORY = new Map<number, ErrorCategory>(
  (Object.entries(CATEGORIES) as [ErrorCategory, { http: number }][]).map(([name, info]) => [info.http, name]),
)

/** The error a client raises for an error response. Mirror of `toProblem`. */
export function fromProblem(status: number, body: unknown, contracts: Contracts): WeftError | DeclaredError {
  const problem = typeof body === "object" && body !== null ? (body as Record<string, unknown>) : {}
  const type = typeof problem.type === "string" ? problem.type : ""
  const detail = typeof problem.detail === "string" ? problem.detail : typeof problem.title === "string" ? problem.title : `HTTP ${status}`
  if (type.startsWith(PROBLEM_TYPE_PREFIX)) {
    const name = type.slice(PROBLEM_TYPE_PREFIX.length)
    if (name in CATEGORIES) {
      const issues = Array.isArray(problem.issues) ? (problem.issues as Issue[]) : []
      return new WeftError(name as ErrorCategory, detail, { issues })
    }
    const dot = name.indexOf(".")
    if (dot > 0) {
      const pkg = name.slice(0, dot)
      const errorName = name.slice(dot + 1)
      try {
        return new DeclaredError(pkg, errorName, contracts.decodeError(pkg, errorName, problem.fields) as Record<string, unknown>, detail)
      } catch {
        return WeftError.internal(`The server sent ${name} with fields that do not match the contract`)
      }
    }
  }
  const mapped = STATUS_TO_CATEGORY.get(status) ?? (status >= 500 ? "Internal" : "InvalidInput")
  return new WeftError(mapped, detail)
}
