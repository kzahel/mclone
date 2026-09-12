#!/usr/bin/env python3
"""Regression coverage for accidentally publishing unused local assets."""
import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("public_assets", Path(__file__).with_name("check-public-assets.py"))
boundary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(boundary)


class PublicBoundaryTests(unittest.TestCase):
    def test_unused_reference_archive_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in boundary.PACKS:
                shutil.copy2(boundary.STAGE / name, root / name)
            boundary.check(root)
            reference = root / "reference/minecraft-1.17.1/extracted.zip"
            reference.parent.mkdir(parents=True)
            reference.write_bytes(b"unused but still publicly downloadable")
            with self.assertRaisesRegex(ValueError, "local reference"):
                boundary.check(root)

    def test_renamed_or_modified_pack_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in boundary.PACKS:
                shutil.copy2(boundary.STAGE / name, root / name)
            with (root / boundary.PACKS[0]).open("ab") as output:
                output.write(b"unapproved content")
            with self.assertRaisesRegex(ValueError, "altered asset pack"):
                boundary.check(root)


if __name__ == "__main__":
    unittest.main()
