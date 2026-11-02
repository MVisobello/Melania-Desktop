#!/usr/bin/env python3
"""Native-session safety gates and explicit isolated preview registration."""
from pathlib import Path
import os
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
LAUNCH=ROOT/"scripts/melania-session"
INSTALL=ROOT/"scripts/install-melania-preview-launcher.sh"
UNINSTALL=ROOT/"scripts/uninstall-melania-preview-launcher.sh"

class SessionGate(unittest.TestCase):
    def test_native_is_explicitly_unavailable_not_fake_success(self):
        env=os.environ.copy()
        env.pop("DISPLAY",None)
        env.pop("WAYLAND_DISPLAY",None)
        r=subprocess.run(["bash",str(LAUNCH),"--check-native"],env=env,
                         capture_output=True,text=True)
        self.assertEqual(r.returncode,78,r.stderr)
        self.assertIn("NATIVE_BACKEND_UNAVAILABLE",r.stderr)
        self.assertIn("DRM/KMS",r.stderr)

    def test_nested_requires_an_existing_host_display(self):
        env=os.environ.copy()
        env.pop("DISPLAY",None)
        env.pop("WAYLAND_DISPLAY",None)
        r=subprocess.run(["bash",str(LAUNCH),"--nested"],env=env,
                         capture_output=True,text=True)
        self.assertEqual(r.returncode,78,r.stderr)
        self.assertIn("BLOCKED",r.stderr)

    def test_optional_installer_writes_only_user_preview_entry(self):
        with tempfile.TemporaryDirectory(prefix="melania-user-preview-") as tmp:
            home=Path(tmp)
            env=os.environ.copy()
            env["HOME"]=str(home)
            env["XDG_DATA_HOME"]=str(home/"XDG Data")
            r=subprocess.run(["bash",str(INSTALL)],env=env,capture_output=True,text=True)
            self.assertEqual(r.returncode,0,r.stderr)
            launcher=home/".local/bin/melania-session"
            desktop=home/"XDG Data/applications/org.melaniadesktop.Nested.desktop"
            self.assertTrue(launcher.is_file())
            self.assertTrue(desktop.is_file())
            self.assertIn("--nested",desktop.read_text())
            self.assertIn("X-Melania-Nested-Preview=true",desktop.read_text())
            self.assertFalse((home/"XDG Data/wayland-sessions").exists())
            native=subprocess.run([str(launcher),"--check-native"],env=env,capture_output=True,text=True)
            self.assertEqual(native.returncode,78,native.stderr)
            # A second installation should be idempotent in this private sandbox.
            again=subprocess.run(["bash",str(INSTALL)],env=env,capture_output=True,text=True)
            self.assertEqual(again.returncode,0,again.stderr)

    def test_nested_preview_uninstall_is_idempotent_and_leaves_host_alone(self):
        with tempfile.TemporaryDirectory(prefix="melania-uninstall-") as tmp:
            home=Path(tmp)
            env=os.environ.copy()
            env["HOME"]=str(home)
            env["XDG_DATA_HOME"]=str(home/"XDG Data")
            launcher=home/".local/bin/melania-session"
            desktop=home/"XDG Data/applications/org.melaniadesktop.Nested.desktop"
            for script in (INSTALL,UNINSTALL,UNINSTALL):
                result=subprocess.run(["bash",str(script)],env=env,
                                      capture_output=True,text=True,timeout=15)
                self.assertEqual(result.returncode,0,result.stderr)
            self.assertFalse(launcher.exists())
            self.assertFalse(desktop.exists())
            self.assertFalse((home/"XDG Data/wayland-sessions").exists())
            self.assertFalse((home/"XDG Data/xsessions").exists())

    def test_uninstall_refuses_tampered_desktop_entry_without_partial_removal(self):
        with tempfile.TemporaryDirectory(prefix="melania-uninstall-block-") as tmp:
            home=Path(tmp)
            env=os.environ.copy()
            env["HOME"]=str(home)
            env["XDG_DATA_HOME"]=str(home/"XDG Data")
            launcher=home/".local/bin/melania-session"
            desktop=home/"XDG Data/applications/org.melaniadesktop.Nested.desktop"
            result=subprocess.run(["bash",str(INSTALL)],env=env,
                                  capture_output=True,text=True,timeout=15)
            self.assertEqual(result.returncode,0,result.stderr)
            desktop.write_text(desktop.read_text().replace(
                "X-Melania-Nested-Preview=true","X-Melania-Nested-Preview=false"))
            result=subprocess.run(["bash",str(UNINSTALL)],env=env,
                                  capture_output=True,text=True,timeout=15)
            self.assertEqual(result.returncode,4,result.stderr)
            self.assertTrue(launcher.is_file())
            self.assertTrue(desktop.is_file())

    def test_uninstall_does_not_follow_symlinked_launcher(self):
        with tempfile.TemporaryDirectory(prefix="melania-uninstall-link-") as tmp:
            home=Path(tmp)
            env=os.environ.copy()
            env["HOME"]=str(home)
            env["XDG_DATA_HOME"]=str(home/"XDG Data")
            result=subprocess.run(["bash",str(INSTALL)],env=env,
                                  capture_output=True,text=True,timeout=15)
            self.assertEqual(result.returncode,0,result.stderr)
            launcher=home/".local/bin/melania-session"
            launcher.unlink()
            outside=home/"important-file"
            outside.write_text("do not delete")
            launcher.symlink_to(outside)
            result=subprocess.run(["bash",str(UNINSTALL)],env=env,
                                  capture_output=True,text=True,timeout=15)
            self.assertEqual(result.returncode,4,result.stderr)
            self.assertEqual(outside.read_text(),"do not delete")
            self.assertTrue(launcher.is_symlink())


    def test_install_refuses_unrelated_desktop_entry_before_writing_launcher(self):
        with tempfile.TemporaryDirectory(prefix="melania-install-collision-") as tmp:
            home=Path(tmp)
            env=os.environ.copy()
            env["HOME"]=str(home)
            env["XDG_DATA_HOME"]=str(home/"XDG Data")
            desktop=home/"XDG Data/applications/org.melaniadesktop.Nested.desktop"
            desktop.parent.mkdir(parents=True)
            desktop.write_text("[Desktop Entry]\\nName=Unrelated application\\n")
            result=subprocess.run(["bash",str(INSTALL)],env=env,
                                  capture_output=True,text=True,timeout=15)
            self.assertEqual(result.returncode,4,result.stderr)
            self.assertEqual(desktop.read_text(),"[Desktop Entry]\\nName=Unrelated application\\n")
            self.assertFalse((home/".local/bin/melania-session").exists())

    def test_install_refuses_symlinks_without_touching_the_target(self):
        with tempfile.TemporaryDirectory(prefix="melania-install-link-") as tmp:
            home=Path(tmp)
            env=os.environ.copy()
            env["HOME"]=str(home)
            env["XDG_DATA_HOME"]=str(home/"XDG Data")
            bin_dir=home/".local/bin"
            bin_dir.mkdir(parents=True)
            outside=home/"important-file"
            outside.write_text("keep this")
            launcher=bin_dir/"melania-session"
            launcher.symlink_to(outside)
            result=subprocess.run(["bash",str(INSTALL)],env=env,
                                  capture_output=True,text=True,timeout=15)
            self.assertEqual(result.returncode,4,result.stderr)
            self.assertEqual(outside.read_text(),"keep this")
            self.assertTrue(launcher.is_symlink())
            self.assertFalse((home/"XDG Data/applications/org.melaniadesktop.Nested.desktop").exists())


if __name__=="__main__":
    unittest.main(verbosity=2)
