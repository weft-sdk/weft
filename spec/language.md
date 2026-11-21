# Contract language specification

Version 0.1. The normative grammar of `.weft` files, as implemented by `weft-syntax`. The guide with examples, the type mapping and the design reasons is [docs/contract-language.md](../docs/contract-language.md). The meaning rules (names, types, operations) that the checker enforces are in the last section.

## Lexical rules

- Source is UTF-8. Whitespace, including newlines, separates tokens and carries no other meaning.
- `// text` is a comment, ignored by the compiler and kept by the formatter. `/// text` is a documentation comment and belongs to the declaration, field, variant or operation that follows it.
- **Identifier:** `[A-Za-z_][A-Za-z0-9_]*`. ASCII only.
- **Keywords are contextual.** `package`, `import`, `enum`, `type`, `event`, `error`, `http`, `rpc`, `graphql`, `socket`, `query`, `mutation`, `subscription`, `server`, `client`, `stream`, `key` and `headers` only mean something where the grammar expects them. Everywhere else they are ordinary names, so `error?: E` and `query: { page?: int }` are valid. The one place a type may not be called `stream` is directly after `->` or in a parameter list.
- **String:** `"..."` on one line. Escapes: `\"`, `\\`, `\n`, `\t`. Any other escape is an error, so a regex digit is written `"\\d"`.
- **Number:** an optional `-`, digits, and an optional `.` followed by digits.
- **Path:** `/` followed by letters, digits and `_ - . / ~ %`, used only for a base path (`http /v1 {`). Route paths are strings, and only they can have `{name}` parameters.
- **Punctuation:** `{ } ( ) [ ] < > : , ? | @ . ->`. A comma between fields, variants, arguments or options is optional.

## Grammar

`X*` is zero or more, `X?` is optional, `A | B` is a choice. `⟨doc⟩` is zero or more `///` lines.

```
file        = package import* decl*
package     = "package" IDENT annotation*
import      = "import" IDENT

decl        = ⟨doc⟩ ( enum | type | event | error | http | rpc | graphql | socket )

enum        = "enum" IDENT annotation* "{" variant* "}"
variant     = ⟨doc⟩ IDENT annotation* ","?
type        = "type" IDENT ( "<" IDENT ("," IDENT)* ","? ">" )? annotation* "{" field* "}"
error       = "error" IDENT annotation* "{" field* "}"
event       = "event" path ":" type-expr ( "{" event-option* "}" )? annotation*
event-option = ( "key" ":" path | "headers" ":" type-expr ) ","?

http        = "http" PATH? "{" route* "}"
route       = ⟨doc⟩ IDENT STRING IDENT params result annotation*
rpc         = "rpc" IDENT "{" method* "}"
method      = ⟨doc⟩ IDENT params result annotation*
graphql     = "graphql" "{" gql-field* "}"
gql-field   = ⟨doc⟩ ( "query" | "mutation" | "subscription" ) IDENT params result annotation*
socket      = "socket" IDENT "{" socket-event* "}"
socket-event = ⟨doc⟩ ( "server" | "client" ) path ":" type-expr annotation*

field       = ⟨doc⟩ IDENT "?"? ":" type-expr annotation* ","?
params      = "(" ( param ("," param)* ","? )? ")"
param       = IDENT "?"? ":" type-expr | "stream"? type-expr
result      = "->" "stream"? result-type ( "|" result-type )*
result-type = type-expr "?"?

type-expr   = ( path ( "<" type-expr ("," type-expr)* ","? ">" )? | "{" field* "}" ) ( "[" "]" )*
path        = IDENT ( "." IDENT )*

annotation  = "@" IDENT ( "(" ( arg ("," arg)* ","? )? ")" )?
arg         = STRING | NUMBER | IDENT
```

## Settled while building the parser

These were open in the guide ("exact grammar, reserved words"). Decided 2026-11-21 in task P1.2.

| Detail | Rule |
| --- | --- |
| Where annotations go | After the thing they describe: after the type of a field, after the name (and type parameters) of a `type`, `enum` or `error`, after the result of an operation, after an event's type and options. `@version` follows the package name |
| Event options | Two, in one braced block after the payload type: `key: <field path>` and `headers: <type>`. The headers type is a set of string-valued fields carried as message headers |
| `?` | `name?: T` on fields and parameters. A trailing `T?` only in a result, where it means the result may be empty |
| Union `\|` | Only in results |
| Parameters | Named (`id: uuid`) or, for `rpc`, a single unnamed type (`(CreateOrder)`). `stream` before an unnamed type marks client streaming |
| HTTP methods | Written as an identifier in the route. The checker accepts `GET POST PUT PATCH DELETE HEAD OPTIONS` |
| Base path | A bare path after `http`, not a string |
| Statement separators | None. Newlines mean nothing |

## Style

`weft fmt` writes one canonical style, and keeps every comment. The formatter is the reference for style.

