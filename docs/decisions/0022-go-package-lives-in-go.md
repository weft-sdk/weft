# ADR 0022: The Go package lives in `go/` at the repository root

- Status: Accepted (2026-11-23)

**Context.** [ADR 0014](0014-name-weft.md) fixes the Go import path as `github.com/weft-sdk/weft/go` until a domain is chosen. Go resolves a module path to a folder of the repository with the same relative path, so that module must be in `go/`. The [architecture](../09-architecture.md#repo-layout) had put every language under `sdks/`.

**Decision.** The Go module is `github.com/weft-sdk/weft/go`, in `go/` at the repository root. Core is the root package, and adapters are packages inside the same module (`.../go/kafka/franz`, `.../go/http/nethttp`), as [04-sdk-design.md](../04-sdk-design.md#structure) already names them. Every other language stays under `sdks/`. One module means an application that imports only the core and `net/http` does not build franz-go.

**Alternatives considered.**
- *`sdks/go` with the module path `github.com/weft-sdk/weft/sdks/go`.* Keeps the layout tidy, but changes a path that ADR 0014 published as the plan, and every user would import a longer one.
- *A separate repository for Go.* Cleaner module path, but splits the conformance suite and the compiler from the package they test, and it is the maintainer's to create.

**Consequences.**
- [09-architecture.md](../09-architecture.md#repo-layout) shows `go/` beside `sdks/`.
- When a domain is chosen (Q3) the module path changes once, as ADR 0014 already says.
