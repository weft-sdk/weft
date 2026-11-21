# Contract language specification

Version 0.1. The normative grammar of `.weft` files, as implemented by `weft-syntax`. The guide with examples, the type mapping and the design reasons is [docs/contract-language.md](../docs/contract-language.md). The meaning rules (names, types, operations) that the checker enforces are in the last section.

## Lexical rules

- Source is UTF-8. Whitespace, including newlines, separates tokens and carries no other meaning.
- `// text` is a comment, ignored by the compiler and kept by the formatter. `/// text` is a documentation comment and belongs to the declaration, field, variant or operation that follows it.
- **Identifier:** `[A-Za-z_][A-Za-z0-9_]*`. ASCII only.
- **Keywords are contextual.** `package`, `import`, `enum`, `type`, `event`, `error`, `http`, `rpc`, `graphql`, `socket`, `query`, `mutation`, `subscription`, `server`, `client`, `stream`, `key` and `headers` only mean something where the grammar expects them. Everywhere else they are ordinary names, so `error?: E` and `query: { page?: int }` are valid. The one place a type may not be called `stream` is directly after `->` or in a parameter list.
- **String:** `"..."` on one line. Escapes: `\"`, `\\`, `\n`, `\t`. Any other escape is an error, so a regex digit is written `"\\d"`.
- **Number:** an optional `-`, digits, and an optional `.` followed by digits.
- **Path:** `/` followed by letters, digits and `_ - . / { } ~ %`, used only for a base path (`http /v1 {`). Route paths are strings.
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

## Meaning rules

Enforced by the checker (task P1.4), not the parser. They are added here as the checker is built.
