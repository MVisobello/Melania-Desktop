#!/usr/bin/env python3
"""Portable nested-preview packaging tests. No real GPU or user-session writes."""
from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/package-nested-preview.py"
spec = importlib.util.spec_from_file_location("melania_bundle_packager", SCRIPT)
packager = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packager)


class PortablePreviewBundle(unittest.TestCase):
    def make_binaries(self, home: Path) -> Path:
        bin_dir = home / "compiled"
        bin_dir.mkdir()
        for name in packager.BINARIES:
            binary = bin_dir / name
            binary.write_text("#!/bin/sh\necho fixture\n", encoding="utf-8")
            binary.chmod(0o755)
        return bin_dir

    def test_bundle_is_relocatable_and_records_integrity_evidence(self):
        with tempfile.TemporaryDirectory(prefix="melania-bundle-test-") as tmp:
            root = Path(tmp)
            bin_dir = self.make_binaries(root)
            output = root / "output"
            commit = "a" * 40
            manifest = packager.stage(bin_dir, output, commit)
            self.assertEqual(manifest["source_commit"], commit)
            self.assertFalse(manifest["native_session_installable"])
            self.assertTrue(manifest["requires_existing_graphical_session"])
            self.assertIn("bin/melania-compositor", manifest["files_sha256"])
            self.assertIn("run.sh", manifest["files_sha256"])
            self.assertIn("share/wallpaper.png", manifest["files_sha256"])
            bundle = output / packager.BUNDLE_NAME
            self.assertTrue(os.access(bundle / "run.sh", os.X_OK))
            self.assertTrue((bundle / "bin/melania-files").is_file())
            self.assertEqual(json.loads((bundle / "manifest.json").read_text()), manifest)
            self.assertNotIn("wayland-sessions", " ".join(manifest["files_sha256"]))
            self.assertNotIn("xsessions", " ".join(manifest["files_sha256"]))
            archive = output / (packager.BUNDLE_NAME + ".tar.gz")
            checksum = (output / "SHA256SUMS").read_text().strip().split()[0]
            self.assertEqual(packager.sha256(archive), checksum)
            with tarfile.open(archive, "r:gz") as tar:
                paths = {member.name: member for member in tar.getmembers()}
                prefix = packager.BUNDLE_NAME + "/"
                self.assertIn(prefix + "run.sh", paths)
                self.assertIn(prefix + "manifest.json", paths)
                self.assertTrue(paths[prefix + "run.sh"].mode & 0o111)
                self.assertTrue(paths[prefix + "bin/melania-panel"].mode & 0o111)
                self.assertTrue(all(
                    name.startswith(prefix) and not member.issym() and not member.islnk()
                    for name, member in paths.items()
                ))
            syntax = subprocess.run(
                ["bash", "-n", str(bundle / "run.sh")],
                capture_output=True, text=True
            )
            self.assertEqual(syntax.returncode, 0, syntax.stderr)

    def test_missing_binary_fails_without_creating_output(self):
        with tempfile.TemporaryDirectory(prefix="melania-bundle-fail-") as tmp:
            root = Path(tmp)
            bin_dir = self.make_binaries(root)
            (bin_dir / "melania-panel").unlink()
            output = root / "no-output"
            with self.assertRaises(ValueError):
                packager.stage(bin_dir, output, "unknown")
            self.assertFalse(output.exists())

    def test_rejects_symlink_binary_and_does_not_follow_it(self):
        with tempfile.TemporaryDirectory(prefix="melania-bundle-link-") as tmp:
            root = Path(tmp)
            bin_dir = self.make_binaries(root)
            (bin_dir / "melania-settings").unlink()
            (bin_dir / "melania-settings").symlink_to(bin_dir / "melania-files")
            output = root / "not-written"
            with self.assertRaises(ValueError):
                packager.stage(bin_dir, output, "unknown")
            self.assertFalse(output.exists())

    def test_never_overwrites_an_existing_output(self):
        with tempfile.TemporaryDirectory(prefix="melania-bundle-existing-") as tmp:
            root = Path(tmp)
            bin_dir = self.make_binaries(root)
            output = root / "release"
            output.mkdir()
            sentinel = output / "important"
            sentinel.write_text("KEEP")
            with self.assertRaises(FileExistsError):
                packager.stage(bin_dir, output, "unknown")
            self.assertEqual(sentinel.read_text(), "KEEP")

    def test_rejects_unverified_commit_format(self):
        with tempfile.TemporaryDirectory(prefix="melania-bundle-commit-") as tmp:
            root = Path(tmp)
            bin_dir = self.make_binaries(root)
            with self.assertRaises(ValueError):
                packager.stage(bin_dir, root / "output", "abc")
            self.assertFalse((root / "output").exists())

    def test_portable_wallpaper_respects_user_preference(self):
        text = (ROOT / "melania-desktop-surface/src/main.rs").read_text()
        self.assertIn('std::env::var_os("MELANIA_WALLPAPER")', text)
        self.assertIn('prefs.desktop_wallpaper', text)
        self.assertIn('std::env::var_os("MELANIA_BUNDLE_DATA_DIR")', text)
        self.assertLess(text.index('prefs.desktop_wallpaper'),
                        text.index('std::env::var_os("MELANIA_BUNDLE_DATA_DIR")'))


if __name__ == "__main__":
    unittest.main(verbosity=2)
