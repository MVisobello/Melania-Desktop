"""Regression for real, on-time QEMU managed-role KMS serial evidence.

This is only a structural guard. The full isolated QEMU test must still
screendump six different framebuffer presentations and compare real pixels.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
GUEST = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()
HOST = (ROOT / "scripts/run-qemu-melania-native.sh").read_text()


class ManagedRoleSerialRelay(unittest.TestCase):
    def test_guest_forwards_live_kernel_events_before_original_crtc_restore(self):
        stage = GUEST.split("# Stage81B:", 1)[1].split("# Stage82D:", 1)[0]
        fifo = 'mkfifo -m 600 "$managed_kms_fifo"'
        forward = 'tee "$managed_kms_log" < "$managed_kms_fifo" > /dev/ttyS0 &'
        present = '"$continuous_kms" < /dev/tty1 > "$managed_kms_fifo" 2>&1 &'
        wait = 'if ! wait "$managed_tail_pid"; then'
        self.assertIn(fifo, stage)
        self.assertIn(forward, stage)
        self.assertIn(present, stage)
        self.assertIn(wait, stage)
        self.assertLess(stage.index(fifo), stage.index(forward))
        self.assertLess(stage.index(forward), stage.index(present))
        self.assertLess(stage.index(present), stage.index(wait))
        self.assertNotIn('tail -n +1 -f -s 0.05 "$managed_kms_log"', stage)
        self.assertIn('fatal \'managed_role_serial_forwarder_failed\'', stage)
        self.assertIn('rmdir "$managed_kms_forward_dir"', stage)
        for frame in (3, 5, 7, 9, 13, 14):
            self.assertIn(
                f'MELANIA_RUST_CONTINUOUS_MANAGED_ROLE_FRAME_VISIBLE=$index',
                stage,
            )
        self.assertIn("MELANIA_RUST_KMS_RESTORE_VERIFIED=true", stage)
        self.assertIn("MELANIA_RUST_KMS_CONNECTOR_RESTORE_VERIFIED=true", stage)

    def test_host_still_requires_six_real_qmp_pixel_captures(self):
        stage = HOST.split(
            'python3 "$root/scripts/capture-qemu-managed-xdg-roles.py"', 1
        )[1].split('managed_role_capture_pid=$!', 1)[0]
        managed_deadline = int(stage.split("--timeout ", 1)[1].split()[0])
        vm_deadline = int(HOST.split(
            'timeout --signal=TERM --kill-after=10s ', 1
        )[1].split("s", 1)[0])
        self.assertGreater(managed_deadline, 1403)
        self.assertLess(managed_deadline, vm_deadline)
        for frame in (3, 5, 7, 9, 13, 14):
            self.assertIn(
                f"MELANIA_QEMU_MANAGED_XDG_FRAME{frame}_PIXELS_VERIFIED=true",
                HOST,
            )
            self.assertIn(f'test -s "$report/managed-role-frame{frame}.ppm"', HOST)
        self.assertIn("MELANIA_QEMU_MANAGED_XDG_SIX_FRAMES_VERIFIED=true", HOST)
        self.assertIn("MELANIA_QEMU_NATIVE_GUEST_EXIT=0", HOST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false", GUEST)


if __name__ == "__main__":
    unittest.main()
