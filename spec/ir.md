# The IR

Version 0.1. The checked, resolved form of one package's contracts, as JSON. Code generators, `weft check`, the dashboard and third-party tools read the IR and never the `.weft` syntax ([architecture](../docs/09-architecture.md#compiler-internals)). The machine-readable description is [ir.schema.json](ir.schema.json), generated from the Rust types in `compiler/crates/weft-ir`. Regenerate it with:

```bash
UPDATE_SCHEMA=1 cargo test --manifest-path compiler/Cargo.toml -p weft-ir --test schema
```

A test fails when the file is out of date.

## Shape

One document per package:

| Key | Holds |
| --- | --- |
| `irVersion` | `"0.1"`. Changes when the JSON shape changes |
| `package`, `version` | The package name, and `@version` if set |
| `imports` | Packages this one imports |
| `types` | Records and enums, including generated ones for inline types |
| `errors` | Errors declared with `error`. Built-in errors are implicit |
| `events` | One per `event` |
| `routes` | Every HTTP route of every `http` block, flattened |
| `rpcServices` | `rpc` blocks with their methods |
| `graphql` | Root fields of every `graphql` block, flattened |
| `sockets` | `socket` blocks with their events |

Empty lists and false flags are left out. Lists keep source order.

## What the checker has already done

A reader of the IR can rely on:

- **Names are resolved.** Every `named` type reference has its package, so `Order` in package `orders` is `{ "package": "orders", "name": "Order" }`. Other packages are referred to, never copied in.
- **Inline types have names.** `event order.cancelled: { id: uuid, reason: string }` becomes a record `OrderCancelled` with `"inline": true`. Naming rules are in the checker's section of [language.md](language.md#meaning-rules).
- **Results are split.** `-> Order | OutOfStock | InvalidInput` is a `value` (`Order`) and `errors` (`orders.OutOfStock`, `weft.InvalidInput`). `-> Order?` sets `optional`. `-> stream Order` sets `stream`.
- **Built-in errors live in the package `weft`**, with the fixed HTTP statuses in `BUILTIN_ERRORS` of the `weft-ir` crate.
- **HTTP parameters are classified.** Path parameters, `query`, `body` and `headers` are separate keys.
- **Wire names are filled in.** `wireName` equals `name` unless `@wire` changed it.
- **Validation is checked and normalised** into `rules`.
- **Everything is valid.** The IR is only produced for contracts without errors.

## Compatibility

`weft check` (task P1.7) compares two IRs of the same package. It relies on names being stable, which is why lists are matched by name and not by position.
