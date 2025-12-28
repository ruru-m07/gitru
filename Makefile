.PHONY: help setup clean dev build build-vite build-tauri build-full install lint format check-types test

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

##@ Setup & Installation

setup: ## Install dependencies and build Rust crates
	@echo "$(GREEN)Installing dependencies...$(NC)"
	bun install
	@echo "$(GREEN)Building Rust crates...$(NC)"
	cargo build
	@echo "$(GREEN)Setup complete!$(NC)"

install: ## Install dependencies only (alias: i)
	@echo "$(GREEN)Installing dependencies...$(NC)"
	bun install

i: install ## Short alias for install

##@ Development

dev: ## Start development server with Tauri
	@echo "$(GREEN)Starting Tauri development server...$(NC)"
	bun --cwd="./apps/desktop" run tauri dev

dev-vite: ## Start Vite development server only (no Tauri)
	@echo "$(GREEN)Starting Vite development server...$(NC)"
	bun --cwd="./apps/desktop" run dev

##@ Build

build-vite:
	@echo "$(GREEN)Building Vite application...$(NC)"
	@. ./scripts/load-env.sh && \
	  bun --cwd="./apps/desktop" run build
	@echo "$(GREEN)Vite build complete!$(NC)"

build-tauri: ## Build Tauri application
	@echo "$(GREEN)Building Tauri application...$(NC)"
	@. ./scripts/load-env.sh && \
	  bun --cwd="./apps/desktop" run tauri build
	@echo "$(GREEN)Tauri build complete!$(NC)"


build-full:
	@echo "$(GREEN)Starting full build...$(NC)"
	@. ./scripts/load-env.sh && \
	  $(MAKE) clean setup build-tauri
	@echo "$(GREEN)Full build complete!$(NC)"

build: build-tauri

##@ Code uality

lint: ## Run linter
	@echo "$(YELLOW)Running linter...$(NC)"
	bunx turbo run lint

format: ## Format code
	@echo "$(YELLOW)Formatting code...$(NC)"
	bun run format

check-types: ## Type check TypeScript code
	@echo "$(YELLOW)Checking types...$(NC)"
	bunx turbo run check-types

check: lint check-types ## Run all checks (lint + type check)

##@ Maintenance

clean: ## Clean build artifacts and dependencies
	@echo "$(RED)Cleaning build artifacts...$(NC)"
	@./scripts/clear.sh
	@echo "$(GREEN)Clean complete!$(NC)"

clean-light: ## Clean build artifacts only (keep node_modules)
	@echo "$(RED)Cleaning build artifacts...$(NC)"
	@rm -rf .turbo apps/desktop/dist target
	@echo "$(GREEN)Light clean complete!$(NC)"

rebuild: clean build ## Clean and rebuild

##@ Utilities

typegen: ## Generate TypeScript types
	@echo "$(YELLOW)Generating types...$(NC)"
	@./scripts/typegen.sh

cargo-build: ## Build Rust crates only
	@echo "$(GREEN)Building Rust crates...$(NC)"
	cargo build

cargo-clean: ## Clean Rust build artifacts
	@echo "$(RED)Cleaning Rust build artifacts...$(NC)"
	cargo clean

##@ Quick Commands

all: setup build ## Setup and build everything

quick-build: ## Quick build without cleaning
	@echo "$(GREEN)Quick building...$(NC)"
	bun --cwd="./apps/desktop" run tauri build

macos-dmg: ## Build macOS DMG (macOS only)
	@if [ "$$(uname)" = "Darwin" ]; then \
		echo "$(GREEN)Building macOS DMG...$(NC)"; \
		$(MAKE) build-tauri; \
	else \
		echo "$(RED)Error: This target is only available on macOS$(NC)"; \
		exit 1; \
	fi
