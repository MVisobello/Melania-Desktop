#!/usr/bin/env python3
"""Non-invasive tests for the nested session launcher (no compositor started)."""
import os
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/run-melania-v018-nested.sh"

class NestedSessionPreflightTests(unittest.TestCase):
    def run_script(self, *args, **env_updates):
        env = os.environ.copy()
        env.pop("DISPLAY", None)
        env.pop("WAYLAND_DISPLAY", None)
        env.update(env_updates)
        return subprocess.run(["bash", str(SCRIPT), *args], env=env,
                              text=True, capture_output=True, timeout=15)

    def test_no_host_display_is_rejected(self):
        result = self.run_script("--preflight")
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertIn("NO_HOST_DISPLAY", result.stderr)

    def test_unknown_option_does_not_start_compositor(self):
        result = self.run_script("--invalid")
        self.assertEqual(result.returncode, 2)
        self.assertIn("Usage:", result.stderr)

    def test_extra_argument_is_rejected(self):
        result = self.run_script("--preflight", "extra")
        self.assertEqual(result.returncode, 2)

if __name__ == "__main__":
    unittest.main()
