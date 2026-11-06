"""Require real USB Super+arrows/Shift, XDG ACK and virtual CRTC pixels."""
import unittest
import runpy
from unittest.mock import patch
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
                      'MELANIA_QEMU_SUPER_SNAP_USB_XDG_KMS5_PASS=true',
                      'MELANIA_NATIVE_MANAGED_XDG_MINIMIZED=true',
                      'MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=6',
                      'MELANIA_NATIVE_SNAP_CLIENT_MINIMIZE_KEYBOARD_LEAVE=true'):
            self.assertIn(token,G)
        for token in ('role.ack_configure(serial)','shm.create_pool(','surface.commit()',
                      'state.callbacks<index','state.released',
                      'wl_keyboard::Event::Key',
                      'MELANIA_NATIVE_SNAP_CLIENT_XDG_RIGHT_THIRD_ACK=true',
                      'MELANIA_NATIVE_SNAP_CLIENT_XDG_THIRD_RESTORE_ACK=true',
                      'MELANIA_NATIVE_SNAP_CLIENT_ALL_FIVE_REAL_FRAMES_PASS=true',
                      'MELANIA_NATIVE_SNAP_CLIENT_ROLE_DESTROY_AFTER_MINIMIZE=true'):
            self.assertIn(token,C)
        for token in ('"execute":"input-send-event"','"execute":"screendump"',
                      '"meta_l"','"left"','"down"','"f8"',
                      'MELANIA_QEMU_SUPER_SNAP_HALF_CRTC_PIXELS=true',
                      'MELANIA_QEMU_SUPER_SNAP_RESTORE_CRTC_PIXELS=true',
                      'MELANIA_QEMU_SUPER_SNAP_RIGHT_THIRD_CRTC_PIXELS=true',
                      'MELANIA_QEMU_SUPER_SNAP_THIRD_RESTORE_CRTC_PIXELS=true',
                      'MELANIA_QEMU_SUPER_SNAP_NORMAL_MINIMIZED_CRTC_PIXELS=true',
                      'client destroyed before minimized CRTC capture'):
            self.assertIn(token,Q)
        for token in ('snap_inject_code','qmp-super-snap.log',
                      'MELANIA_QEMU_SUPER_SNAP_USB_XDG_KMS5_PASS=true'):
            self.assertIn(token,H)
        self.assertIn('test -x target/debug/qemu-super-snap-client',W)
        self.assertIn('NATIVE_SESSION_READY=false',G)
        for marker in ('super-snap-right-third-frame4.ppm',
                       'super-snap-third-restored-frame5.ppm',
                       'super-snap-minimized-frame6.ppm'):
            self.assertIn(marker,H)
            self.assertIn(marker,Q)
        self.assertIn('for frame in 1 2 3 4 5',G)
        self.assertIn('Some(RightThird) 427x800 at 853,0',G)
        self.assertIn('Some(RightThird) 427x800 at 853,0',Q)

    def test_real_usb_shift_chord_modifier_order(self):
        # Mock ONLY the USB emitter; DRM/KMS proof is still mandatory QEMU.
        fn=runpy.run_path(str(ROOT/"scripts/inject-qemu-super-snap.py"))["chord_shift"]
        emitted=[]
        with patch.dict(fn.__globals__,{"key":lambda _path,k,pressed:emitted.append((k,pressed))}):
            fn(Path("/no-qmp-connection"),"right")
        self.assertEqual(emitted,[
            ("meta_l",True),("shift",True),("right",True),
            ("right",False),("shift",False),("meta_l",False),
        ])
if __name__=="__main__":
    unittest.main()
