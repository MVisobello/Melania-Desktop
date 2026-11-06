"""Stage 45: continuous DRM/KMS receiver is QEMU-only and independent."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
OLD = (ROOT / "melania-native/src/bin/qemu-rust-kms-stream.rs").read_text()
NEW = (ROOT / "melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
WIRE = (ROOT / "melania-native/src/continuous_kms_stream.rs").read_text()

class NativeContinuousReceiverSafety(unittest.TestCase):
    def test_legacy_six_frame_receiver_is_preserved(self):
        self.assertIn('MELANIA_QEMU_RUST_LIVE_STREAM_KMS', OLD)
        self.assertIn('MELANIA_QEMU_REOPEN_FRAME', OLD)
        self.assertIn('MELANIA_RUST_REOPEN_FRAME6_PASS=true', OLD)

    def test_explicit_qemu_guard_and_vt(self):
        self.assertIn('MELANIA_QEMU_CONTINUOUS_KMS', NEW)
        self.assertIn('YES_DISPOSABLE_QEMU', NEW)
        self.assertIn('/dev/tty1', NEW)
        self.assertIn('fs::read_to_string("/sys/class/dmi/id/sys_vendor")', NEW)
        self.assertIn('std::env::var_os("DISPLAY").is_some()', NEW)
        self.assertIn('std::env::var_os("WAYLAND_DISPLAY").is_some()', NEW)
        self.assertIn('/run/melania-native-qemu/continuous-frame-stream.sock', NEW)

    def test_versioned_ack_only_after_real_crtc_and_restore(self):
        for expected in ('b"MLF1"', 'b"MLA1"', 'read_exact(&mut header)',
                         'wait_for_fb(&card,crtc_handle,framebuffer)?',
                         'stream.write_all(&ack)?',
                         'native_frame_order::FrameIdentity::new(',
                         '!identity.accepted_after(previous_identity,persistent)',
                         'rollback.restore()?', 'MELANIA_RUST_CONTINUOUS_FRAMES_PASS={total}'):
            self.assertIn(expected, NEW)
        self.assertLess(NEW.index('wait_for_fb(&card,crtc_handle,framebuffer)?'),
                        NEW.index('stream.write_all(&ack)?'))
        self.assertLess(NEW.index('let restored=rollback.restore();'),
                        NEW.index('MELANIA_RUST_CONTINUOUS_FRAMES_PASS={total}'))
        # The real CPU receiver uses the SAME order check as GBM, including
        # the strict finite-QEMU requirements checked in the helper itself.
        helper=(ROOT / 'melania-native/src/native_frame_order.rs').read_text()
        self.assertIn('self.epoch==old.epoch',helper)
        self.assertIn('self.sequence>old.sequence',helper)
        self.assertIn('self.generation>old.generation',helper)
        self.assertIn('(persistent && self.generation==old.generation)',helper)
        self.assertIn('if self.sequence==0 {return false;}',helper)
        # Stage92 checks the shared parser instead of the removed inline fallback.
        self.assertIn('native_frame_budget::receiver_frame_budget(', NEW)
        self.assertNotIn('.unwrap_or(12);', NEW)
        parser=(ROOT / 'melania-native/src/native_frame_budget.rs').read_text()
        self.assertIn('pub fn receiver_frame_budget(', parser)
        self.assertIn('(8..=MAX_QEMU_FRAMES).contains(&count)', parser)
        self.assertIn('IMAGE_BYTES', WIRE)

if __name__ == "__main__":
    unittest.main()
