import hashlib
import io
import os
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import prepare
import run


class VerificationTests(unittest.TestCase):
    def test_wrong_artifact_hash_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sqlite3.c"
            path.write_bytes(b"changed source")
            with self.assertRaisesRegex(RuntimeError, "SHA-256 mismatch"):
                prepare.verify(path, hashlib.sha256(b"pinned source").hexdigest())

    def test_archive_traversal_and_links_are_refused_before_extraction(self):
        for name, kind in [("../outside", tarfile.REGTYPE), ("/absolute", tarfile.REGTYPE),
                           ("root/link", tarfile.SYMTYPE), ("root/hard", tarfile.LNKTYPE)]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                archive = Path(directory) / "source.tar.gz"
                with tarfile.open(archive, "w:gz") as output:
                    info = tarfile.TarInfo(name)
                    info.type = kind
                    info.linkname = "../../outside"
                    output.addfile(info, io.BytesIO())
                target = Path(directory) / "unpacked"
                with self.assertRaisesRegex(RuntimeError, "Unsafe archive"):
                    prepare.extract(archive, target)
                self.assertFalse(target.exists())

    def test_native_overrides_do_not_select_an_unverified_engine(self):
        for name in ("LIBSQLITE3_SYS_USE_PKG_CONFIG", "LIBSQLITE3_FLAGS", "SQLCIPHER_LIB_DIR", "OPENSSL_DIR"):
            with self.subTest(name=name), patch.dict(os.environ, {name: "fixture"}, clear=True):
                with self.assertRaisesRegex(RuntimeError, "Remove native build overrides"):
                    run.environment_check()

    def test_current_collaboration_blob_drift_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            path = target / "collaboration-source" / "crates" / "collaboration" / "src" / "lib.rs"
            path.parent.mkdir(parents=True)
            contents = b"pinned production source\n"
            path.write_bytes(contents)
            blob = hashlib.sha1(b"blob " + str(len(contents)).encode() + b"\0" + contents).hexdigest()
            listing = f"100644 blob {blob}\tcrates/collaboration/src/lib.rs\0".encode()
            with patch.object(prepare, "TARGET", target), patch.object(prepare.subprocess, "check_output", return_value=listing):
                prepare.verify_collaboration()
                path.write_bytes(b"changed production gate\n")
                with self.assertRaisesRegex(RuntimeError, "differs from pinned Git object"):
                    prepare.verify_collaboration()

    def test_prepared_rust_build_script_cannot_be_modified(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            archive = target / "libsqlite3-sys.crate"
            original = b"verified crate build script"
            with tarfile.open(archive, "w:gz") as output:
                info = tarfile.TarInfo("libsqlite3-sys/build.rs")
                info.size = len(original)
                output.addfile(info, io.BytesIO(original))
            package = target / "verified" / "libsqlite3-sys"
            package.mkdir(parents=True)
            path = package / "build.rs"
            path.write_bytes(original)
            pins = {"crate_sha256": prepare.digest(archive), "amalgamation": {}}
            with patch.object(prepare, "TARGET", target), patch.object(prepare, "PINS", pins), patch.object(prepare, "verify_openssl"):
                prepare.verify_prepared()
                path.write_bytes(b"changed link target")
                with self.assertRaisesRegex(RuntimeError, "SHA-256 mismatch"):
                    prepare.verify_prepared()
                path.write_bytes(original)
                (package / "injected.rs").write_text("extra source")
                with self.assertRaisesRegex(RuntimeError, "Unexpected files"):
                    prepare.verify_prepared()


if __name__ == "__main__":
    unittest.main()
