#!/usr/bin/env python3
"""Guest software-renderer selection tests: no real GPU or host modifications."""
from pathlib import Path
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts/run-melania-bundle.sh"
BINARIES = (
    "melania-compositor", "melania-desktop-surface", "melania-panel",
    "melania-files", "melania-settings", "melania-winctl",
    "melania-notifyd", "melania-notifyctl",
)


class VirtualBoxSoftwareRenderer(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="melania-vbox-preflight-")
        self.addCleanup(self.tmp.cleanup)
        path = Path(self.tmp.name)
        self.app = path / "preview"
        (self.app / "bin").mkdir(parents=True)
        (self.app / "share").mkdir()
        (self.app / "share/wallpaper.png").write_bytes(b"preview-placeholder")
        run = self.app / "run.sh"
        run.write_text(SCRIPT.read_text())
        run.chmod(0o755)
        for name in BINARIES:
            exe = self.app / "bin" / name
            exe.write_text("#!/bin/sh\nexit 0\n")
            exe.chmod(0o755)
        tools = path / "tools"
        tools.mkdir()
        for tool, body in {
            "ldd": "#!/bin/sh\nprintf 'libc.so.6 => /lib/libc.so.6\\n'\n",
            "dbus-run-session": (
                "#!/bin/sh\n"
                "echo \"VM_TEST_RENDERER=$GSK_RENDERER:$GALLIUM_DRIVER:$MESA_LOADER_DRIVER_OVERRIDE\"\n"
            ),
            "systemd-detect-virt": "#!/bin/sh\necho oracle\n",
        }.items():
            target = tools / tool
            target.write_text(body)
            target.chmod(0o755)
        self.env = os.environ.copy()
        self.env["PATH"] = str(tools) + os.pathsep + self.env["PATH"]
        self.env["DISPLAY"] = ":42"
        self.env.pop("WAYLAND_DISPLAY", None)
        self.env["XDG_RUNTIME_DIR"] = str(path)
        for key in ("GSK_RENDERER", "GALLIUM_DRIVER", "MESA_LOADER_DRIVER_OVERRIDE",
                    "LIBGL_ALWAYS_SOFTWARE", "MELANIA_SOFTWARE_RENDER"):
            self.env.pop(key, None)

    def run_preview(self, args=(), **env):
        environ = dict(self.env, **env)
        return subprocess.run(
            ["bash", str(self.app / "run.sh"), *args], env=environ,
            capture_output=True, text=True, timeout=15
        )

    def test_detects_virtualbox_in_default_preflight(self):
        p = self.run_preview(("--check",))
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertIn("MELANIA_NESTED_BUNDLE_PREFLIGHT_OK=true", p.stdout)
        self.assertIn("MELANIA_RENDERING_BACKEND=software", p.stdout)

    def test_user_can_disable_virtualbox_auto_detection(self):
        p = self.run_preview(("--check",), MELANIA_SOFTWARE_RENDER="0")
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertIn("MELANIA_RENDERING_BACKEND=host", p.stdout)

    def test_explicit_software_switch_exports_guest_safe_renderer(self):
        p = self.run_preview(("--software",), MELANIA_SOFTWARE_RENDER="0")
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertIn("MELANIA_RENDERING_BACKEND=software", p.stdout)
        self.assertIn("VM_TEST_RENDERER=cairo:llvmpipe:llvmpipe", p.stdout)

    def test_invalid_mode_is_blocked(self):
        p = self.run_preview(("--check",), MELANIA_SOFTWARE_RENDER="broken")
        self.assertEqual(p.returncode, 2, p.stderr)
        self.assertIn("must be 0, 1 or auto", p.stderr)

    def test_compositor_reports_child_start_and_exit(self):
        content = (ROOT / "melania-compositor/src/main.rs").read_text()
        self.assertIn("MELANIA_CLIENT_STARTED", content)
        self.assertIn("MELANIA_CLIENT_EXIT", content)


if __name__ == "__main__":
    unittest.main(verbosity=2)
