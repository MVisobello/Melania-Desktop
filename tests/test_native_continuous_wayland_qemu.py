"""Stage46: continuous scene updates must be wired to real DRM/KMS ACKs."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
DRM=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-continuous-client.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()

class TwelveRealFramesContract(unittest.TestCase):
    def test_opt_in_separate_worker_and_main_loop(self):
        for token in (
            'MELANIA_NATIVE_CONTINUOUS_KMS', 'continuous_kms_mode:bool',
            'continuous_worker:Option<ContinuousKmsWorker>',
            'self.continuous_redraw.next_frame()',
            'worker.submit(ContinuousKmsJob{planned,pixels})',
            'self.continuous_redraw.presented(planned)',
            'fn poll_continuous_kms(&mut self)',
            'state.poll_continuous_kms()?',
            'self.note_scene_change();',
            'continuous-frame-stream.sock',
        ):
            self.assertIn(token,BRIDGE)
        self.assertIn('if self.continuous_kms_mode',BRIDGE)
        self.assertIn('if state.continuous_kms_mode',BRIDGE)
        self.assertIn('self.continuous_inflight.is_some()',BRIDGE)

    def test_callback_follows_real_ack_and_no_fixed_six_frame_cutoff(self):
        start=BRIDGE.index('fn poll_continuous_kms(&mut self)')
        end=BRIDGE.index('fn begin_generation_proof',start)
        block=BRIDGE[start:end]
        ack=block.index('self.continuous_redraw.presented(planned)')
        callback=block.index('complete_presented_frame(')
        # Stage50 holds each submitted client receipt, then releases its
        # buffer only after the verified KMS ACK.
        release=block.index('completed.resource.release();')
        self.assertLess(ack,callback)
        self.assertLess(callback,release)
        self.assertNotIn('pixel_frame_count<3',block)
        self.assertNotIn('ordinal:6',block)
        self.assertIn('self.submit_continuous_frame()?',block)

    def test_disposable_guest_requires_all_twelve_real_callbacks(self):
        self.assertIn('for index in 1..=total',CLIENT)
        self.assertIn('MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES',CLIENT)
        self.assertIn('frame_budget(',CLIENT)
        self.assertIn('as_deref(),12',CLIENT)
        self.assertIn('surface.frame(&qh,())',CLIENT)
        self.assertIn('state.buffer_released',CLIENT)
        self.assertIn('state.frame_done_count<index',CLIENT)
        self.assertIn('MELANIA_NATIVE_CONTINUOUS_CLIENT_12_FRAMES_PASS=true',CLIENT)
        self.assertIn('MELANIA_QEMU_CONTINUOUS_KMS=YES_DISPOSABLE_QEMU',GUEST)
        self.assertIn('MELANIA_QEMU_CONTINUOUS_FRAMES=12',GUEST)
        self.assertIn('MELANIA_RUST_CONTINUOUS_FRAME_ACKED=',GUEST)
        self.assertIn('MELANIA_RUST_KMS_RESTORE_VERIFIED=true',GUEST)
        self.assertIn('MELANIA_QEMU_NATIVE_CONTINUOUS_12_REAL_FRAMES_PASS=true',GUEST)
        self.assertIn('MELANIA_QEMU_NATIVE_CONTINUOUS_12_REAL_FRAMES_PASS=true',HOST)
        self.assertIn('wait_for_fb(&card,crtc_handle,framebuffer)?',DRM)

if __name__=="__main__":
    unittest.main()
