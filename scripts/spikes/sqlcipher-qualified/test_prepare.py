import hashlib
import io
import os
from pathlib import Path
import tarfile
import subprocess
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

    def test_pinned_git_archive_ignores_checkout_crlf_conversion(self):
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory) / "repository"
            repository.mkdir()
            def git(*arguments):
                return subprocess.check_output(["git", *arguments], cwd=repository)
            git("init", "--quiet")
            git("config", "core.autocrlf", "false")
            source = repository / "crates" / "collaboration" / "src"
            source.mkdir(parents=True)
            expected = b"pinned source\nline two\n"
            (source / "lib.rs").write_bytes(expected)
            (repository / "Cargo.lock").write_bytes(expected)
            git("add", "Cargo.lock", "crates")
            git("-c", "user.name=Qualification Fixture", "-c", "user.email=fixture@example.invalid",
                "commit", "--no-gpg-sign", "--quiet", "-m", "fixture")
            commit = git("rev-parse", "HEAD").decode().strip()
            tree = git("rev-parse", "HEAD:crates/collaboration").decode().strip()
            git("config", "core.autocrlf", "true")
            # Git archive itself, not Python I/O, applies this checkout setting.
            raw = git("archive", "--format=tar", commit, "Cargo.lock")
            with tarfile.open(fileobj=io.BytesIO(raw)) as archive:
                self.assertIn(b"\r\n", archive.extractfile("Cargo.lock").read())
            target = repository / "isolated"
            target.mkdir()
            root = repository / "scripts" / "spikes" / "sqlcipher-qualified"
            with patch.object(prepare, "ROOT", root), patch.object(prepare, "TARGET", target), patch.object(prepare, "PINS", {
                "collaboration_commit": commit, "collaboration_tree": tree,
            }):
                prepare.prepare_collaboration()
                self.assertEqual((target / "collaboration-source" / "Cargo.lock").read_bytes(), expected)
                self.assertEqual((target / "collaboration-source" / "crates" / "collaboration" / "src" / "lib.rs").read_bytes(), expected)

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
