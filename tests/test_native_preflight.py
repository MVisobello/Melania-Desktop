#!/usr/bin/env python3
"""Native session preflight must never claim DRM/KMS works from file discovery."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
PROBE = ROOT / "scripts/probe-native-session.py"
LAUNCH = ROOT / "scripts/melania-session"

spec = importlib.util.spec_from_file_location("melania_native_preflight", PROBE)
preflight = importlib.util.module_from_spec(spec)
spec.loader.exec_module(preflight)


class NativePreflightTests(unittest.TestCase):
    def test_empty_root_fails_closed_without_hardware_changes(self):
        with tempfile.TemporaryDirectory(prefix="melania-native-fixture-") as tmp:
            root = Path(tmp)
            report = preflight.probe(root, {})
            self.assertFalse(report["native_session_ready"])
            self.assertFalse(report["native_backend_compiled"])
            self.assertEqual(report["backend"], "winit-nested")
            self.assertEqual(report["drm_cards"], [])
            self.assertEqual(report["connectors"], [])
            self.assertEqual(report["input_event_node_count"], 0)
            self.assertFalse((root / "dev").exists(), "probe cannot create devices")

    def test_synthetic_drm_connector_and_input_do_not_claim_native_support(self):
        with tempfile.TemporaryDirectory(prefix="melania-native-fixture-") as tmp:
            root = Path(tmp)
            card = root / "dev/dri/card0"
            card.parent.mkdir(parents=True)
            card.touch()
            connector = root / "sys/class/drm/card0-HDMI-A-1"
            connector.mkdir(parents=True)
            (connector / "status").write_text("connected\n")
            (connector / "modes").write_text("1920x1080\n1280x720\n")
            input_node = root / "dev/input/event0"
            input_node.parent.mkdir(parents=True)
            input_node.touch()
            private_runtime = root / "run/user/private"
            private_runtime.mkdir(parents=True)
            private_runtime.chmod(0o700)
            env = {
                "XDG_RUNTIME_DIR": str(private_runtime),
                "XDG_SESSION_TYPE": "tty",
                "XDG_SESSION_ID": "7",
            }
            report = preflight.probe(root, env)
            self.assertEqual(len(report["drm_cards"]), 1)
            self.assertFalse(report["drm_cards"][0]["character_device"])
            self.assertIsNone(report["drm_cards"][0]["filesystem_access"])
            self.assertEqual(report["connectors"][0]["status"], "connected")
            self.assertEqual(report["connectors"][0]["advertised_mode_count"], 2)
            self.assertEqual(report["input_event_node_count"], 1)
            self.assertTrue(report["session"]["xdg_runtime_dir_valid"])
            self.assertFalse(report["native_session_ready"])
            self.assertEqual(report["status"], "blocked")
            self.assertEqual((connector / "status").read_text(), "connected\n")

    def test_insecure_runtime_is_reported_without_chmod(self):
        with tempfile.TemporaryDirectory(prefix="melania-native-fixture-") as tmp:
            root = Path(tmp)
            runtime = root / "runtime"
            runtime.mkdir(mode=0o755)
            runtime.chmod(0o755)
            r = preflight.probe(root, {"XDG_RUNTIME_DIR": str(runtime)})
            self.assertFalse(r["session"]["xdg_runtime_dir_valid"])
            self.assertEqual(runtime.stat().st_mode & 0o777, 0o755)
            self.assertFalse(r["native_session_ready"])

    def test_json_and_main_launcher_are_fail_closed(self):
        with tempfile.TemporaryDirectory(prefix="melania-native-fixture-") as tmp:
            direct = subprocess.run(
                [sys.executable, str(PROBE), "--root", tmp, "--json"],
                capture_output=True, text=True,
            )
            self.assertEqual(direct.returncode, 78, direct.stderr)
            result = json.loads(direct.stdout)
            self.assertEqual(result["schema_version"], 1)
            self.assertEqual(result["native_backend_compiled"], False)
            env = os.environ.copy()
            env.pop("DISPLAY", None)
            env.pop("WAYLAND_DISPLAY", None)
            wrapper = subprocess.run(
                ["bash", str(LAUNCH), "--probe-native"],
                env=env, capture_output=True, text=True,
            )
            self.assertEqual(wrapper.returncode, 78, wrapper.stderr)
            self.assertFalse(json.loads(wrapper.stdout)["native_session_ready"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
