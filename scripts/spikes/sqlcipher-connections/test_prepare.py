"""Finite exact-source controls; no native build or personal database."""
import hashlib
import io
import tarfile
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import prepare


class AdaptationTests(unittest.TestCase):
    def test_each_input_change_is_refused(self):
        for name in prepare.PINS['files']:
            with self.subTest(name=name), self.assertRaisesRegex(RuntimeError, 'source hash'):
                prepare.adapt(name, b'changed upstream bytes')

    def test_output_pin_and_unique_literal_boundary_fail_closed(self):
        name = next(iter(prepare.PINS['files']))
        raw = b'one exact boundary\n'
        rule = {'input_sha256': hashlib.sha256(raw).hexdigest(), 'output_sha256': '0' * 64,
                'replacements': [['boundary', 'replacement']]}
        with patch.dict(prepare.PINS, {'files': {name: rule}}):
            with self.assertRaisesRegex(RuntimeError, 'adapted hash'):
                prepare.adapt(name, raw)
            rule['replacements'] = [['missing boundary', 'replacement']]
            with self.assertRaisesRegex(RuntimeError, 'boundary mismatch'):
                prepare.adapt(name, raw)

    def test_prepared_tree_rejects_extra_and_modified_source(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            package = target / 'verified' / 'sqlx-sqlite'
            package.mkdir(parents=True)
            file = package / 'source.rs'
            file.write_bytes(b'original')
            with patch.object(prepare, 'TARGET', target), \
                 patch.object(prepare.native, 'verify_prepared'), \
                 patch.object(prepare.native, 'verify'), \
                 patch.object(prepare, 'expected_files', return_value={'source.rs': b'original'}):
                prepare.verify()
                file.write_bytes(b'modified')
                with self.assertRaisesRegex(RuntimeError, 'adapted bytes'):
                    prepare.verify()
                file.write_bytes(b'original')
                (package / 'extra.rs').write_bytes(b'extra')
                with self.assertRaisesRegex(RuntimeError, 'tree differs'):
                    prepare.verify()

    def test_native_archive_guard_precedes_driver_writes(self):
        for name in ['sqlx-sqlite-0.9.0/../escape', '/absolute', 'C:/drive', 'sqlx-sqlite-0.9.0/CON', 'sqlx-sqlite-0.9.0/file:stream']:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                archive = Path(directory) / 'bad.crate'
                with tarfile.open(archive, 'w:gz') as tar:
                    entry = tarfile.TarInfo(name)
                    entry.size = 1
                    tar.addfile(entry, io.BytesIO(b'x'))
                with self.assertRaisesRegex(RuntimeError, 'Unsafe archive member'):
                    prepare.expected_files(archive)

    def test_review_patch_uses_explicit_lf_attribute(self):
        root = prepare.ROOT.parents[2]
        import subprocess
        value = subprocess.check_output(['git', 'check-attr', 'eol', '--', 'scripts/spikes/sqlcipher-connections/driver-adaptation.patch'], cwd=root, text=True)
        self.assertTrue(value.strip().endswith(': lf'))


if __name__ == '__main__':
    unittest.main()
