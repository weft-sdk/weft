# Publishing the TypeScript packages to npm

Only the maintainer does this ([workflow › What only the maintainer does](../14-workflow.md#what-only-the-maintainer-does)): it needs an npm account, and a published version cannot be taken back. Three packages go out: `@weft-sdk/core`, `@weft-sdk/http-express` and `@weft-sdk/kafka-kafkajs`, all at **0.1.0**. The licence is Apache-2.0 ([ADR 0013](../decisions/0013-open-core.md)). The other packages in `sdks/typescript` (`weft-e2e`, `@weft-sdk/conformance-runner`) are private and are never published.

## Once: the account and the scope

1. **An npm account** at npmjs.com, with **two-factor authentication on** (npm asks for it when you publish).
2. **The `weft-sdk` organisation.** On npmjs.com choose *Add Organization*, name it `weft-sdk`, and pick the free plan (public packages). The organisation *is* the scope: it makes `@weft-sdk/...` yours. If the name is taken, stop and decide a new one with [ADR 0014](../decisions/0014-name-weft.md); do not publish under another scope without a record.
3. **Log in on your machine:** `npm login` (it opens a browser). Check with `npm whoami`.

## Before every release

```
make ts-install          # a clean install
make ts-check            # build, type-check and test
make ts-publish-dry-run  # builds, then shows exactly what each package would publish
```

The dry run lists the files of each tarball (`dist/`, `README.md`, `LICENSE`, `package.json`) and nothing else. If anything is missing or unexpected, fix it first.

## Publish

From `sdks/typescript`, **core first**, because the adapters depend on it:

```
pnpm --filter @weft-sdk/core publish --access public
pnpm --filter @weft-sdk/http-express publish --access public
pnpm --filter @weft-sdk/kafka-kafkajs publish --access public
```

Do it from a clean checkout of `main`: pnpm refuses to publish from another branch ("publish-branch") or with uncommitted changes (`ERR_PNPM_GIT_UNCLEAN`). Do not turn those checks off.

`pnpm publish` runs the build first (`prepublishOnly`), replaces `workspace:^` in the adapters' peer dependencies with `^0.1.0`, and asks for your one-time password. `--access public` is needed the first time, because scoped packages default to private.

**New packages are reviewed first.** For a package that has never been published, npm shows a placeholder version (`0.0.0-stage`, "Temporary Holding Version") while it reviews the real one. The terminal still says `✅ Published`. It took from a few minutes to under half an hour for the first three. Publish `core` first and wait until `npm view @weft-sdk/core version` prints `0.1.0` before relying on it; do not publish again while you wait.

## Check it worked

```
mkdir /tmp/try && cd /tmp/try && npm init -y
npm install @weft-sdk/core @weft-sdk/http-express @weft-sdk/kafka-kafkajs express kafkajs
node -e "import('@weft-sdk/core').then(m => console.log(m.version))"   # 0.1.0
```

Then `weft init --yes --http express --kafka kafkajs` in a project folder installs the same packages for you.

## Next releases

Change the version in each `package.json` (and `version` in `packages/core/src/version.ts`: a test fails if they differ), update [the roadmap](../12-roadmap.md), then run the steps above. Weft is pre-release: before 1.0 a minor version may change the API. Once a version is published, never reuse its number.

## Not covered here

- The **Go module** is released by pushing a tag: `git tag go/v0.1.1 && git push origin go/v0.1.1` (the module is in the `go/` folder, so its tags carry that prefix). `go/v0.1.0` was pushed on 2026-11-23, and `weft init` in a clean Go module builds against it. Never move or reuse a tag: the Go proxy caches it.
- The **`weft` binary** (cargo-dist, Homebrew, `npx @weft-sdk/cli`) is a separate piece of work.
