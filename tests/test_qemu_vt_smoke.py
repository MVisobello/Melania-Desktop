#!/usr/bin/env python3
"""QEMU VT smoke contract + simulated recovery paths, no host GPU/VM required."""
import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
GUEST = ROOT / "scripts/melania-vt-guest.py"
RUNNER = ROOT / "scripts/run-qemu-vt-smoke.sh"
WORKFLOW = ROOT / ".github/workflows/qemu-vt-smoke.yml"
spec = importlib.util.spec_from_file_location("melania_guest_vt", GUEST)
guest = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guest)


class QemuVtSmokeTests(unittest.TestCase):
    def test_guest_roundtrip_uses_real_vt_ioctls_and_restores_original(self):
        vt = [1]
        calls = []

        def fake_ioctl(fd, cmd, arg, mutate_flag=False):
            calls.append(cmd)
            if cmd == guest.VT_GETSTATE:
                arg[:2] = vt[0].to_bytes(2, "little")
            elif cmd == guest.VT_ACTIVATE:
                vt[0] = arg
            elif cmd == guest.VT_WAITACTIVE:
                self.assertEqual(vt[0], arg)
            else:
                self.fail("Unknown tty ioctl")

        output = io.StringIO()
        with mock.patch.object(guest, "dmi_is_qemu", return_value=True), \
             mock.patch.object(guest, "virtual_vga_present", return_value=True), \
             mock.patch.object(guest, "drm_card_present", return_value=False), \
             mock.patch.object(guest.os, "open", return_value=23), \
             mock.patch.object(guest.os, "close"), \
             mock.patch.object(guest.fcntl, "ioctl", side_effect=fake_ioctl), \
             mock.patch.object(guest.time, "sleep"), \
             contextlib.redirect_stdout(output):
            guest.main()
        self.assertEqual(vt[0], 1)
        self.assertEqual(calls.count(guest.VT_ACTIVATE), 2)
        self.assertIn("MELANIA_QEMU_VT_SWITCH=true", output.getvalue())
        self.assertIn("MELANIA_QEMU_VT_RESTORED=1", output.getvalue())
        self.assertIn("MELANIA_QEMU_VT_PASS=true", output.getvalue())

    def test_restoration_attempted_when_first_switch_raises(self):
        calls = []
        with mock.patch.object(guest, "dmi_is_qemu", return_value=True), \
             mock.patch.object(guest, "virtual_vga_present", return_value=True), \
             mock.patch.object(guest, "drm_card_present", return_value=False), \
             mock.patch.object(guest.os, "open", return_value=12), \
             mock.patch.object(guest.os, "close"), \
             mock.patch.object(guest, "active_vt", return_value=1), \
             mock.patch.object(guest, "switch_vt") as switch:
            def switch_side_effect(fd, target):
                calls.append(target)
                if target == 2:
                    raise RuntimeError("GPU/VT unavailable")
            switch.side_effect = switch_side_effect
            with self.assertRaisesRegex(RuntimeError, "unavailable"):
                guest.main()
        self.assertEqual(calls, [2, 1])

    def test_qemu_without_drm_driver_still_reports_vga_separately(self):
        self.assertIn("MELANIA_QEMU_VGA_ENUMERATED=true", GUEST.read_text())
        self.assertIn("MELANIA_QEMU_DRM_ENUMERATED=", GUEST.read_text())
        self.assertIn("MELANIA_QEMU_VGA_ENUMERATED=true", RUNNER.read_text())
        self.assertIn("MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false", RUNNER.read_text())

    def test_guest_refuses_non_qemu_before_touching_console(self):
        with mock.patch.object(guest, "dmi_is_qemu", return_value=False), \
             mock.patch.object(guest.os, "open") as opened:
            with self.assertRaisesRegex(RuntimeError, "QEMU DMI"):
                guest.main()
            opened.assert_not_called()

    def test_host_script_requires_opt_in_before_touching_vm(self):
        env = {"PATH": "/usr/bin:/bin"}
        result = subprocess.run(["bash", str(RUNNER)], env=env,
                                text=True, capture_output=True)
        self.assertEqual(result.returncode, 78, result.stderr)
        self.assertIn("MELANIA_QEMU_HOST_BLOCKED", result.stderr)

    def test_isolated_vm_checksums_timeout_and_explicit_non_kms_result(self):
        script = RUNNER.read_text()
        workflow = WORKFLOW.read_text()
        for term in ("MELANIA_QEMU_TTY_SMOKE", "YES_DISPOSABLE_QEMU",
                     "qemu-system-x86_64", "accel=tcg", "-display none",
                     "-vga std", "sha256sum -c verify.sha256",
                     "timeout --signal=TERM", "MELANIA_QEMU_TTY_RESULT=PASS",
                     "MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false"):
            self.assertIn(term, script)
        self.assertIn("cloud-images.ubuntu.com/noble/current", script)
        self.assertIn("if: always()", workflow)
        self.assertIn("qemu-vt-evidence", workflow)
        self.assertNotIn("accel=kvm", script)
        self.assertNotIn("vfio", script)


if __name__ == "__main__":
    unittest.main()
