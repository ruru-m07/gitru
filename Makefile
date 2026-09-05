.PHONY: help setup clean dev dev-vite build-vite build-tauri build format typegen test format-check clippy verify

.DEFAULT_GOAL := help

CYAN := \033[0;36m
GREEN := \033[0;32m
YELLOW := \033[0;33m
RED := \033[0;31m
NC := \033[0m

##@ General

help: ## Display this help message
	@echo "$(CYAN)Gitru Build System$(NC)"
	@echo ""
	@awk 'BEGIN {FS = ":.*##"; printf "Usage:\n  make $(CYAN)<target>$(NC)\n"} /^[a-zA-Z_0-9-]+:.*?##/ { printf "  $(CYAN)%-20s$(NC) %s\n", $$1, $$2 } /^##@/ { printf "\n$(YELLOW)%s$(NC)\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

setup: ## Install dependencies and build Rust crates
	@echo "$(GREEN)Installing dependencies...$(NC)"
	bun install
	@echo "$(GREEN)Building Rust crates...$(NC)"
	cargo build
	@echo "$(GREEN)Setup complete!$(NC)"

##@ Development

dev: ## Start development server with Tauri
	@echo "$(GREEN)Starting Tauri development server...$(NC)"
	bun --cwd="./apps/desktop" run tauri dev

dev-vite: ## Start Vite development server only (no Tauri)
	@echo "$(GREEN)Starting Vite development server...$(NC)"
	bun --cwd="./apps/desktop" run dev

##@ Build

build-vite: ## Build Vite application only (no Tauri)
	@echo "$(GREEN)Building Vite application...$(NC)"
	@. ./scripts/load-env.sh && \
	  bun --cwd="./apps/desktop" run build
	@echo "$(GREEN)Vite build complete!$(NC)"

build-tauri: ## Build Tauri application
	@echo "$(GREEN)Building Tauri application...$(NC)"
	@. ./scripts/load-env.sh && \
	  bun --cwd="./apps/desktop" run tauri build
	@echo "$(GREEN)Tauri build complete!$(NC)"

build: ## Build both Vite and Tauri applications
	@echo "$(GREEN)Building both Vite and Tauri applications...$(NC)"
	@make build-tauri

##@ Utilities

format: ## Format code
	@echo "$(YELLOW)Formatting code...$(NC)"
	bun run format

typegen: ## Generate TypeScript types
	@echo "$(YELLOW)Generating types...$(NC)"
	@./scripts/typegen.sh

clean: ## Clean build artifacts and dependencies
	@echo "$(RED)Cleaning build artifacts...$(NC)"
	@./scripts/clear.sh
	@echo "$(GREEN)Clean complete!$(NC)"

##@ Testing

test: ## Run all Rust tests
	@echo "$(GREEN)Running Rust tests...$(NC)"
	cargo test --workspace --verbose

format-check: ## Check code formatting
	@echo "$(YELLOW)Checking code formatting...$(NC)"
	cargo fmt --all -- --check

clippy: ## Run Clippy linter
	@echo "$(YELLOW)Running Clippy linter...$(NC)"
	cargo clippy --workspace --all-targets -- -D warnings

verify: ## Run the complete local and CI verification suite
	@echo "$(YELLOW)Running frontend tests, lint, type checks, and desktop build...$(NC)"
	bun test apps/desktop/tests
	bun run lint
	bun run check-types
	bun run build --filter gitru
	@echo "$(YELLOW)Running Rust formatting, lint, and tests...$(NC)"
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace
