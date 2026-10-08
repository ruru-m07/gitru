#!/usr/bin/env python3
"""Build the actual retained desktop harness against verified native SQLCipher inputs."""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
QUALIFIED = ROOT / "scripts/spikes/sqlcipher-qualified"
CONNECTIONS = ROOT / "scripts/spikes/sqlcipher-connections"


def main() -> None:
    subprocess.run([sys.executable, str(QUALIFIED / "prepare.py")], cwd=ROOT, check=True)
    subprocess.run([sys.executable, str(CONNECTIONS / "prepare.py")], cwd=ROOT, check=True)
    build = ROOT / "target/keyed-harness"
    build.mkdir(parents=True, exist_ok=True)
    config = build / "cargo-config.toml"
    config.write_text(
        "[patch.crates-io]\n"
        f'sqlx-sqlite = {{ path = "{CONNECTIONS / "target/verified/sqlx-sqlite"}" }}\n'
        f'libsqlite3-sys = {{ path = "{QUALIFIED / "target/verified/libsqlite3-sys"}" }}\n'
        f'openssl-src = {{ path = "{QUALIFIED / "target/verified/openssl-src"}" }}\n'
    )
    cargo = shutil.which("cargo")
    if cargo is None:
        raise RuntimeError("Cargo is unavailable")
    if os.name == "nt":
        wrapper = build / "cargo-keyed.cmd"
        wrapper.write_text(f'@"{cargo}" --config "{config}" %*\r\n')
    else:
        wrapper = build / "cargo-keyed"
        wrapper.write_text(f'#!/bin/sh\nexec "{cargo}" --config "{config}" "$@"\n')
        wrapper.chmod(0o700)
    environment = os.environ.copy()
    environment["CARGO"] = str(wrapper)
    lock = ROOT / "Cargo.lock"
    original_lock = lock.read_bytes()
    try:
        subprocess.run(
            [
                "bun",
                "x",
                "tauri",
                "build",
                "--no-bundle",
                "--features",
                "collaboration-harness,native-keyed-storage",
                "--config",
                "src-tauri/tauri.harness.conf.json",
            ],
            cwd=ROOT / "apps/desktop",
            env=environment,
            check=True,
        )
    finally:
        # Cargo cannot encode conditional patches in one lockfile. The keyed
        # build resolves only verified local sources, then restores the normal
        # workspace lock byte-for-byte for ordinary builds and review.
        lock.write_bytes(original_lock)


if __name__ == "__main__":
    main()
