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
import regression_fixture
import run


class VerificationTests(unittest.TestCase):
    def test_pinned_http_helper_stays_exact_through_autocrlf_checkout(self):
        pins = prepare.PINS["http_fixture_adaptation"]
        expected = (prepare.ROOT / "inbox_http_fixture.rs").read_bytes()
        original = subprocess.check_output([
            "git", "show", prepare.PINS["collaboration_commit"] + ":crates/collaboration/" + pins["path"],
        ], cwd=prepare.ROOT.parents[2])
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory) / "repository"
            repository.mkdir()
            def git(*arguments):
                return subprocess.check_output(["git", *arguments], cwd=repository)
            git("init", "--quiet")
            git("config", "core.autocrlf", "false")
            attributes = prepare.ROOT.parents[2] / ".gitattributes"
            (repository / ".gitattributes").write_bytes(attributes.read_bytes())
            relative = Path("scripts/spikes/sqlcipher-qualified/inbox_http_fixture.rs")
            helper = repository / relative
            helper.parent.mkdir(parents=True)
            helper.write_bytes(expected)
            ordinary = repository / "ordinary.txt"
            ordinary.write_bytes(b"ordinary checkout\nline two\n")
            git("add", ".gitattributes", relative.as_posix(), "ordinary.txt")
            git("-c", "user.name=Qualification Fixture", "-c", "user.email=fixture@example.invalid",
                "commit", "--no-gpg-sign", "--quiet", "-m", "fixture")
            git("config", "core.autocrlf", "true")
            helper.unlink()
            ordinary.unlink()
            git("checkout", "HEAD", "--", relative.as_posix(), "ordinary.txt")
            self.assertIn(b"\r\n", ordinary.read_bytes())
            self.assertEqual(helper.read_bytes(), expected)
            prepare.verify(helper, pins["helper_sha256"])
            target = repository / "regression"
            path = target / pins["path"]
            path.parent.mkdir(parents=True)
            path.write_bytes(original)
            with patch.object(prepare, "ROOT", helper.parent):
                regression_fixture.apply(target)
            self.assertEqual(prepare.digest(path), pins["patched_sha256"])
            # The attribute preserves bytes; verification still rejects an
            # unreviewed byte instead of normalizing it into the pinned source.
            helper.write_bytes(expected + b" ")
            path.write_bytes(original)
            with patch.object(prepare, "ROOT", helper.parent):
                with self.assertRaisesRegex(RuntimeError, "SHA-256 mismatch"):
                    regression_fixture.apply(target)
            self.assertEqual(path.read_bytes(), original)

    def test_http_fixture_adaptation_requires_exact_source_helper_and_result(self):
        pins = prepare.PINS["http_fixture_adaptation"]
        original = subprocess.check_output([
            "git", "show", prepare.PINS["collaboration_commit"] + ":crates/collaboration/" + pins["path"],
        ], cwd=prepare.ROOT.parents[2])
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            path = target / pins["path"]
            path.parent.mkdir(parents=True)
            path.write_bytes(original)
            regression_fixture.apply(target)
            self.assertEqual(prepare.digest(path), pins["patched_sha256"])
            self.assertIn(b"qualification_http_request(&mut s, deadline)", path.read_bytes())
            path.write_bytes(original + b"\n")
            with self.assertRaisesRegex(RuntimeError, "SHA-256 mismatch"):
                regression_fixture.apply(target)
            self.assertEqual(path.read_bytes(), original + b"\n")
            path.write_bytes(original)
            with patch.object(regression_fixture, "NEW", regression_fixture.NEW + "\n"):
                with self.assertRaisesRegex(RuntimeError, "differs from reviewed result"):
                    regression_fixture.apply(target)
            self.assertEqual(path.read_bytes(), original)
            helper = target / "helper"
            helper.mkdir()
            (helper / "inbox_http_fixture.rs").write_bytes(b"unreviewed fixture code")
            with patch.object(prepare, "ROOT", helper):
                with self.assertRaisesRegex(RuntimeError, "SHA-256 mismatch"):
                    regression_fixture.apply(target)
            self.assertEqual(path.read_bytes(), original)

    def test_wrong_artifact_hash_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sqlite3.c"
            path.write_bytes(b"changed source")
            with self.assertRaisesRegex(RuntimeError, "SHA-256 mismatch"):
                prepare.verify(path, hashlib.sha256(b"pinned source").hexdigest())

    def test_archive_traversal_and_links_are_refused_before_extraction(self):
        for name, kind in [
            ("../outside", tarfile.REGTYPE), ("/absolute", tarfile.REGTYPE),
            ("C:/outside", tarfile.REGTYPE), ("C:outside", tarfile.REGTYPE),
            ("//server/share/outside", tarfile.REGTYPE),
            (r"\rooted", tarfile.REGTYPE), (r"root\..\outside", tarfile.REGTYPE),
            ("root/file:stream", tarfile.REGTYPE), ("root/.. /outside", tarfile.REGTYPE),
            ("root/directory./file", tarfile.REGTYPE), ("root/NUL.txt", tarfile.REGTYPE),
            ("root/com1", tarfile.REGTYPE), ("root/link", tarfile.SYMTYPE),
            ("root/hard", tarfile.LNKTYPE), ("root/fifo", tarfile.FIFOTYPE),
        ]:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                archive = Path(directory) / "source.tar.gz"
                with tarfile.open(archive, "w:gz") as output:
                    # Preflight must reject the complete archive before even a
                    # preceding valid member is written.
                    safe = tarfile.TarInfo("root/safe.txt")
                    safe.size = 4
                    output.addfile(safe, io.BytesIO(b"safe"))
                    info = tarfile.TarInfo(name)
                    info.type = kind
                    info.linkname = "../../outside"
                    output.addfile(info, io.BytesIO())
                target = Path(directory) / "unpacked"
                with self.assertRaisesRegex(RuntimeError, "Unsafe archive"):
                    prepare.extract(archive, target)
                self.assertFalse(target.exists())

    def test_portable_archive_extracts_exact_regular_files(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "source.tar.gz"
            contents = b"pinned archive bytes\n\x00\xff"
            with tarfile.open(archive, "w:gz") as output:
                folder = tarfile.TarInfo("root/subdirectory/")
                folder.type = tarfile.DIRTYPE
                output.addfile(folder)
                info = tarfile.TarInfo("root/subdirectory/file.c")
                info.size = len(contents)
                output.addfile(info, io.BytesIO(contents))
            target = Path(directory) / "unpacked"
            prepare.extract(archive, target)
            self.assertEqual((target / "root/subdirectory/file.c").read_bytes(), contents)

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
