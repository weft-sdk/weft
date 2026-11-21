# ADR 0021: Generated code embeds the IR

- Status: Proposed (2026-11-21)

**Context.** [sdk-design.md](../sdk-design.md#generated-code) says `weft gen` writes types, a typed facade, runtime validators and operation metadata. Generating a validator function per type means eleven code generators each reimplement every validation rule, wire-name mapping and encoding detail, and the conformance suite has to prove all of them agree.

**Decision.** Generated code carries the checked [IR](../../spec/ir.md) of each package as a constant, next to the generated types. Validation, wire names, encoding and operation metadata run in the language's core package, reading that constant. Generated code is types plus data. The facade (P2) is generated from the same IR.

**Alternatives considered.**
- *A validator function per type, generated.* Fast at run time and needs no interpreter, but the rules are written once per language in templates, which is the work the shared compiler is meant to avoid.
- *Ship the IR as a separate JSON file.* Needs file loading and a bundler step in every language. A constant in the generated file travels with the code.

**Consequences.**
- The core package of each language implements one validator and one encoder over the IR. Behaviour is the same by construction, and the [conformance suite](../conformance.md) checks that one implementation per language.
- The generated file grows with the contracts, since the IR is in it. It is still one file and deterministic.
- Runtime validation is interpreted, not compiled. If it is too slow for a hot path, a language can add a generated fast path later without changing the contract or the IR.
- [sdk-design.md](../sdk-design.md#generated-code) is updated to say this.
