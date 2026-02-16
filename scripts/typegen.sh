#!/bin/bash

cargo tauri-typegen generate --project-path . --output-path ./packages/commands/src --validation zod
