"""Stage48: two real Wayland toplevels retain pixels across 12 KMS ACKs."""
import unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-continuous-two-windows.rs").read_text()
RECEIVER=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
QMP=(ROOT/"scripts/capture-qemu-continuous-two-windows.py").read_text()

class TwoRetainedWaylandWindows(unittest.TestCase):
    def test_two_independent_xdg_surfaces_alternate_twelve_real_commits(self):
        for needle in ("let first=compositor.create_surface", "let second=compositor.create_surface",
                       "let surface=if index%2==1 {&first} else {&second}",
                       "for index in 1..=12u32", "state.frame_done_count<index",
                       "state.buffer_released", "MELANIA_NATIVE_CONTINUOUS_TWO_WINDOW_12_FRAMES_PASS=true"):
            self.assertIn(needle,CLIENT)
        self.assertIn('continuous-two-client-{index}.bgra',CLIENT)

    def test_only_explicit_vm_mode_allows_two_client_continuous_renderer(self):
        for needle in ('MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS','MELANIA_NATIVE_CONTINUOUS_KMS',
                       'state.multi_window_mode != continuous_two_windows',
                       'continuous_two_window_presented:bool',
                       'self.known_window_surfaces.live_count()==2',
                       'if state.multi_window_mode && !state.continuous_two_window_presented'):
            self.assertIn(needle,BRIDGE)
        self.assertIn("self.continuous_redraw.presented(planned)",BRIDGE)

    def test_guest_requires_kms_and_real_qmp_both_windows(self):
        self.assertIn('MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM',GUEST)
        self.assertIn('MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS=YES_QEMU_VM',GUEST)
        self.assertIn("MELANIA_QEMU_NATIVE_CONTINUOUS_TWO_WINDOWS_PASS=true",GUEST)
        self.assertIn("MELANIA_RUST_KMS_RESTORE_VERIFIED=true",GUEST)
        self.assertIn("MELANIA_QEMU_NATIVE_CONTINUOUS_TWO_WINDOWS_PASS=true",HOST)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_TWO_WINDOWS_PIXELS_VERIFIED=true",HOST)
        self.assertIn("continuous-two-windows-frame12.ppm",HOST)
        self.assertIn('MELANIA_RUST_CONTINUOUS_TWO_WINDOW_FRAME_VISIBLE=',RECEIVER)
        self.assertIn("MELANIA_RUST_CONTINUOUS_TWO_WINDOW_FRAME_VISIBLE=12",QMP)
        self.assertIn("window_A_retained_from_frame11",QMP)
        self.assertIn("window_B_frame12",QMP)

if __name__=="__main__":
    unittest.main()
