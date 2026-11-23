"""Offline packaging checks; the fixture EXE is just bytes and is never run."""
import contextlib
import hashlib
from io import StringIO
from pathlib import Path
import runpy
import tempfile
import unittest
import zipfile

script = runpy.run_path(str(Path(__file__).with_name("windows-qa-archive.py")))
build = script["build"]
ci_build_context = script["ci_build_context"]
COMMIT = "a" * 40
EXPECTED = {"commit": COMMIT, "kind": "integration", "ref": "refs/heads/dev/alpha-0.2-integration", "run": "123", "attempt": "1"}
BUILD = "".join(f"{key}={value}\n" for key, value in EXPECTED.items()) + "audience=internal-QA-only\n"
INSTALLER = f"Rosenfu-alpha02-integration-{COMMIT[:12]}-windows-x64-setup.exe"


class QaArchive(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.stage = Path(temp.name) / "stage"
        self.zip = Path(temp.name) / "result.zip"
        self.stage.mkdir()
        for name in (
            "windows-alpha02-test-play.txt", "data-and-notices/road-data/manifest.json",
            "data-and-notices/water-japan-coarse-20261114/manifest.json",
            "data-and-notices/LINE-Seed-JP-OFL.txt",
            "data-and-notices/software/DEPENDENCIES.json", "data-and-notices/software/README.txt",
        ):
            path = self.stage / name
            path.parent.mkdir(exist_ok=True, parents=True)
            path.write_text("test")
        (self.stage / INSTALLER).write_bytes(b"fixture")
        sha = hashlib.sha256(b"fixture").hexdigest()
        (self.stage / "SHA256SUMS.txt").write_text(f"{sha}  {INSTALLER}\n")
        (self.stage / "BUILD.txt").write_text(BUILD)

    def test_roundtrip(self):
        with contextlib.redirect_stdout(StringIO()) as report:
            build(self.stage, self.zip, EXPECTED)
        self.assertEqual(report.getvalue().split()[0], hashlib.sha256(self.zip.read_bytes()).hexdigest())
        with zipfile.ZipFile(self.zip) as archive:
            self.assertEqual(archive.read(INSTALLER), b"fixture")
            self.assertIn(INSTALLER, archive.read("PACKAGE-SHA256SUMS.txt").decode("ascii"))

    def test_checks_product_version_when_workflow_expects_it(self):
        env = {
            "GITHUB_EVENT_NAME": "push", "GITHUB_SHA": COMMIT,
            "GITHUB_REF": EXPECTED["ref"], "GITHUB_RUN_ID": EXPECTED["run"],
            "GITHUB_RUN_ATTEMPT": EXPECTED["attempt"], "ROSENFU_QA_PRODUCT_VERSION": "0.2.1",
        }
        expected = ci_build_context(env)
        self.assertEqual(expected["product_version"], "0.2.1")
        path = self.stage / "BUILD.txt"
        for metadata in (BUILD, BUILD + "product_version=0.2.0-alpha.1\n"):
            path.write_text(metadata)
            with self.assertRaisesRegex(ValueError, "expected product version"):
                build(self.stage, self.zip, expected)
            self.assertFalse(self.zip.exists())
        path.write_text(BUILD + "product_version=0.2.1\n")
        with contextlib.redirect_stdout(StringIO()):
            build(self.stage, self.zip, expected)
        with zipfile.ZipFile(self.zip) as archive:
            self.assertIn(b"product_version=0.2.1", archive.read("BUILD.txt"))

    def test_repeat_is_byte_stable(self):
        with contextlib.redirect_stdout(StringIO()):
            build(self.stage, self.zip, EXPECTED)
            second = self.zip.with_name("repeat.zip")
            build(self.stage, second, EXPECTED)
        self.assertEqual(self.zip.read_bytes(), second.read_bytes())

    def test_rejects_reserved_manifest_file_and_directory(self):
        path = self.stage / "package-sha256sums.TXT"
        path.write_text("shadow")
        with self.assertRaisesRegex(ValueError, "Reserved ZIP"):
            build(self.stage, self.zip, EXPECTED)
        path.unlink()
        path.mkdir()
        with self.assertRaisesRegex(ValueError, "Reserved ZIP"):
            build(self.stage, self.zip, EXPECTED)

    def test_rejects_case_collisions(self):
        (self.stage / "build.TXT").touch()
        if len(list(self.stage.glob("*[bB][uU][iI][lL][dD].*"))) == 1:
            self.skipTest("Filesystem itself prevents case collisions")
        with self.assertRaisesRegex(ValueError, "Case-colliding"):
            build(self.stage, self.zip, EXPECTED)

    def test_rejects_wrong_run_and_audience(self):
        path = self.stage / "BUILD.txt"
        for metadata, error in [(BUILD.replace("run=123", "run=124"), "GitHub workflow context"),
                                (BUILD.replace("attempt=1", "attempt=2"), "GitHub workflow context"),
                                (BUILD.replace("internal-QA-only", "external"), "QA audience")]:
            path.write_text(metadata)
            with self.assertRaisesRegex(ValueError, error):
                build(self.stage, self.zip, EXPECTED)

    def test_matching_preview_context_roundtrips(self):
        preview = INSTALLER.replace("integration", "preview")
        (self.stage / INSTALLER).rename(self.stage / preview)
        expected = dict(EXPECTED, kind="preview", ref="refs/pull/257/merge")
        (self.stage / "BUILD.txt").write_text(
            "".join(f"{key}={value}\n" for key, value in expected.items()) + "audience=internal-QA-only\n"
        )
        checksum = self.stage / "SHA256SUMS.txt"
        checksum.write_text(checksum.read_text().replace(INSTALLER, preview))
        with contextlib.redirect_stdout(StringIO()):
            build(self.stage, self.zip, expected)

    def test_rejects_wrong_preview_ref(self):
        preview = INSTALLER.replace("integration", "preview")
        (self.stage / INSTALLER).rename(self.stage / preview)
        (self.stage / "BUILD.txt").write_text(BUILD.replace("kind=integration", "kind=preview"))
        with self.assertRaisesRegex(ValueError, "PR merge ref"):
            build(self.stage, self.zip, EXPECTED)

    def test_rejects_wrong_sha(self):
        (self.stage / INSTALLER).write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "checksum mismatch"):
            build(self.stage, self.zip, EXPECTED)

    def test_rejects_wrong_integration_ref(self):
        (self.stage / "BUILD.txt").write_text(f"commit={COMMIT}\nkind=integration\nref=refs/heads/main\n")
        with self.assertRaisesRegex(ValueError, "alpha 0.2 branch"):
            build(self.stage, self.zip, EXPECTED)

    def test_rejects_missing_notice(self):
        (self.stage / "data-and-notices" / "software" / "README.txt").unlink()
        with self.assertRaisesRegex(ValueError, "Incomplete stage"):
            build(self.stage, self.zip, EXPECTED)

    def test_rejects_unexpected_executable(self):
        (self.stage / "unrelated.exe").write_bytes(b"test")
        with self.assertRaisesRegex(ValueError, "Unexpected executable"):
            build(self.stage, self.zip, EXPECTED)

    def test_rejects_reserved_windows_path(self):
        # Cannot create an NT device name on Windows; Linux verifies preflight.
        if __import__("os").name == "nt":
            self.skipTest("Reserved filenames are prohibited by the Windows filesystem")
        (self.stage / "NUL.txt").write_bytes(b"test")
        with self.assertRaisesRegex(ValueError, "Unsafe staged ZIP path"):
            build(self.stage, self.zip, EXPECTED)


if __name__ == "__main__":
    unittest.main()
