"""Require real USB Super+Left/Down XDG ACK and virtual CRTC pixel acceptance."""
import unittest
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
G=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
H=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
C=(ROOT/"melania-native/src/bin/qemu-super-snap-client.rs").read_text()
Q=(ROOT/"scripts/inject-qemu-super-snap.py").read_text()
W=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
class SuperSnapRealKms(unittest.TestCase):
    def test_real_wayland_usb_buffer_and_kernel_output_are_required(self):
        self.assertEqual(G.count("# Stage120:"),1)
        for token in ('MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM',
                      'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=',
                      'MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true',
                      'MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true',
                      'MELANIA_QEMU_SUPER_SNAP_USB_XDG_KMS3_PASS=true'):
            self.assertIn(token,G)
        for token in ('role.ack_configure(serial)','shm.create_pool(','surface.commit()',
                      'state.callbacks<index','state.released',
                      'wl_keyboard::Event::Key'):
            self.assertIn(token,C)
        for token in ('"execute":"input-send-event"','"execute":"screendump"',
                      '"meta_l"','"left"','"down"','"f8"',
                      'MELANIA_QEMU_SUPER_SNAP_HALF_CRTC_PIXELS=true',
                      'MELANIA_QEMU_SUPER_SNAP_RESTORE_CRTC_PIXELS=true'):
            self.assertIn(token,Q)
        for token in ('snap_inject_code','qmp-super-snap.log',
                      'MELANIA_QEMU_SUPER_SNAP_USB_XDG_KMS3_PASS=true'):
            self.assertIn(token,H)
        self.assertIn('test -x target/debug/qemu-super-snap-client',W)
        self.assertIn('NATIVE_SESSION_READY=false',G)
if __name__=="__main__":
    unittest.main()
