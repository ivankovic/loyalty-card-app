# This file is part of Loyalty Card App.
#
# Copyright (C) 2026 Marko Ivankovic
#
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU Affero General Public License as published
# by the Free Software Foundation, version 3 of the License.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
# GNU Affero General Public License for more details.
#
# You should have received a copy of the GNU Affero General Public License
# along with this program. If not, see <https://www.gnu.org/licenses/>.

# The developer's commands. `make help` lists them.

CARGO ?= cargo
BUN ?= bun
# nextest when it is installed: it enforces the time budgets in .config/nextest.toml.
RUNNER := $(shell command -v cargo-nextest >/dev/null 2>&1 && echo "$(CARGO) nextest run" || echo "$(CARGO) test")

.PHONY: help web build test test-rust test-web e2e integration-test coverage lint fmt hooks \
	third-party-notices check-third-party-notices check-versions check-headers image ci

help:
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | sed 's/:.*## /\t/' | expand -t 28

web/node_modules: web/package.json web/bun.lock
	cd web && $(BUN) install --frozen-lockfile
	@touch $@

web: web/node_modules ## Build the web app into web/dist, which the binary embeds
	cd web && $(BUN) run build

build: web ## Build the server with the web app in it (release)
	$(CARGO) build --release --locked

test: test-rust test-web ## The unit and API tests, Rust and web

test-rust:
	$(RUNNER) --locked

test-web: web/node_modules
	cd web && $(BUN) test src

e2e: web/node_modules ## The end-to-end tests: the real server, the built app, a browser
	cd web && $(BUN) run e2e

integration-test: ## The image behind a real nginx (needs podman or docker)
	LOYALTY_CONTAINER_TESTS=1 $(CARGO) nextest run --locked --test container

coverage: ## Rust line coverage, failing under the floor in CONTRIBUTING.md
	$(CARGO) llvm-cov nextest --locked --fail-under-lines 90 --summary-only

lint: web/node_modules ## Formatting, clippy, biome and the type checker
	$(CARGO) fmt --check
	$(CARGO) clippy --locked --all-targets -- -D warnings
	cd web && $(BUN) run lint && $(BUN) run typecheck

fmt: web/node_modules ## Format everything
	$(CARGO) fmt
	cd web && $(BUN) run lint:fix

hooks: ## Use the repository's git hooks
	git config core.hooksPath .githooks

third-party-notices: web/node_modules ## Write THIRD-PARTY-NOTICES.md
	$(CARGO) about generate --locked -c packaging/notices/about.toml \
		packaging/notices/third-party-notices.hbs -o THIRD-PARTY-NOTICES.md
	$(BUN) run scripts/js-notices.ts >> THIRD-PARTY-NOTICES.md

check-third-party-notices: web/node_modules ## Fail if THIRD-PARTY-NOTICES.md is out of date
	@tmp=$$(mktemp) && \
	$(CARGO) about generate --locked -c packaging/notices/about.toml \
		packaging/notices/third-party-notices.hbs -o $$tmp && \
	$(BUN) run scripts/js-notices.ts >> $$tmp && \
	diff -u THIRD-PARTY-NOTICES.md $$tmp && rm $$tmp || \
	{ echo "THIRD-PARTY-NOTICES.md is out of date: run make third-party-notices"; exit 1; }

check-versions: ## Fail if the version differs between Cargo.toml, web/package.json and CHANGELOG.md
	$(BUN) run scripts/check-versions.ts

check-headers: ## Fail if a file lacks the licence header
	$(BUN) run scripts/check-headers.ts

image: ## Build the container image
	docker build -f packaging/docker/Dockerfile -t loyalty-card-app .

ci: lint test check-versions check-headers check-third-party-notices e2e ## What CI checks, here
