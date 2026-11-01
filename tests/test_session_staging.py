#!/usr/bin/env python3
"""Native-session packaging stays closed; nested preview stages as an app only."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest

ROOT=Path(__file__).resolve().parents[1]
SCRIPT=ROOT/"scripts/stage-melania-session.py"
spec=importlib.util.spec_from_file_location("melania_stager",SCRIPT)
stager=importlib.util.module_from_spec(spec)
spec.loader.exec_module(stager)

class PackagingGates(unittest.TestCase):
    def run_script(self,flag):
        return subprocess.run([sys.executable,str(SCRIPT),flag],cwd=ROOT,
                              capture_output=True,text=True,timeout=10)

    def test_current_build_never_claims_a_login_session(self):
        p=self.run_script("--status")
        self.assertEqual(p.returncode,0,p.stderr)
        state=json.loads(p.stdout)
        self.assertFalse(state["native_ready"])
        self.assertFalse(state["login_manager_installable"])
        self.assertTrue(state["nested_preview_available"])
        self.assertTrue(state["fail_closed_markers_present"])
        self.assertEqual(len(state["blocking_reasons"]),4)

    def test_native_session_staging_is_a_hard_failure(self):
        p=self.run_script("--stage-native")
        self.assertEqual(p.returncode,78,p.stdout+p.stderr)
        self.assertIn("NATIVE_LOGIN_SESSION_BLOCKED",p.stdout)

    def test_preview_stages_an_application_not_login_manager_entry(self):
        p=self.run_script("--stage-preview")
        self.assertEqual(p.returncode,0,p.stdout+p.stderr)
        output=stager.STAGING/"org.melaniadesktop.NestedPreview.desktop"
        self.assertTrue(output.is_file())
        self.assertEqual(output.parent.name,"applications")
        self.assertTrue(output.is_relative_to(ROOT/"target"))
        self.assertFalse(any(part in {"wayland-sessions","xsessions"}
                             for part in output.parts))
        text=output.read_text()
        self.assertIn("Type=Application",text)
        self.assertIn("Name=Melania Desktop (Nested Preview)",text)
        self.assertIn("Exec=/usr/bin/env bash ",text)
        self.assertIn(" --nested",text)
        self.assertIn("X-Melania-NativeReady=false",text)
        self.assertNotIn("Exec=melania-native --experimental-kms-test",text)
        self.assertNotIn("DesktopNames=Melania",text)
        self.assertIn("MELANIA_NATIVE_LOGIN_SESSION_READY=false",p.stdout)

    def test_rejects_unsafe_checkout_paths_or_missing_preview(self):
        original=stager.PREVIEW
        try:
            stager.PREVIEW=Path("/home/user/My Project/scripts/melania-session")
            with self.assertRaises(ValueError):
                stager.preview_desktop_entry()
            stager.PREVIEW=ROOT/"path-that-does-not-exist"
            with self.assertRaises(FileNotFoundError):
                stager.preview_desktop_entry()
        finally:
            stager.PREVIEW=original

if __name__=="__main__":
    unittest.main()
