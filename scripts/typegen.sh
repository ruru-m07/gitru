#!/bin/bash

# will do work around `--validation none`
cargo tauri-typegen generate --project-path . --output-path ./apps/desktop/src/tauri
