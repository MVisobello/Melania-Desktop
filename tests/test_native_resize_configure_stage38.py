#!/usr/bin/env python3
"""Stage38 xdg-shell resize configure/ACK, QEMU-only safety contract."""
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-client-window.rs").read_text()
DRAG=(ROOT/"scripts/inject-qemu-resize-configure.py").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class ResizeHandshakeStage38(unittest.TestCase):
    def test_wayland_client_receives_real_configure_and_acks_serial(self):
        self.assertIn("xdg_toplevel::Event::Configure{width,height,..}",CLIENT)
        self.assertIn("surface.ack_configure(serial);",CLIENT)
        self.assertIn("state.resize_requested.take()",CLIENT)
        self.assertIn("MELANIA_NATIVE_CLIENT_RESIZE_CONFIGURE_RECEIVED=",CLIENT)
        self.assertIn("MELANIA_NATIVE_CLIENT_RESIZE_CONFIGURE_ACK=",CLIENT)

    def test_compositor_sends_size_and_resizing_state_only_under_vm_gate(self):
        for marker in (
            "MELANIA_NATIVE_RESIZE_CONFIGURE",
            "resize_configure_mode:bool",
            "last_resize_request:Option<(u64,i32,i32)>",
            "state.size=Some((rect.width,rect.height).into());",
            "state.states.set(xdg_toplevel::State::Resizing)",
            "top.send_pending_configure();",
            "pending.states.unset(xdg_toplevel::State::Resizing)",
            "MELANIA_NATIVE_RESIZE_CONFIGURE_SENT=",
            "MELANIA_NATIVE_RESIZE_CONFIGURE_END=",
        ):
            self.assertIn(marker,BRIDGE)
        self.assertIn("MELANIA_NATIVE_LAYOUT_REPAINT=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_NATIVE_RESIZE_CONFIGURE=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)

    def test_private_qmp_real_usb_input_is_verified_in_ci(self):
        self.assertIn("MELANIA_RUST_MULTI_FRAME4_ACTIVE=true",DRAG)
        self.assertIn('"button":"left","down":down',DRAG)
        self.assertIn('("PRESS",btn(True))',DRAG)
        self.assertIn('("RELEASE",btn(False))',DRAG)
        self.assertIn("MELANIA_NATIVE_CLIENT_RESIZE_CONFIGURE_ACK=",DRAG)
        self.assertIn("inject-qemu-resize-configure.py",HOST)
        self.assertIn("MELANIA_QEMU_REAL_XDG_RESIZE_ACK_PASS=true",HOST)
        self.assertIn("test_native_resize_configure_stage38.py",CI)
        self.assertIn("cargo test -p melania-native --all-targets",CI)
        for path in ("scripts/melania-qemu-native-guest.sh",
                     "scripts/run-qemu-melania-native.sh"):
            p=subprocess.run(["bash","-n",str(ROOT/path)],
                text=True,capture_output=True,timeout=6)
            self.assertEqual(p.returncode,0,p.stderr)

if __name__=="__main__":
    unittest.main()
