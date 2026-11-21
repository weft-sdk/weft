# P1.2: Parser

- Branch: `feat/p1-2-parser`
- Task: P1.2 in the roadmap, needs P1.1 (done)
- Started: 2026-11-21 · Last session: 2026-11-21
- Pushed: no

## Goal

`weft-syntax` parses every construct of the contract language into an AST, and bad input gives located errors. Done when the complete example in contract-language.md parses and error cases have snapshot tests.

## Plan

1. Work file, committed first.
2. `spec/language.md`: lexical rules and grammar, with the details contract-language.md left open (reserved words, `headers`, parameters, where annotations go).
3. `weft-syntax`: token + hand-written lexer, AST with spans, chumsky token parser, miette diagnostics.
4. Tests: the complete example, every construct, and snapshot tests (insta) for errors.
5. Roadmap, contract-language.md links, CLAUDE.md if needed. Delete this file in the finishing commit.

## Decisions made on this branch

- Lexer is hand-written, the token parser uses chumsky ([tech-stack](../tech-stack.md)). A hand-written lexer gives exact error spans for strings and paths, and chumsky works on the token stream.
- Keywords are contextual: `error`, `type`, `query` and so on are ordinary identifiers except where a declaration or operation starts. The language spec itself uses `error?: E` as a field.
- Newlines carry no meaning. Fields and variants may be separated by newlines, spaces or commas.
- Details settled here are written in `spec/language.md`.

## Done

- Nothing yet.

## Next step

Step 2: write `spec/language.md`.

## Waiting on

- Nothing.

## How to check

    cargo test --manifest-path compiler/Cargo.toml -p weft-syntax

## Found, not done

- Nothing yet.
