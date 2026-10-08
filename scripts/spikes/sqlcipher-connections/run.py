#!/usr/bin/env python3
"""Verify exact native/driver inputs and execute the owned keyed factory gates."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import prepare

spec = importlib.util.spec_from_file_location('native_qualification_run', prepare.ROOT.parent / 'sqlcipher-qualified' / 'run.py')
# The inherited runner imports its own prepare module. Import environment policy
# directly from source under an explicit module mapping, then restore our module.
import sys
saved = sys.modules.get('prepare')
sys.modules['prepare'] = prepare.native
native_run = importlib.util.module_from_spec(spec)
try:
    spec.loader.exec_module(native_run)
finally:
    sys.modules['prepare'] = saved


def call(*args):
    subprocess.run(args, cwd=prepare.ROOT, check=True)


def main():
    native_run.environment_check()
    prepare.verify()
    metadata = json.loads(subprocess.check_output(['cargo', 'metadata', '--locked', '--format-version=1'], cwd=prepare.ROOT))
    if Path(metadata['target_directory']).resolve() != prepare.TARGET.resolve():
        raise RuntimeError('Keyed factory qualification requires its isolated target directory')
    for name, manifest in {
        'sqlx-sqlite': prepare.TARGET / 'verified/sqlx-sqlite/Cargo.toml',
        'libsqlite3-sys': prepare.native.TARGET / 'verified/libsqlite3-sys/Cargo.toml',
        'openssl-src': prepare.native.TARGET / 'verified/openssl-src/Cargo.toml',
    }.items():
        packages = [p for p in metadata['packages'] if p['name'] == name]
        if len(packages) != 1 or Path(packages[0]['manifest_path']).resolve() != manifest.resolve():
            raise RuntimeError('Expected exactly one authenticated package: ' + name)
    links = [p for p in metadata['packages'] if p.get('links') == 'sqlite3']
    if len(links) != 1 or links[0]['name'] != 'libsqlite3-sys':
        raise RuntimeError('Expected exactly one authenticated SQLite linkage owner')
    call('cargo', 'fmt', '--', '--check')
    call('cargo', 'clippy', '--locked', '--all-targets', '--', '-D', 'warnings')
    build = subprocess.check_output(['cargo', 'test', '--locked', '--no-run', '--message-format=json'], cwd=prepare.ROOT, text=True)
    executables = [Path(v['executable']) for line in build.splitlines() if (v := json.loads(line)).get('executable') and v.get('target', {}).get('name') == 'gitru_keyed_connections']
    if len(executables) != 1:
        raise RuntimeError('Expected one native factory test executable')
    binary = executables[0]
    result = subprocess.run([str(binary), '--nocapture'], cwd=prepare.ROOT, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    print(result.stdout, flush=True)
    (prepare.TARGET / 'factory-evidence.txt').write_text(result.stdout, encoding='utf-8')
    result.check_returncode()
    if platform.system() == 'Darwin':
        linkage = subprocess.check_output(['otool', '-L', binary], text=True)
    elif platform.system() == 'Linux':
        linkage = subprocess.check_output(['ldd', binary], text=True)
    elif platform.system() == 'Windows':
        linkage = subprocess.check_output(['dumpbin', '/DEPENDENTS', binary], text=True)
    else:
        raise RuntimeError('Unsupported native qualification host')
    if any(any(word in line.lower() for word in ['sqlite', 'sqlcipher', 'libcrypto', 'libssl']) for line in linkage.splitlines() if str(binary) not in line):
        raise RuntimeError('Unexpected dynamic SQLite or crypto source')
    (prepare.TARGET / 'linkage.txt').write_text(linkage, encoding='utf-8')
    report = {
        'host': platform.system(), 'architecture': platform.machine(),
        'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
        'native_pins': prepare.native.PINS, 'driver_adaptation': prepare.PINS,
        'factory_lock_sha256': hashlib.sha256((prepare.ROOT / 'Cargo.lock').read_bytes()).hexdigest(),
        'test_executable_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
        'application_encryption_qualified': False,
        'personal_vault_or_data_used': False,
    }
    (prepare.TARGET / 'factory-report.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print('Actual keyed factory gates passed; application activation remains unqualified', flush=True)


if __name__ == '__main__':
    main()
