#!/usr/bin/env python3
"""Stage21: QEMU-only GETFB2 -> GEM_CLOSE real-kernel lifecycle gate."""
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
SOURCE=(ROOT/"scripts/melania-qemu-fb2-probe.c").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
GATE=(ROOT/"melania-native/src/rollback_audit.rs").read_text()


class FramebufferGemStage21(unittest.TestCase):
    def test_gate_rejects_host_gui_without_qemu_dmi_consent_and_vt(self):
        for pattern in ('qemu_dmi()', '"MELANIA_FB2_QEMU_ONLY"',
                        '"YES_DISPOSABLE_VM"', 'getenv("DISPLAY")',
                        'getenv("WAYLAND_DISPLAY")',
                        'S_ISCHR(tty.st_mode)', 'ttyname(STDIN_FILENO)',
                        "MELANIA_FB2_BLOCKED=REAL_LINUX_VT_REQUIRED"):
            self.assertIn(pattern,SOURCE)
        self.assertIn('MELANIA_FB2_QEMU_ONLY=YES_DISPOSABLE_VM',GUEST)
        self.assertIn('< /dev/tty1',GUEST)

    def test_getfb2_returns_gem_handles_closed_exactly_once(self):
        for pattern in ('drmModeGetPlaneResources(fd)', 'drmModeGetPlane(fd',
                        'drmModeGetFB2(fd, fb_id)',
                        'struct drm_gem_close request',
                        'ioctl(fd, DRM_IOCTL_GEM_CLOSE, &request)',
                        'drmModeFreeFB2(fb)',
                        'drmModeFreePlaneResources(resources)',
                        'if (distinct[j] == value) duplicate = 1',
                        'if (duplicate) continue',
                        'acquired != released',
                        'MELANIA_FB2_ROUNDTRIP=PASS',
                        'MELANIA_FB2_ROLLBACK_READY=false'):
            self.assertIn(pattern,SOURCE)
        self.assertIn('MAX_PLANES 128U',SOURCE)
        self.assertIn('MAX_GEM_HANDLES 512U',SOURCE)
        self.assertIn('if (!ok || !seen || !acquired',SOURCE)

    def test_no_visual_drm_mutations_or_false_session_readiness(self):
        for name in ('drmModeSetCrtc(', 'drmModeAddFB(', 'drmModeAddFB2(',
                     'drmModeRmFB(', 'drmModeAtomicCommit(',
                     'drmModePageFlip(', 'drmModeCreatePropertyBlob(',
                     'DRM_IOCTL_MODE_ATOMIC'):
            self.assertNotIn(name,SOURCE)
        self.assertIn('visible_kms_commit_allowed(&self) -> bool {false}',GATE)
        self.assertIn('MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false',HOST)
        self.assertIn('MELANIA_NATIVE_VISIBLE_SCANOUT=false',GUEST)

    def test_in_guest_real_binary_run_after_rust_test_before_vt(self):
        self.assertLess(GUEST.index('MELANIA_QEMU_NATIVE_KMS_PASS=true'),
                        GUEST.index('melania-fb2-probe'))
        self.assertLess(GUEST.index('MELANIA_QEMU_NATIVE_FB2_GEM_CLOSE_PASS=true'),
                        GUEST.index('python3 "$vt_probe"'))
        for marker in ('MELANIA_FB2_ROUNDTRIP=PASS',
                       'MELANIA_QEMU_NATIVE_FB2_GEM_CLOSE_PASS=true',
                       'MELANIA_FB2_ROLLBACK_READY=false'):
            self.assertIn(marker,HOST if marker!='MELANIA_QEMU_NATIVE_FB2_GEM_CLOSE_PASS=true' else GUEST+HOST)
        self.assertIn('"melania-fb2-probe=$fb_probe"',HOST)

    def test_host_and_guest_shell_syntax_checked_independently(self):
        for path in ("scripts/run-qemu-melania-native.sh",
                     "scripts/melania-qemu-native-guest.sh"):
            result = subprocess.run(["bash", "-n", str(ROOT/path)],
                                    capture_output=True, text=True, timeout=5)
            self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("bash -n scripts/run-qemu-melania-native.sh", CI)
        self.assertIn("bash -n scripts/melania-qemu-native-guest.sh", CI)

    def test_guest_serial_crlf_is_accepted_without_allowing_zero_handles(self):
        # QEMU's serial console emits CRLF; grep '$' without allowing CR
        # incorrectly marks a successful GEM acquisition as failed.
        expression = r'^MELANIA_FB2_GEM_HANDLES_ACQUIRED=[1-9][0-9]*[[:space:]]*$'
        self.assertIn(expression, HOST)
        for value, expected in (("1\\r\\n", 0), ("13\\n", 0),
                                ("0\\r\\n", 1), ("bogus\\r\\n", 1)):
            run = subprocess.run(["grep", "-E", expression],
                                 input="MELANIA_FB2_GEM_HANDLES_ACQUIRED="+value,
                                 capture_output=True, text=True, timeout=5)
            self.assertEqual(run.returncode, expected, value)

    def test_compiled_with_libdrm_in_existing_vm_not_on_host(self):
        self.assertIn('scripts/melania-qemu-fb2-probe.c',CI)
        self.assertIn('-o target/debug/melania-qemu-fb2-probe -ldrm',CI)
        self.assertIn("sha256sum -c verify.sha256",HOST)
        self.assertIn("accel=tcg",HOST)
        self.assertIn("-vga virtio",HOST)
        self.assertNotIn("vfio",HOST)
        self.assertIn("timeout --signal=TERM",HOST)


if __name__=="__main__":
    unittest.main()
