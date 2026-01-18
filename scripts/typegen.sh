#!/bin/bash

# will do work around `--validation none`
cargo tauri-typegen generate --project-path . --output-path ./packages/commands/src --validation zod
# node ./scripts/js/fix-used-before-declaration.js ./apps/desktop/src/tauri/types.ts ./apps/desktop/src/tauri/types.ts
# node ./scripts/js/replace-union-with-enum.js ./apps/desktop/src/tauri/types.ts ./apps/desktop/src/tauri/types.ts
