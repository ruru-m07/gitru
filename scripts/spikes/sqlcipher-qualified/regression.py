#!/usr/bin/env python3
"""Run collaboration sources against the verified engine in an isolated copy.

Store is still unkeyed here: this tests engine compatibility, not R140 lifecycle.
"""
import argparse
import json
from pathlib import Path
import shutil
import subprocess

import prepare
from run import environment_check


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--freeze-lock", action="store_true",
                        help="Maintainer-only: generate the separately reviewed regression lock")
    args = parser.parse_args()
    environment_check()
    prepare.verify_prepared()
    prepare.prepare_collaboration()
    source_root = prepare.TARGET / "collaboration-source"
    source = source_root / "crates" / "collaboration"
    destination = prepare.TARGET / "collaboration-regression"
    if destination.exists():
        for path in destination.iterdir():
            if path.name == "target":
                continue
            if path.is_dir() and not path.is_symlink():
                shutil.rmtree(path)
            else:
                path.unlink()
    shutil.copytree(source, destination, ignore=shutil.ignore_patterns("target"), dirs_exist_ok=True)
    manifest = destination / "Cargo.toml"
    original = manifest.read_text(encoding="utf-8")
    before = 'libsqlite3-sys = { version = "=0.37.0", features = ["bundled"] }'
    if original.count(before) != 1:
        raise RuntimeError("Collaboration manifest changed; explicitly review cipher feature injection")
    changed = original.replace(before, before.replace('["bundled"]', '["bundled-sqlcipher"]'))
    native = (prepare.TARGET / "verified" / "libsqlite3-sys").as_posix()
    changed += '\n[workspace]\n\n[patch.crates-io]\nlibsqlite3-sys = { path = ' + json.dumps(native) + ' }\n'
    crypto = (prepare.TARGET / "verified" / "openssl-src").as_posix()
    changed += 'openssl-src = { path = ' + json.dumps(crypto) + ' }\n'
    changed += '\n[target.\'cfg(not(target_vendor = "apple"))\'.dependencies]\n'
    changed += 'libsqlite3-sys = { version = "=0.37.0", features = ["bundled-sqlcipher-vendored-openssl"] }\n'
    manifest.write_bytes(changed.encode("utf-8"))
    for path in source.rglob("*"):
        if path.is_file() and path.name != "Cargo.toml":
            if prepare.digest(path) != prepare.digest(destination / path.relative_to(source)):
                raise RuntimeError("Regression source copy differs")
    # These three test fixtures explicitly identify the ordinary bundled engine.
    # Adapt only their exact version literal for this candidate; preserve every
    # data/migration/rollback assertion and the entire production WAL gate.
    for name, checksum in prepare.PINS["test_version_adaptations"].items():
        path = destination / name
        prepare.verify(path, checksum)
        contents = path.read_text(encoding="utf-8")
        if contents.count('"3.51.3"') != 1:
            raise RuntimeError(f"Expected one reviewed engine-identity literal in {name}")
        path.write_bytes(contents.replace('"3.51.3"', '"3.53.4"').encode("utf-8"))
    lock = prepare.ROOT / "regression.lock"
    if args.freeze_lock:
        shutil.copyfile(source_root / "Cargo.lock", destination / "Cargo.lock")
        # Keep existing main-workspace resolutions where possible. The resulting
        # separate lock is reviewed/committed; ordinary qualification is locked.
        subprocess.run(["cargo", "update", "-p", "openssl-src", "--precise",
                        prepare.PINS["openssl"]["patched_version"]], cwd=destination, check=True)
        subprocess.run(["cargo", "metadata", "--format-version=1"], cwd=destination,
                       check=True, stdout=subprocess.DEVNULL)
        shutil.copyfile(destination / "Cargo.lock", lock)
        print("Regression lock frozen; review before committing")
        return
    shutil.copyfile(lock, destination / "Cargo.lock")
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version=1"], cwd=destination))
    for name in ("libsqlite3-sys", "openssl-src"):
        packages = [package for package in metadata["packages"] if package["name"] == name]
        expected = prepare.TARGET / "verified" / name / "Cargo.toml"
        if len(packages) != 1 or Path(packages[0]["manifest_path"]).resolve() != expected.resolve():
            raise RuntimeError(f"Regression must use the verified {name} patch")
    subprocess.run(["cargo", "test", "--locked", "--features", "test-harness"], cwd=destination, check=True)
    print("UNKEYED_STORE_ENGINE_REGRESSION_PASS; keyed_store_lifecycle=not_qualified")


if __name__ == "__main__":
    main()
