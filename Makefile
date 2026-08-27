.PHONY: build test clippy fmt lint check run run-sample install clean coverage coverage-ci help npm-test npm-stage npm-pack npm-test-local

BINARY := restui
SAMPLE := examples/sample.http

help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'

build: ## Build in debug mode
	cargo build

build-release: ## Build in release mode
	cargo build --release

test: ## Run all tests
	cargo test

test-verbose: ## Run all tests with verbose output
	cargo test -- --nocapture

clippy: ## Run clippy (treat warnings as errors)
	cargo clippy -- -D warnings

fmt: ## Format code
	cargo fmt

fmt-check: ## Check code formatting
	cargo fmt --check

lint: fmt clippy ## Run formatter and clippy

check: ## Quick compile check
	cargo check

run: ## Run with local-apis.http
	cargo run -- $(SAMPLE)

run-file: ## Run the binary (requires FILE=...)
	cargo run -- $(FILE)

install: ## Install binary to ~/.cargo/bin
	cargo install --path .

coverage: ## Run test coverage (requires cargo-tarpaulin)
	cargo tarpaulin --fail-under 60 --out Html

coverage-ci: ## Run coverage for CI (XML output, 60% threshold)
	cargo tarpaulin --fail-under 60 --out xml

clean: ## Remove build artifacts
	cargo clean

npm-test: ## Run npm packaging script tests
	node --test 'npm/scripts/*.test.mjs'

npm-stage: ## Build host binary and stage host-only npm packages in target/npm
	node npm/scripts/test-local.mjs --stage-only

npm-pack: ## Stage and pack host-only npm tarballs (dry inspection)
	node npm/scripts/test-local.mjs --pack-only

npm-test-local: ## Full local e2e: build, stage, pack, install, run --help via shim
	node npm/scripts/test-local.mjs
