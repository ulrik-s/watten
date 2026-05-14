# Convenience targets for local development. Run `make help` for a summary.

.PHONY: help dev build test test-rust test-js test-e2e coverage coverage-rust coverage-js clean fmt

help:
	@echo "Watten — local commands"
	@echo ""
	@echo "  make dev            Build wasm and start the Vite dev server"
	@echo "                      (open http://localhost:5173)"
	@echo "  make build          Production build into frontend/dist"
	@echo "  make test           Run all Rust + JS unit/integration tests"
	@echo "  make test-rust      Rust tests only"
	@echo "  make test-js        Vitest only"
	@echo "  make test-e2e       Playwright E2E (Chromium + Firefox + WebKit)"
	@echo "  make coverage       Generate Rust + JS coverage reports"
	@echo "  make fmt            cargo fmt"
	@echo "  make clean          Remove build artefacts"

dev:
	cd frontend && npm install && npm run build:wasm && npx vite

build:
	cd frontend && npm install && npm run build

test: test-rust test-js

test-rust:
	cargo test

test-js:
	cd frontend && npm test

test-e2e:
	cd frontend && npm run test:e2e

coverage: coverage-rust coverage-js

coverage-rust:
	cargo llvm-cov --html --output-dir coverage-rust
	cargo llvm-cov --lcov --output-path coverage-rust/lcov.info
	@echo "Rust HTML report: coverage-rust/html/index.html"

coverage-js:
	cd frontend && npm run coverage
	@echo "JS HTML report: frontend/coverage/index.html"

fmt:
	cargo fmt

clean:
	cargo clean
	rm -rf frontend/dist frontend/pkg frontend/pkg-test pkg-test \
	       frontend/coverage frontend/test-results frontend/playwright-report \
	       coverage-rust
