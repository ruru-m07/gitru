#!/bin/bash
set -euo pipefail
cargo tauri-typegen generate --project-path . --output-path ./packages/commands/src --validation zod --force
bun scripts/collaboration-bindings.ts
