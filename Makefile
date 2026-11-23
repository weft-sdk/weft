# Everything runs through this file. `make check` is what CI runs.

CARGO := cargo
MANIFEST := --manifest-path compiler/Cargo.toml
PNPM := pnpm --dir website

.DEFAULT_GOAL := help
.PHONY: help check fmt fmt-check lint test test-tsc build weft schema snapshots \
        site-install site-dev site-check site-build \
        ts-install ts-check ts-build ts-test ts-publish-dry-run ts-fixtures ts-kafka-up ts-kafka-down ts-kafka-test conformance conformance-table go-check go-test go-kafka-test go-fixtures

help: ## List the commands
	@grep -hE '^[a-zA-Z_-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  make %-14s %s\n", $$1, $$2}'

check: fmt-check lint test build site-check site-build ts-check go-check ## What CI runs: compiler, website, TypeScript and Go checks (run site-install and ts-install first)

fmt: ## Format the compiler's Rust code
	$(CARGO) fmt $(MANIFEST) --all

fmt-check: ## Check formatting
	$(CARGO) fmt $(MANIFEST) --all --check

lint: ## Clippy, warnings denied
	$(CARGO) clippy $(MANIFEST) --all-targets --locked -- -D warnings

test: ## Run the compiler's tests
	$(CARGO) test $(MANIFEST) --locked

test-tsc: ## Tests, and the code generator's output must type-check (needs: npm i -g typescript)
	WEFT_REQUIRE_TSC=1 $(CARGO) test $(MANIFEST) --locked -p weft-codegen

build: ## Build the compiler workspace
	$(CARGO) build $(MANIFEST) --locked

weft: ## Run the CLI from source: make weft ARGS="check ."
	$(CARGO) run $(MANIFEST) -p weft-cli -- $(ARGS)

schema: ## Regenerate spec/ir.schema.json after changing the IR types
	UPDATE_SCHEMA=1 $(CARGO) test $(MANIFEST) -p weft-ir --test schema

snapshots: ## Accept changed snapshot tests (read the .snap diffs before committing)
	INSTA_UPDATE=always $(CARGO) test $(MANIFEST)

site-install: ## Install the website's dependencies (Node 22.23.4 or later, pnpm 11)
	$(PNPM) install --frozen-lockfile

site-dev: ## Run the website at http://localhost:4321
	$(PNPM) dev

site-check: ## Type-check the website
	$(PNPM) check

site-build: ## Build the static site into website/dist/
	$(PNPM) build

TS := pnpm --dir sdks/typescript

ts-install: ## Install the TypeScript packages' dependencies (Node 22.23.4 or later, pnpm 11)
	$(TS) install --frozen-lockfile

ts-build: ## Build the TypeScript packages (ESM and CJS)
	$(TS) build

ts-test: ## Test the TypeScript packages (run ts-build first)
	$(TS) test

ts-check: ## Build, type-check and test the TypeScript packages (adapters type-check against core's build)
	$(TS) build
	$(TS) typecheck
	$(TS) test

ts-fixtures: ## Regenerate the TypeScript tests' fixture from spec/examples with the real compiler
	cd sdks/typescript/fixtures && $(CARGO) run -q --manifest-path ../../../compiler/Cargo.toml -p weft-cli -- gen
	cd sdks/typescript/packages/e2e && $(CARGO) run -q --manifest-path ../../../../compiler/Cargo.toml -p weft-cli -- gen

ts-kafka-up: ## Start a Kafka broker for the adapter tests (needs Docker)
	docker compose --file sdks/typescript/docker-compose.yaml up -d --wait

ts-kafka-down: ## Stop the test broker
	docker compose --file sdks/typescript/docker-compose.yaml down

ts-kafka-test: ts-build ## Run the Kafka adapter and end-to-end tests against the broker from ts-kafka-up
	WEFT_TEST_KAFKA=127.0.0.1:9092 $(TS) --filter @weft-sdk/kafka-kafkajs --filter weft-e2e test

conformance: ts-build ## Run the conformance cases against every language runner (needs Node and Go; the cross-language Kafka cases run when WEFT_TEST_KAFKA is set)
	$(CARGO) run -q $(MANIFEST) -p weft-conformance -- run

go-check: ## Format check, vet and test the Go module (with the race detector)
	cd go && test -z "$$(gofmt -l .)" && go vet ./... && go test -race ./...

go-test: ## Test the Go module
	cd go && go test ./...

go-kafka-test: ## Run the Go Kafka adapter and end-to-end tests against the broker from ts-kafka-up
	cd go && WEFT_TEST_KAFKA=127.0.0.1:9092 go test -race ./kafka/... ./e2e/...

go-fixtures: ## Regenerate go/e2e/weft.gen.go from spec/examples with the real compiler
	cd go/e2e && $(CARGO) run -q --manifest-path ../../compiler/Cargo.toml -p weft-cli -- gen

conformance-table: ts-build ## Run everything, Kafka included (make ts-kafka-up first), and write conformance/compatibility.md
	WEFT_TEST_KAFKA=127.0.0.1:9092 $(CARGO) run -q $(MANIFEST) -p weft-conformance -- run --require-infra --table conformance/compatibility.md

ts-publish-dry-run: ts-build ## Show what each publishable TypeScript package would put on npm (publishes nothing)
	$(TS) --filter "@weft-sdk/*" exec npm pack --dry-run
