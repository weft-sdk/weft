# Security policy

## Reporting a vulnerability

Please report security problems privately, not in a public issue.

Use [GitHub's private vulnerability reporting](https://github.com/weft-sdk/weft/security/advisories/new). It reaches the maintainers only, and lets us work on a fix with you before anything is public.

Include what you found, how to reproduce it, which version or commit you tested, and what you think the impact is.

## What to expect

- We acknowledge a report within a few days.
- We tell you whether we can reproduce it, and keep you updated as we fix it.
- We publish a fix and a security advisory together, and credit you if you want.

## Scope

In scope: the `weft` compiler and CLI, the TypeScript and Go packages and their adapters, the conformance runner, and the code Weft generates.

Out of scope: vulnerabilities in the libraries Weft runs underneath (Express, KafkaJS, franz-go and so on) should go to those projects. We will help you reach them if it is not clear who owns the problem.

## Supported versions

Weft is at 0.x. Security fixes go into the latest release.
