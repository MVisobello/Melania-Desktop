"""Stage49 guards for real QEMU continuous close/reopen and retained B."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-continuous-two-windows.rs").read_text()
KMS=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
QMP=(ROOT/"scripts/capture-qemu-continuous-reopen.py").read_text()

class ReopenAfterTwelveKmsFrames(unittest.TestCase):
    def test_reopen_is_explicitly_vm_gated_and_legacy_test_unchanged(self):
        for word in ("MELANIA_NATIVE_CONTINUOUS_REOPEN",
                     "continuous_reopen_mode:bool", "continuous_frame_target:u64",
                     '"YES_QEMU_VM") {14} else if',
                     '"YES_QEMU_VM") {13} else {',
                     'native_frame_budget::frame_budget(',
                     'MELANIA_NATIVE_QEMU_FRAME_TARGET',
                     "continuous_close_frame_presented",
                     "continuous_reopen_frame_presented"):
            self.assertIn(word,BRIDGE)
        self.assertIn("continuous reopen requires two real windows in disposable QEMU",BRIDGE)
        self.assertIn("MELANIA_NATIVE_CONTINUOUS_REOPEN=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_REOPEN=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_REOPEN_CLIENT=YES_QEMU_VM",GUEST)

    def test_close_requires_real_kms_ack_before_surviving_client_callback(self):
        start=BRIDGE.index("fn poll_continuous_kms")
        end=BRIDGE.index("fn begin_generation_proof",start)
        region=BRIDGE[start:end]
        self.assertLess(region.index("self.continuous_redraw.presented(planned)"),
                        region.index("MELANIA_NATIVE_CONTINUOUS_CLOSE_FRAME13_CALLBACK=true"))
        self.assertIn("complete_presented_frame(&survivor,13*16)",region)
        self.assertIn("self.continuous_frame_count==13",region)
        self.assertIn("self.continuous_reopen_frame_presented=true",region)
        self.assertIn("self.submit_continuous_frame()",BRIDGE)
        self.assertIn("self.continuous_frame_count<self.continuous_frame_target",BRIDGE)

    def test_real_new_xdg_surface_only_after_close_frame13(self):
        self.assertIn("let _survivor_callback=second.frame(&qh,());",CLIENT)
        self.assertIn("first_top.destroy();",CLIENT)
        self.assertIn("while state.frame_done_count<13",CLIENT)
        # Both scenarios run in ONE guest; create_new fixtures must not collide.
        self.assertIn("continuous-reopen-seed-{index}.bgra",CLIENT)
        self.assertIn("continuous-two-client-{index}.bgra",CLIENT)
        self.assertIn("let reopened=compositor.create_surface(&qh,());",CLIENT)
        self.assertIn("while state.frame_done_count<14 || !state.buffer_released",CLIENT)
        self.assertIn("MELANIA_NATIVE_CONTINUOUS_REOPEN_14_FRAMES_PASS=true",CLIENT)
        self.assertIn("MELANIA_NATIVE_CONTINUOUS_REOPEN_FRAME14_ACKED=true",CLIENT)

    def test_real_qmp_observer_survives_slow_tcg_without_weaker_pixel_gate(self):
        # Full guest artifact 13948798287 had real DRM frame13/14 ACKs
        # while the historical 1350s host observer had already timed out.
        # The QEMU watchdog remains bounded; extend only the observer.
        import re
        guest_timeout=re.search(r'timeout --signal=TERM --kill-after=10s (\d+)s',HOST)
        self.assertIsNotNone(guest_timeout)
        self.assertEqual(int(guest_timeout.group(1)),2050)
        observer=re.search(
            r'capture-qemu-continuous-reopen\.py.*?--reopen [^\n]+ --timeout (\d+)',
            HOST,re.S)
        self.assertIsNotNone(observer)
        self.assertGreaterEqual(int(observer.group(1)),1900)
        self.assertLess(int(observer.group(1)),int(guest_timeout.group(1)))
        self.assertIn('MELANIA_QEMU_CONTINUOUS_FRAME{index}_PIXELS_VERIFIED=true',QMP)
        self.assertIn('screendump(a.qmp,path)',QMP)
        self.assertIn('observed=verify(path.read_bytes(),index)',QMP)
        self.assertIn('continuous_reopen_capture_code',HOST)

    def test_qmp_must_show_disappeared_then_remapped_A_and_survivor_B(self):
        self.assertIn("MELANIA_RUST_CONTINUOUS_REOPEN_FRAME_VISIBLE=",KMS)
        self.assertIn("A_region",QMP)
        self.assertIn("B_retained",QMP)
        self.assertIn("frame==13",QMP)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_FRAME{index}_PIXELS_VERIFIED=true",QMP)
        self.assertIn("MELANIA_QEMU_NATIVE_CONTINUOUS_REOPEN_14_FRAMES_PASS=true",GUEST)
        self.assertIn("MELANIA_RUST_KMS_RESTORE_VERIFIED=true",GUEST)
        self.assertIn("continuous-close-frame13.ppm",HOST)
        self.assertIn("continuous-reopen-frame14.ppm",HOST)
        self.assertIn("continuous_reopen_capture_code",HOST)

if __name__=="__main__":
    unittest.main()
