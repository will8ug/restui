.PHONY: build test clippy fmt lint check run run-sample install clean coverage coverage-ci help

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

run: ## Run the binary (requires FILE=...)
	cargo run -- $(FILE)

run-sample: ## Run with the sample .http file
	cargo run -- $(SAMPLE)

install: ## Install binary to ~/.cargo/bin
	cargo install --path .

coverage: ## Run test coverage (requires cargo-tarpaulin)
	cargo tarpaulin --fail-under 60 --out Html

coverage-ci: ## Run coverage for CI (XML output, 60% threshold)
	cargo tarpaulin --fail-under 60 --out xml

clean: ## Remove build artifacts
	cargo clean
