#!/bin/bash

bun --cwd="./apps/desktop" run tauri build

mkdir -p dist

# ? Run post-build for custom DMG installation on macOS
if [[ "$OSTYPE" == "darwin"* ]]; then
    echo "Running post-macOS-dmg-build script..."
    ./scripts/post-macOS-dmg-build.sh
fi
