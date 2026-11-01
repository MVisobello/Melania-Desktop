#!/usr/bin/env python3
"""Stage17: real compiled Rust backend must run in isolated QEMU, never host."""
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
HOST = (ROOT / "scripts/run-qemu-melania-native.sh").read_text()
GUEST = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()
CI = (ROOT / ".github/workflows/qemu-melania-native.yml").read_text()
NATIVE = (ROOT / "melania-native/src/main.rs").read_text()

class NativeGuestEndToEnd(unittest.TestCase):
    def test_host_refuses_without_explicit_vm_consent(self):
        cmd = subprocess.run(["bash", "scripts/run-qemu-melania-native.sh"],
                             cwd=ROOT, env={"PATH":"/usr/bin:/bin"},
                             capture_output=True, text=True, timeout=5)
        self.assertEqual(cmd.returncode, 78, cmd.stderr)
        self.assertIn("MELANIA_QEMU_NATIVE_HOST_BLOCKED", cmd.stderr)

    def test_binary_is_real_melania_native_built_by_cargo_and_isolated(self):
        self.assertIn("cargo build -p melania-native", CI)
        self.assertIn("cargo test -p melania-native --all-targets", CI)
        self.assertIn("target/debug/melania-native", CI)
        self.assertIn('target/debug/melania-native', HOST)
        self.assertIn("genisoimage -quiet -R -J -V MELANIA_BIN", HOST)
        self.assertIn("accel=tcg", HOST)
        self.assertIn("-vga virtio", HOST)
        self.assertIn("-display none", HOST)
        self.assertIn("sha256sum -c verify.sha256", HOST)
        self.assertIn("timeout --signal=TERM", HOST)
        self.assertNotIn("accel=kvm", HOST)
        self.assertNotIn("vfio", HOST)
        self.assertNotIn("/dev/dri/", HOST)

    def test_guest_requires_dedicated_vm_tty_and_seatd(self):
        for needle in ("QEMU_DMI_required", "/dev/tty1", "/dev/dri/card0",
                       "LIBSEAT_BACKEND=seatd", "systemctl start seatd",
                       "MELANIA_EXPERIMENTAL_SEAT=1",
                       "MELANIA_KMS_TEST_ONLY=YES_QEMU_VM",
                       "unset DISPLAY WAYLAND_DISPLAY",
                       "--experimental-kms-test", "XDG_RUNTIME_DIR"):
            self.assertIn(needle, GUEST)
        self.assertIn('assert_dedicated_console();\n    assert_qemu_test_guest();', NATIVE)
        self.assertIn('NATIVE_SESSION_READY=false', NATIVE)

    def test_real_test_must_pass_and_never_claim_restore(self):
        for needle in ("Melania QEMU atomic TEST_ONLY passed:",
                       "baseline_unchanged=true", "resources_unchanged=true",
                       "restore_ready=false visible=false",
                       "MELANIA_QEMU_NATIVE_KMS_PASS=true",
                       "MELANIA_QEMU_NATIVE_GUEST_EXIT=0",
                       "MELANIA_QEMU_NATIVE_RESULT=FAIL",
                       "MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false"):
            self.assertIn(needle, HOST)
        self.assertIn("MELANIA_NATIVE_VISIBLE_SCANOUT=false", GUEST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false", GUEST)
        self.assertIn("qemu-real-melania-native-evidence", CI)

    def test_readonly_drm_ioctls_guarded_against_host_execution(self):
        for forbidden in ("DRM_IOCTL_MODE_ATOMIC", "drmModeSetCrtc", "queue_buffer(",
                          "page_flip(", "atomic_commit(", "chvt", "set_current_vt("):
            self.assertNotIn(forbidden, GUEST)

if __name__ == "__main__":
    unittest.main()
