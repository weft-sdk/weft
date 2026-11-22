/**
 * The checked contracts (the IR), as TypeScript types. This mirrors `spec/ir.md`; the generated
 * `weft.gen.ts` embeds one document per package, and the runtime reads them.
 */

export type PrimitiveName =
  | "string"
  | "bool"
  | "int"
  | "int32"
  | "int64"
  | "float"
  | "float32"
  | "float64"
  | "decimal"
  | "uuid"
  | "timestamp"
  | "bytes"

export type TypeRef =
  | { readonly kind: "primitive"; readonly name: PrimitiveName }
  | { readonly kind: "named"; readonly package: string; readonly name: string; readonly args?: readonly TypeRef[] }
  | { readonly kind: "param"; readonly name: string }
  | { readonly kind: "list"; readonly item: TypeRef }
  | { readonly kind: "map"; readonly key: TypeRef; readonly value: TypeRef }

export type FormatName = "email" | "url" | "uri" | "hostname" | "ipv4" | "ipv6" | "date" | "time"

export type Rule =
  | { readonly rule: "min"; readonly value: string }
  | { readonly rule: "max"; readonly value: string }
  | { readonly rule: "length"; readonly min: number; readonly max: number }
  | { readonly rule: "pattern"; readonly regex: string }
  | { readonly rule: "format"; readonly format: FormatName }

export interface FieldDef {
  readonly name: string
  readonly wireName: string
  readonly type: TypeRef
  readonly optional?: boolean
  readonly doc?: readonly string[]
  readonly deprecated?: string
  readonly rules?: readonly Rule[]
}

export interface VariantDef {
  readonly name: string
  readonly doc?: readonly string[]
  readonly deprecated?: string
}

export type TypeDef = {
  readonly name: string
  readonly doc?: readonly string[]
  readonly params?: readonly string[]
  readonly inline?: boolean
  readonly deprecated?: string
} & (
  | { readonly kind: "record"; readonly fields: readonly FieldDef[] }
  | { readonly kind: "enum"; readonly variants: readonly VariantDef[] }
)

export interface ErrorDef {
  readonly name: string
  readonly doc?: readonly string[]
  readonly status: number
  readonly fields: readonly FieldDef[]
}

export interface ErrorRef {
  readonly package: string
  readonly name: string
}

export interface ResultDef {
  readonly value: TypeRef
  readonly optional?: boolean
  readonly stream?: boolean
  readonly errors?: readonly ErrorRef[]
}

export interface ParamDef {
  readonly name: string
  readonly type: TypeRef
  readonly optional?: boolean
}

export interface EventDef {
  readonly name: string
  readonly doc?: readonly string[]
  readonly payload: TypeRef
  readonly key?: readonly string[]
  readonly headers?: TypeRef
  readonly deprecated?: string
}

export type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"

export interface RouteDef {
  readonly name: string
  readonly doc?: readonly string[]
  readonly method: HttpMethod
  readonly base?: string
  readonly path: string
  readonly pathParams?: readonly ParamDef[]
  readonly query?: TypeRef
  readonly body?: TypeRef
  readonly headers?: TypeRef
  readonly result: ResultDef
  readonly deprecated?: string
}

/** One package's contracts. Operations other than events and routes are read by later tasks. */
export interface Ir {
  readonly irVersion: string
  readonly package: string
  readonly version?: string
  readonly imports?: readonly string[]
  readonly types?: readonly TypeDef[]
  readonly errors?: readonly ErrorDef[]
  readonly events?: readonly EventDef[]
  readonly routes?: readonly RouteDef[]
  readonly rpcServices?: readonly unknown[]
  readonly graphql?: readonly unknown[]
  readonly sockets?: readonly unknown[]
}

/** The package `weft` holds the built-in errors. */
export const BUILTIN_PACKAGE = "weft"
