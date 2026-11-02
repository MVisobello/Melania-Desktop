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
from xml.etree import ElementTree

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

    def test_original_panel_icons_are_in_extracted_archive_and_svg_valid(self):
        with tempfile.TemporaryDirectory(prefix="melania-preview-icons-") as tmp:
            root = Path(tmp)
            binaries = self.make_binaries(root)
            output = root / "output"
            manifest = packager.stage(binaries, output, "c" * 40)
            prefix = "share/icons/Melania-Preview/"
            self.assertEqual(manifest["bundled_icon_theme"], "Melania-Preview")
            files = manifest["files_sha256"]
            self.assertIn(prefix + "index.theme", files)
            for name in packager.PREVIEW_APP_ICONS:
                self.assertIn(prefix + "scalable/apps/" + name, files)
            for kind in ("wifi", "bluetooth", "volume",
                         "battery", "notifications"):
                for suffix in ("", "-light"):
                    filename = prefix + "scalable/status/melania-tray-" + kind + suffix + ".svg"
                    self.assertIn(filename, files)
                    file = output / packager.BUNDLE_NAME / filename
                    root_node = ElementTree.parse(file).getroot()
                    self.assertEqual(root_node.tag, "{http://www.w3.org/2000/svg}svg")
                    self.assertGreater(len(root_node), 0)
                    self.assertEqual(packager.sha256(file), files[filename])
            with tarfile.open(output / (packager.BUNDLE_NAME + ".tar.gz"),
                              "r:gz") as archive:
                archived = set(archive.getnames())
                for name in files:
                    self.assertIn(packager.BUNDLE_NAME + "/" + name, archived)

    def test_icon_fallback_only_changes_melania_bundle_apps(self):
        cfg = (ROOT / "melania-config/src/lib.rs").read_text()
        self.assertIn('std::env::var_os("MELANIA_BUNDLE_DATA_DIR")', cfg)
        self.assertIn('dir.join("Melania-Preview/index.theme").is_file()', cfg)
        self.assertIn('"Melania-Preview"', cfg)
        self.assertIn('else {"Adwaita"}', cfg)

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

    def test_vendored_mint_runtime_is_packaged_without_glibc_or_drivers(self):
        with tempfile.TemporaryDirectory(prefix="melania-mint-runtime-") as tmp:
            root = Path(tmp)
            binaries = self.make_binaries(root)
            library = root / "libgtk4-layer-shell.so.0.0.0"
            library.write_bytes(b"\x7fELF" + b"private-gtk4-layer-shell-fixture")
            output = root / "output"
            manifest = packager.stage(binaries, output, "b" * 40,
                                      gtk_layer_shell_lib=library)
            self.assertTrue(manifest["vendored_gtk4_layer_shell"])
            self.assertEqual(manifest["vendored_libraries"],
                             ["lib/libgtk4-layer-shell.so.0"])
            self.assertNotIn("libc.so.6", manifest["files_sha256"])
            bundled = output / packager.BUNDLE_NAME / "lib/libgtk4-layer-shell.so.0"
            self.assertEqual(bundled.read_bytes(), library.read_bytes())
            self.assertEqual(manifest["files_sha256"]["lib/libgtk4-layer-shell.so.0"],
                             packager.sha256(bundled))
            with tarfile.open(output / (packager.BUNDLE_NAME + ".tar.gz"),
                              "r:gz") as archive:
                paths = set(archive.getnames())
                self.assertIn(packager.BUNDLE_NAME + "/lib/libgtk4-layer-shell.so.0",
                              paths)

    def test_invalid_vendored_runtime_fails_without_output(self):
        with tempfile.TemporaryDirectory(prefix="melania-mint-bad-lib-") as tmp:
            root = Path(tmp)
            binaries = self.make_binaries(root)
            library = root / "not-an-elf"
            library.write_bytes(b"invalid")
            output = root / "output"
            with self.assertRaises(ValueError):
                packager.stage(binaries, output, "unknown",
                               gtk_layer_shell_lib=library)
            self.assertFalse(output.exists())

    def test_mint_launcher_isolated_library_and_system_glibc(self):
        launcher = (ROOT / "scripts/run-melania-bundle.sh").read_text()
        builder = (ROOT / "scripts/build-mint-layer-shell.sh").read_text()
        self.assertIn("LD_LIBRARY_PATH", launcher)
        self.assertIn("libgtk4-layer-shell.so.0", launcher)
        self.assertIn("BLOCKED: missing/incompatible Linux runtime", launcher)
        self.assertIn("536ff516ed68b9bb34afc4c07f942a54b2b4b03f", builder)
        self.assertIn("--prefix", builder)
        self.assertNotIn("sudo ", builder)
        for script in ("scripts/run-melania-bundle.sh",
                       "scripts/build-mint-layer-shell.sh"):
            result = subprocess.run(["bash", "-n", str(ROOT / script)],
                                    capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_portable_wallpaper_respects_user_preference(self):
        text = (ROOT / "melania-desktop-surface/src/main.rs").read_text()
        self.assertIn('std::env::var_os("MELANIA_WALLPAPER")', text)
        self.assertIn('prefs.desktop_wallpaper', text)
        self.assertIn('std::env::var_os("MELANIA_BUNDLE_DATA_DIR")', text)
        self.assertLess(text.index('prefs.desktop_wallpaper'),
                        text.index('std::env::var_os("MELANIA_BUNDLE_DATA_DIR")'))


if __name__ == "__main__":
    unittest.main(verbosity=2)
