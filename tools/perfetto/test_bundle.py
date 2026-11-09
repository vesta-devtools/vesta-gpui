"""Packaging contracts without downloading or building."""
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
import warnings
import zipfile

spec = importlib.util.spec_from_file_location("bundle", Path(__file__).with_name("bundle.py"))
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)


class BundleTests(unittest.TestCase):
    def archive(self, extra):
        data = io.BytesIO()
        with warnings.catch_warnings(), zipfile.ZipFile(data, "w") as archive:
            warnings.simplefilter("ignore", UserWarning)
            for name in ("index.html", f"{bundle.RELEASE['ui_directory']}/frontend_bundle.js",
                         f"{bundle.RELEASE['ui_directory']}/trace_processor.wasm", extra):
                entry = zipfile.ZipInfo("fixture")
                entry.filename = name
                archive.writestr(entry, b"fixture")
        data.seek(0)
        return data

    def test_archive_escape_and_duplicates_fail_before_writing(self):
        for name in ("../escape", "/absolute", "dir\\escape", "x:stream", "index.html"):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                with self.assertRaises(ValueError):
                    bundle.extract(self.archive(name), Path(directory))
                self.assertEqual(list(Path(directory).iterdir()), [])

    def test_manifest_records_exact_unmodified_asset_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            files = bundle.extract(self.archive("assets/font.woff2"), Path(directory))
            self.assertEqual(len(files), 4)
            for entry in files:
                self.assertEqual(entry["sha256"], bundle.sha(b"fixture"))
                self.assertEqual((Path(directory) / entry["path"]).read_bytes(), b"fixture")

    def test_corrupted_cached_release_is_never_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / "ui.zip"
            archive.write_bytes(b"corrupt")
            with self.assertRaisesRegex(ValueError, "Checksum mismatch"):
                bundle.obtain("https://unused.invalid", bundle.RELEASE["archive_sha256"], archive)


if __name__ == "__main__":
    unittest.main()
