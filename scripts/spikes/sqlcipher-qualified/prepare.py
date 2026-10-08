#!/usr/bin/env python3
"""Prepare a verified qualification-only native patch. Never accesses app data."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parent
PINS = json.loads((ROOT / "pins.json").read_text(encoding="utf-8"))
TARGET = ROOT / "target"


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            result.update(chunk)
    return result.hexdigest()


def verify(path, expected):
    actual = digest(path)
    if actual != expected:
        raise RuntimeError(f"SHA-256 mismatch for {path.name}: {actual}")


def download(name, url, expected):
    path = TARGET / name
    if not path.exists():
        with urllib.request.urlopen(url, timeout=60) as response:
            # Bound even an unexpected origin response before hash verification.
            with tempfile.NamedTemporaryFile(dir=TARGET, delete=False) as output:
                temporary = Path(output.name)
                try:
                    size = 0
                    while chunk := response.read(1024 * 1024):
                        size += len(chunk)
                        if size > 80 * 1024 * 1024:
                            raise RuntimeError("Source archive exceeds qualification bound")
                        output.write(chunk)
                    output.close()
                    verify(temporary, expected)
                    temporary.replace(path)
                finally:
                    temporary.unlink(missing_ok=True)
    verify(path, expected)
    return path


def extract(archive, destination):
    with tarfile.open(archive, "r:*") as source:
        # Pins authenticate content; also refuse traversal, links and special files.
        members = source.getmembers()
        for member in members:
            path = Path(member.name)
            if path.is_absolute() or ".." in path.parts or not (member.isdir() or member.isfile()):
                raise RuntimeError(f"Unsafe archive member {member.name}")
        for member in members:
            path = destination / member.name
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as incoming, path.open("wb") as output:
                    shutil.copyfileobj(incoming, output)
                path.chmod(member.mode & 0o777)


def verify_prepared():
    """Authenticate all FFI sources, not just the two replaced native files."""
    archive = TARGET / "libsqlite3-sys.crate"
    verify(archive, PINS["crate_sha256"])
    package = TARGET / "verified" / "libsqlite3-sys"
    expected_paths = set()
    with tarfile.open(archive, "r:gz") as source:
        for member in source.getmembers():
            if not member.isfile():
                continue
            relative = Path(*Path(member.name).parts[1:])
            expected_paths.add(relative)
            expected = PINS["amalgamation"].get(relative.name) if relative.parent == Path("sqlcipher") else None
            if expected is None:
                expected = hashlib.sha256(source.extractfile(member).read()).hexdigest()
            verify(package / relative, expected)
    actual_paths = {path.relative_to(package) for path in package.rglob("*") if path.is_file()}
    if actual_paths != expected_paths:
        raise RuntimeError("Unexpected files in verified native patch")
    verify_openssl()


def openssl_sources():
    pins = PINS["openssl"]
    crate = download("openssl-src.crate", pins["crate_url"], pins["crate_sha256"])
    source = download("openssl-3.6.5-source.tar.gz", pins["source_url"], pins["source_sha256"])
    return crate, source


def prepare_openssl():
    crate, source = openssl_sources()
    pins = PINS["openssl"]
    with tempfile.TemporaryDirectory(dir=TARGET, prefix="openssl-") as temporary:
        stage = Path(temporary)
        extract(crate, stage)
        extract(source, stage)
        wrapper = stage / f"openssl-src-{pins['crate_version']}"
        payload = stage / f"openssl-openssl-{pins['version']}"
        version = (payload / "VERSION.dat").read_text(encoding="utf-8")
        if "MAJOR=3\nMINOR=6\nPATCH=5\n" not in version:
            raise RuntimeError("Unexpected OpenSSL payload version")
        shutil.rmtree(wrapper / "openssl")
        shutil.move(str(payload), wrapper / "openssl")
        # Preserve build logic; label the new native payload truthfully in Cargo.
        for name in ("Cargo.toml", "Cargo.toml.orig"):
            path = wrapper / name
            contents = path.read_text(encoding="utf-8")
            old = f'version = "{pins["crate_version"]}"'
            if contents.count(old) != 1:
                raise RuntimeError("Unexpected OpenSSL wrapper manifest")
            path.write_bytes(contents.replace(old, f'version = "{pins["patched_version"]}"').encode("utf-8"))
        destination = TARGET / "verified" / "openssl-src"
        if destination.exists():
            shutil.rmtree(destination)
        shutil.move(str(wrapper), destination)
        shutil.copyfile(destination / "openssl" / "LICENSE.txt", TARGET / "verified" / "OPENSSL-LICENSE.txt")


def verify_openssl():
    pins = PINS["openssl"]
    package = TARGET / "verified" / "openssl-src"
    expected = {}
    for archive, digest_expected, native in [
        (TARGET / "openssl-src.crate", pins["crate_sha256"], False),
        (TARGET / "openssl-3.6.5-source.tar.gz", pins["source_sha256"], True),
    ]:
        verify(archive, digest_expected)
        with tarfile.open(archive, "r:gz") as source:
            for member in source.getmembers():
                if not member.isfile():
                    continue
                relative = Path(*Path(member.name).parts[1:])
                if not native and relative.parts[0] == "openssl":
                    continue
                content = source.extractfile(member).read()
                if not native and str(relative) in ("Cargo.toml", "Cargo.toml.orig"):
                    content = content.replace(pins["crate_version"].encode(), pins["patched_version"].encode())
                if native:
                    relative = Path("openssl") / relative
                expected[relative] = hashlib.sha256(content).hexdigest()
    for path, checksum in expected.items():
        verify(package / path, checksum)
    present = {path.relative_to(package) for path in package.rglob("*") if path.is_file()}
    if present != set(expected):
        raise RuntimeError("Unexpected files in verified OpenSSL patch")


def prepare_collaboration():
    repository = ROOT.parents[2]
    environment = dict(os.environ, GIT_NO_REPLACE_OBJECTS="1")
    commit = PINS["collaboration_commit"]
    tree = subprocess.check_output(["git", "rev-parse", f"{commit}:crates/collaboration"],
                                   cwd=repository, env=environment, text=True).strip()
    if tree != PINS["collaboration_tree"]:
        raise RuntimeError("Pinned collaboration source tree does not match")
    with tempfile.TemporaryDirectory(dir=TARGET, prefix="collaboration-source-") as temporary:
        stage = Path(temporary)
        archive = stage / "source.tar"
        subprocess.run(["git", "-c", "core.autocrlf=false", "-c", "core.eol=lf",
                        "archive", "--format=tar", f"--output={archive}", commit,
                        "crates/collaboration", "Cargo.lock"], cwd=repository, env=environment, check=True)
        extract(archive, stage / "unpacked")
        destination = TARGET / "collaboration-source"
        if destination.exists():
            shutil.rmtree(destination)
        shutil.move(str(stage / "unpacked"), destination)
    verify_collaboration()
    print(f"Pinned collaboration source={commit}; tree={tree}")


def verify_collaboration():
    listing = subprocess.check_output(
        ["git", "ls-tree", "-rz", "--full-tree", PINS["collaboration_commit"],
         "crates/collaboration", "Cargo.lock"], cwd=ROOT.parents[2],
        env=dict(os.environ, GIT_NO_REPLACE_OBJECTS="1"))
    source = TARGET / "collaboration-source"
    expected = set()
    for entry in listing.split(b"\0"):
        if not entry:
            continue
        facts, name = entry.split(b"\t", 1)
        mode, kind, object_id = facts.split(b" ")
        if mode not in (b"100644", b"100755") or kind != b"blob":
            raise RuntimeError("Only regular source files are allowed in the qualification copy")
        path = Path(name.decode("utf-8"))
        expected.add(path)
        content = (source / path).read_bytes()
        actual = hashlib.sha1(b"blob " + str(len(content)).encode() + b"\0" + content).hexdigest()
        if actual != object_id.decode("ascii"):
            raise RuntimeError(f"Collaboration source differs from pinned Git object: {path}")
    present = {path.relative_to(source) for path in source.rglob("*") if path.is_file()}
    if present != expected:
        raise RuntimeError("Pinned source copy contains missing or extra files")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--amalgamation-dir", type=Path,
                        help="Use previously generated artifacts, verified against the same pins")
    args = parser.parse_args()
    TARGET.mkdir(exist_ok=True)
    crate = download("libsqlite3-sys.crate", PINS["crate_url"], PINS["crate_sha256"])
    upstream = download("sqlcipher-source.tar.gz", PINS["source_url"], PINS["source_sha256"])
    with tempfile.TemporaryDirectory(dir=TARGET, prefix="prepare-") as temporary:
        stage = Path(temporary)
        extract(crate, stage)
        extract(upstream, stage)
        source = stage / f"sqlcipher-{PINS['commit']}"
        if (source / "VERSION").read_text(encoding="utf-8").strip() != PINS["sqlite_version"]:
            raise RuntimeError("Pinned SQLite version differs from source VERSION")
        amalgamation = args.amalgamation_dir
        if amalgamation is None:
            if os.name == "nt":
                raise RuntimeError("Windows consumes the hash-verified Unix generation artifact")
            subprocess.run(["./configure", "--disable-tcl", "--enable-fts5"], cwd=source, check=True)
            subprocess.run(["make", "-j2", "sqlite3.c"], cwd=source, check=True)
            amalgamation = source
        package = stage / f"libsqlite3-sys-{PINS['crate_version']}"
        for name, expected in PINS["amalgamation"].items():
            verify(amalgamation / name, expected)
            shutil.copyfile(amalgamation / name, package / "sqlcipher" / name)
        # No build.rs/Rust/feature changes: existing FFI owns the single sqlite3 link.
        verified = TARGET / "verified"
        verified.mkdir(exist_ok=True)
        package_target = verified / "libsqlite3-sys"
        if package_target.exists():
            shutil.rmtree(package_target)
        shutil.move(str(package), package_target)
        for name in ("LICENSE.md", "README.md"):
            shutil.copyfile(source / name, verified / f"SQLCIPHER-{name}")
        shutil.copyfile(ROOT / "pins.json", verified / "pins.json")
        print(f"Verified SQLCipher {PINS['sqlcipher_version']} / SQLite {PINS['sqlite_version']}")
        print(f"Qualification-only patch: {package_target}")
    prepare_openssl()
    verify_prepared()
    prepare_collaboration()


if __name__ == "__main__":
    main()
