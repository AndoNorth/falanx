SHELL := /bin/bash
.DEFAULT_GOAL := help

FALANX_VERSION := $(shell grep '^version = ' rust/crates/falanx-engine/Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

# get compose command
COMPOSE ?= $(shell \
	if command -v docker >/dev/null 2>&1 && docker compose version >/dev/null 2>&1; then \
		echo "docker compose"; \
	elif command -v docker-compose >/dev/null 2>&1; then \
		echo "docker-compose"; \
	fi)

# ── Dev ─────────────────────────────────────────────────────────

.PHONY: validate
validate: ## Run fmt + clippy + tests (requires nix shell)
	@command -v cargo >/dev/null 2>&1 || { echo "Run 'nix develop' first"; exit 1; }
	cd rust && cargo fmt --check && cargo clippy -- -D warnings && cargo nextest run --no-tests=pass

.PHONY: validate-full
validate-full: validate ## validate + cargo audit
	cd rust && cargo audit

# ── Image ────────────────────────────────────────────────────────

.PHONY: falanx-image
falanx-image: ## Build falanx-engine Docker image via Nix (headless, no shell)
	@arch=$$(uname -m); \
	case "$$arch" in \
	  x86_64|amd64) target=amd64 ;; \
	  aarch64|arm64) target=arm64 ;; \
	  *) echo "unsupported architecture: $$arch" >&2; exit 1 ;; \
	esac; \
	nix build ".#falanx-engine-docker-$$target"

.PHONY: falanx-image-load
falanx-image-load: falanx-image ## Build image and load into local Docker daemon
	@arch=$$(uname -m); \
	case "$$arch" in \
	  x86_64|amd64) target=amd64 ;; \
	  aarch64|arm64) target=arm64 ;; \
	  *) echo "unsupported architecture: $$arch" >&2; exit 1 ;; \
	esac; \
	./result | docker load; \
	docker tag falanx/engine:$(FALANX_VERSION)-$$target falanx/engine:latest; \
	echo "Loaded falanx/engine:latest ($(FALANX_VERSION)-$$target)"

REGISTRY ?= localhost:5000

.PHONY: falanx-image-push
falanx-image-push: falanx-image-load ## Build, load, and push to REGISTRY (default: localhost:5000)
	docker tag falanx/engine:latest $(REGISTRY)/falanx/engine:latest
	docker tag falanx/engine:latest $(REGISTRY)/falanx/engine:$(FALANX_VERSION)
	docker push $(REGISTRY)/falanx/engine:latest
	docker push $(REGISTRY)/falanx/engine:$(FALANX_VERSION)
	@echo "Pushed to $(REGISTRY)/falanx/engine"

# ── Help ─────────────────────────────────────────────────────────

.PHONY: help
help: ## Show this help
	@grep -E '^[a-zA-Z0-9_%-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-22s\033[0m %s\n", $$1, $$2}'
