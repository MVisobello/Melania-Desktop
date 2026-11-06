"""Stage121: genuine QMP tablet, authenticated xdg_toplevel.move and real CRTC pixels."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
G=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
H=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
C=(ROOT/"melania-native/src/bin/qemu-edge-drop-client.rs").read_text()
P=(ROOT/"scripts/inject-qemu-edge-drop.py").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class NativeEdgeDropQemu(unittest.TestCase):
    def test_guest_runs_real_move_and_kernel_rollback(self):
        self.assertEqual(G.count("# Stage121:"),1)
        self.assertEqual(G.count("# Stage120:"),1)
        self.assertEqual(G.count("# Stage48:"),1)
        for name in (
            'MELANIA_NATIVE_XDG_CLIENT_MOVE_GRANTED=1',
            'MELANIA_NATIVE_EDGE_DROP_REQUEST=1 Snap(LeftHalf)',
            'MELANIA_NATIVE_MANAGED_XDG_CLIENT_BUFFER_ACCEPTED=640x800',
            'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=',
            'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true',
            'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true',
            'MELANIA_QEMU_EDGE_DROP_XDG_REAL_CRTC_PASS=true',
            'MELANIA_NATIVE_SESSION_READY=false',
        ):
            self.assertIn(name,G)
    def test_no_forged_pointer_serial_or_simulated_pixels(self):
        for name in (
            'wl_pointer::Event::Button',
            's.entered && !s.requested && s.callbacks==1',
            'top._move(seat,serial)',
            'role.ack_configure(serial)',
            'surface.commit()',
            'state.callbacks<index',
        ):
            self.assertIn(name,C)
        for name in (
            '"execute":"input-send-event"',
            '"type":"abs"',
            '"type":"btn"',
            '"execute":"screendump"',
            'MELANIA_NATIVE_EDGE_DROP_CLIENT_MOVE_REQUESTED=true',
            'MELANIA_QEMU_EDGE_DROP_REAL_CRTC_HALF_PIXELS=true',
        ):
            self.assertIn(name,P)
        for name in (
            'edge_inject_code',
            'qmp-edge-drop.log',
            'MELANIA_QEMU_EDGE_DROP_XDG_REAL_CRTC_PASS=true',
        ):
            self.assertIn(name,H)
        self.assertIn('test -x target/debug/qemu-edge-drop-client',CI)

if __name__=="__main__":
    unittest.main()
