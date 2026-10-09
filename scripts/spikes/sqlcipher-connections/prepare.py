#!/usr/bin/env python3
"""Exact source adaptation for the opt-in native connection factory."""
import difflib
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile

ROOT = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location('native_qualification_prepare', ROOT.parent / 'sqlcipher-qualified' / 'prepare.py')
native = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(native)
PINS = json.loads((ROOT / 'driver-adaptation.json').read_text(encoding='utf-8'))
TARGET = ROOT / 'target'


def adapt(name, raw):
    rule = PINS['files'].get(name)
    if rule is None:
        return raw
    if hashlib.sha256(raw).hexdigest() != rule['input_sha256']:
        raise RuntimeError('SQLx source hash mismatch: ' + name)
    text = raw.decode('utf-8')
    for before, after in rule['replacements']:
        if text.count(before) != 1:
            raise RuntimeError('SQLx adaptation boundary mismatch: ' + name)
        text = text.replace(before, after)
    result = text.encode('utf-8')
    if hashlib.sha256(result).hexdigest() != rule['output_sha256']:
        raise RuntimeError('SQLx adapted hash mismatch: ' + name)
    return result


def expected_files(archive):
    expected = {}
    diff = []
    # Reuse the already qualified host-independent path/type guard before any
    # archive member is written. No second weaker Windows extraction policy.
    with tempfile.TemporaryDirectory() as tmp:
        extracted = Path(tmp)
        native.extract(archive, extracted)
        package = extracted / 'sqlx-sqlite-0.9.0'
        if list(extracted.iterdir()) != [package] or not package.is_dir():
            raise RuntimeError('Unexpected authenticated SQLx archive root')
        for file in sorted(package.rglob('*')):
            if not file.is_file():
                continue
            name = file.relative_to(package).as_posix()
            raw = file.read_bytes()
            transformed = adapt(name, raw)
            expected[name] = transformed
            if raw != transformed:
                diff.extend(difflib.unified_diff(raw.decode().splitlines(True), transformed.decode().splitlines(True), fromfile='a/'+name, tofile='b/'+name))
    if ''.join(diff) != (ROOT / 'driver-adaptation.patch').read_text(encoding='utf-8'):
        raise RuntimeError('Review patch differs from exact adaptation')
    return expected


def verify():
    native.verify_prepared()
    archive = native.TARGET / 'sqlx-sqlite.crate'
    native.verify(archive, PINS['archive_sha256'])
    expected = expected_files(archive)
    package = TARGET / 'verified' / 'sqlx-sqlite'
    if any(p.is_symlink() for p in package.rglob('*')):
        raise RuntimeError('Symlink in adapted SQLx source tree')
    actual = {p.relative_to(package).as_posix() for p in package.rglob('*') if p.is_file()}
    if actual != set(expected):
        raise RuntimeError('SQLx adapted source tree differs')
    for name, contents in expected.items():
        if (package / name).read_bytes() != contents:
            raise RuntimeError('SQLx adapted bytes differ: ' + name)


def main():
    TARGET.mkdir(exist_ok=True)
    native.verify_prepared()
    archive = native.download('sqlx-sqlite.crate', 'https://static.crates.io/crates/sqlx-sqlite/sqlx-sqlite-0.9.0.crate', PINS['archive_sha256'])
    expected = expected_files(archive)
    with tempfile.TemporaryDirectory(dir=TARGET) as tmp:
        root = Path(tmp)
        for name, contents in expected.items():
            destination = root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(contents)
        destination = TARGET / 'verified' / 'sqlx-sqlite'
        destination.parent.mkdir(exist_ok=True)
        if destination.exists():
            shutil.rmtree(destination)
        shutil.move(root, destination)
    verify()
    print('Verified native inputs and exact four-file SQLx initialization adaptation')


if __name__ == '__main__':
    main()