- Two-space indent. One field, variant, route or method per line. Newlines and commas between items are written as newlines.
- One blank line between top-level declarations. Blank lines elsewhere are kept, at most one in a row, and not at the start or end of a block.
- A `//` comment on its own line stays above the item that follows it. A comment after an item on the same line stays after it, and trailing comments in one block line up two spaces after the longest line.
- `enum` values stay on one line (`enum Currency { EUR GBP USD }`) unless an enum holds comments or docs, or is over 80 characters.
- Inline types are one line (`{ id: uuid, reason: string }`) unless they hold comments or docs.
- In an `http` block the method and path columns line up. A route over 100 characters moves `->` and the result to the next line.
- In a `graphql` block the kind column lines up, and in a `socket` block the event names do.
- Strings are written with the escapes `\"`, `\\`, `\n` and `\t`.
- Formatting never changes what a contract means: the IR before and after is the same.

## Meaning rules

Enforced by the checker (`weft-check`, task P1.4). Every rule has a failing test. The checker reports every error it finds, not only the first.

**Packages**

- Files with the same `package` name are one package. A package may not be called `weft`, which holds the built-in errors.
- A package imports only packages that are among the files being checked. Import cycles are errors.
- `@version` is a semantic version (`MAJOR.MINOR.PATCH`, optionally with `-pre` and `+build`), and at most one value per package.

**Names**

- Types, enums and errors share one namespace per package. The names of the built-in scalars (`string`, `int`, ...), `map`, `stream` and the built-in errors cannot be declared.
- Declarations can be used before they are declared.
- Another package's names are written `package.Name` and need `import package` in the same file. A package's own names are written without the package. Built-in errors may also be written `weft.NotFound`.
- Unknown names get a "did you mean" hint when one is close.

**Types**

- `T<A, B>` needs exactly as many arguments as `T` has parameters, and non-generic types take none. `map<K, V>` takes two, and `K` is `string`, `uuid` or an enum.
- Errors are not types. They appear only after a result's value.
- Inline types are not allowed inside a generic type. Elsewhere they get a generated name from where they are written. Generated names that clash with another type are an error.

| Written in | Generated name |
| --- | --- |
| A field `placedBy` of type `Order` | `OrderPlacedBy` |
| `event order.cancelled: {...}` | `OrderCancelled` |
| Headers of that event | `OrderCancelledHeaders` |
| `query`, `body`, `headers` of route `listOrders` | `ListOrdersQuery`, `ListOrdersBody`, `ListOrdersHeaders` |
| A result of operation `x` | `XResult` |
| Named parameters of `rpc` method `streamUpdates` | `StreamUpdatesRequest` |
| A GraphQL argument `input` of field `placeOrder` | `PlaceOrderInput` |
| Socket event `courier.moved` | `CourierMoved` |

**Annotations**

| Annotation | On | Arguments | Rule |
| --- | --- | --- | --- |
| `@version` | package | `"1.2.3"` | Semantic version |
| `@deprecated` | everything but a package | none, or a message | |
| `@wire` | field | a name | Wire names in one type must differ |
| `@status` | error | `400`–`599` | Defaults to 422 |
| `@min`, `@max` | numeric field | a number | Whole numbers for integer types. `@min` must not exceed `@max` |
| `@length` | string, bytes, list, map | `min, max` | `0 ≤ min ≤ max` |
| `@pattern` | string | a regex | Must compile in the regex subset every language supports: no lookaround and no backreferences. Go's RE2 has neither lookahead nor lookbehind, so lookahead is out too |
| `@format` | string | `email`, `url`, `uri`, `hostname`, `ipv4`, `ipv6`, `date`, `time` | |

An unknown annotation, one in the wrong place, or one used twice is an error.

**Results**

- `-> Value | Error | Error`: the first member is the value, every other member must be an error (built-in, declared here or in an imported package). Only the value may end in `?`. An error cannot be listed twice.
- `stream` results are only for `rpc` methods.

**Events and sockets**

- Event names are unique in a package. The payload, and a socket event's payload, is a record.
- `key` names a field path into the payload. Each field on the path exists and is required, and the last one is a string, whole number, uuid or enum.
- `headers` is a record whose fields are all `string`.
- A socket's event names are unique in that socket.

**HTTP**

- Methods are `GET POST PUT PATCH DELETE HEAD OPTIONS`. Route paths start with `/`, with `{name}` parameters.
- Every `{name}` in the path has a parameter of that name, which is a required scalar (not bytes or a record). Parameters not in the path must be `query`, `body` or `headers`.
- `query` and `headers` are records, none of the three can be optional, and `GET` and `HEAD` take no `body`.
- Route names are unique (they become function names), and so are method and path shapes: `/a/{x}` and `/a/{y}` are the same route.

**RPC and GraphQL**

- Service names are unique. Method names are unique in a service.
- An `rpc` method takes either one unnamed record type (optionally `stream`), or named parameters, which become the record `<Method>Request`. Mixing the two is an error.
- GraphQL arguments are named. A query, a mutation and a subscription may share a name; two of the same kind may not.
