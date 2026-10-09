#!/usr/bin/env python3
"""Run the source-verified probe; reject native build overrides and duplicate linkage."""
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess

import prepare


def environment_check():
    # These can silently select a system engine/provider or change bundled flags.
    prefixes = ("SQLITE3_", "SQLCIPHER_", "LIBSQLITE3_", "OPENSSL_", "DEP_OPENSSL_")
    compiler_overrides = ("CFLAGS", "CPPFLAGS", "CXXFLAGS", "LDFLAGS", "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")
    forbidden = [name for name in os.environ if name.startswith(prefixes + compiler_overrides)]
    if forbidden:
        raise RuntimeError("Remove native build overrides: " + ", ".join(sorted(forbidden)))


def command(*arguments):
    subprocess.run(arguments, cwd=prepare.ROOT, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--portability-dir", type=Path)
    args = parser.parse_args()
    environment_check()
    prepare.verify_prepared()
    prepare.verify_collaboration()
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version=1"], cwd=prepare.ROOT))
    links = [package for package in metadata["packages"] if package.get("links") == "sqlite3"]
    expected = prepare.TARGET / "verified" / "libsqlite3-sys" / "Cargo.toml"
    if Path(metadata["target_directory"]).resolve() != prepare.TARGET.resolve():
        raise RuntimeError("Qualification requires its isolated target directory; remove CARGO_TARGET_DIR")
    if len(links) != 1 or Path(links[0]["manifest_path"]).resolve() != expected.resolve():
        raise RuntimeError("Expected exactly one verified native SQLite link owner")
    crypto = [package for package in metadata["packages"] if package["name"] == "openssl-src"]
    if len(crypto) != 1 or Path(crypto[0]["manifest_path"]).resolve() != (prepare.TARGET / "verified" / "openssl-src" / "Cargo.toml").resolve():
        raise RuntimeError("Expected the verified current OpenSSL source patch")
    print(f"host={platform.system()} {platform.machine()}; sqlite_link_owners=1", flush=True)
    command("cargo", "fmt", "--", "--check")
    command("cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings")
    command("cargo", "build", "--locked", "--release")
    binary = prepare.TARGET / "release" / ("gitru-sqlcipher-qualified.exe" if os.name == "nt" else "gitru-sqlcipher-qualified")
    arguments = [str(binary)]
    if args.portability_dir:
        arguments += ["--portability-dir", str(args.portability_dir.resolve())]
    executed = subprocess.run(arguments, cwd=prepare.ROOT, text=True,
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    evidence = executed.stdout
    print(evidence, flush=True)
    (prepare.TARGET / "probe-evidence.txt").write_text(evidence)
    executed.check_returncode()
    # Check the actually packaged executable, independently of Cargo's graph.
    if platform.system() == "Darwin":
        linkage = subprocess.check_output(["otool", "-L", binary], text=True)
    elif platform.system() == "Linux":
        linkage = subprocess.check_output(["ldd", binary], text=True)
    elif platform.system() == "Windows":
        linkage = subprocess.check_output(["dumpbin", "/DEPENDENTS", binary], text=True)
    else:
        raise RuntimeError("Unqualified operating system")
    print(linkage, flush=True)
    if any("sqlite" in line.lower() or "sqlcipher" in line.lower() or "libcrypto" in line.lower() or "libssl" in line.lower()
           for line in linkage.splitlines() if str(binary) not in line):
        raise RuntimeError("Unexpected dynamic SQLite/cipher linkage")
    (prepare.TARGET / "linkage.txt").write_text(linkage)
    toolchain = subprocess.check_output(["rustc", "--version", "--verbose"], text=True)
    print(toolchain, flush=True)
    compiler = subprocess.run(["cl"] if os.name == "nt" else ["cc", "--version"],
                              text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT).stdout
    report = {"pins": prepare.PINS, "os": platform.system(), "architecture": platform.machine(),
              "platform_release": platform.release(), "macos_version": platform.mac_ver()[0],
              "c_compiler": compiler,
              "rustc": toolchain, "binary_sha256": prepare.digest(binary),
              "probe_lock_sha256": prepare.digest(prepare.ROOT / "Cargo.lock"),
              "native_sqlite_link_owners": 1,
              "application_encryption_qualified": False}
    (prepare.TARGET / "build-report.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
