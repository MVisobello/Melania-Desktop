#!/usr/bin/env python3
"""Nested desktop screenshots are real host Xvfb pixels, not mocked renders."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
CI=(ROOT/".github/workflows/full-desktop-e2e.yml").read_text()
SMOKE=(ROOT/"scripts/smoke-full-desktop-wayland.sh").read_text()

class RealNestedUI(unittest.TestCase):
    def test_builds_every_real_desktop_component(self):
        for name in ("melania-compositor", "melania-desktop-surface",
                     "melania-panel", "melania-files", "melania-settings",
                     "melania-winctl"):
            self.assertIn(name,CI)
            self.assertIn(name,SMOKE)
        self.assertIn("xvfb-run -a",CI)
        self.assertIn("python3-pil",CI)
        self.assertIn("xdotool",CI)
        self.assertIn("LIBGL_ALWAYS_SOFTWARE",CI)
        self.assertIn('MELANIA_DEBUG_RENDER: "1"',CI)
        self.assertIn("dbus-run-session -- bash",CI)
        self.assertIn("dbus-x11",CI)
        self.assertIn("melania-real-full-desktop-screens",CI)

    def test_ui_smoke_checks_rendered_pixels_not_just_process_presence(self):
        for term in ("scrot -z -o reports/nested-wayland-full-desktop.png",
                     "from PIL import Image",
                     "tools/check_shared_window_chrome.py",
                     'target/debug/melania-winctl activate "$SETTINGS_ID"',
                     "MELANIA_RENDER .*xdg=[1-9]",
                     "MELANIA_FILES_LISTING_READY",
                     "MELANIA_START_SEARCH_RECEIVED",
                     "target/debug/melania-winctl quit",
                     "target/debug/melania-winctl workspace 2",
                     "target/debug/melania-winctl workspace 1",
                     "desktop_wallpaper=",
                     "panel_position=top",
                     "PASS: desktop layer, panel, Files, Settings",
                     "No CI settings test may mutate developer or runner personal"):
            self.assertIn(term,SMOKE)

    def test_no_login_manager_or_host_kms_access_in_workflow(self):
        for forbidden in ("MELANIA_EXPERIMENTAL_SEAT=1", "/dev/dri/card0",
                          "-device vfio", "loginctl terminate-session",
                          "chvt ", "atomic_commit"):
            self.assertNotIn(forbidden,CI)
        self.assertIn("XDG_RUNTIME_DIR",SMOKE)
        self.assertIn("XDG_CONFIG_HOME",SMOKE)

if __name__=="__main__":
    unittest.main()
