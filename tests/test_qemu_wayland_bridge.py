#!/usr/bin/env python3
"""Native QEMU seat bridge: no login, real virtual USB -> libinput -> wl_*.

The Rust/C compilers and VM prove behavior; these structural tests prevent
accidental host DRM access, false readiness, and missing evidence checks.
"""
import ast
import json
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
RUST=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
C=(ROOT/"scripts/melania-qemu-wayland-client.c").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
QMP=(ROOT/"scripts/inject-qemu-native-input.py").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
NATIVE_CARGO=(ROOT/"melania-native/Cargo.toml").read_text()

class NativeWaylandBridge(unittest.TestCase):
    def test_qemu_and_vt_consent_required_before_device_acquisition(self):
        block=MAIN[MAIN.index('[flag] if flag=="--experimental-wayland-seat"'): ]
        for needed in ('assert_dedicated_console();','assert_qemu_test_guest();',
                       'MELANIA_NATIVE_WAYLAND_EXPERIMENT',
                       'YES_QEMU_VM','wayland_bridge::run()'):
            self.assertIn(needed,block)
        self.assertLess(block.index('assert_dedicated_console();'),
                        block.index('wayland_bridge::run()'))
        self.assertLess(block.index('assert_qemu_test_guest();'),
                        block.index('wayland_bridge::run()'))
        self.assertIn('"wayland_frontend"',NATIVE_CARGO)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        self.assertIn("MELANIA_QEMU_NATIVE_WAYLAND_RENDER_READY=false",GUEST)

    def test_smithay_compositor_and_seat_forward_only_registered_libinput(self):
        for needle in ('CompositorState::new::<BridgeState>',
                       'seats.new_wl_seat(&handle',
                       'seat.add_keyboard(Default::default()',
                       'seat.add_pointer()',
                       'delegate_compositor!(BridgeState)',
                       'delegate_seat!(BridgeState)',
                       'LibinputInputBackend::new',
                       'input_registry_mut().event(',
                       'InputEvent::Keyboard {event}',
                       'InputEvent::PointerButton {event}',
                       'keyboard.input::<(),_>',
                       'pointer.button(state,',
                       'state.native_presentation_guard.revoke();',
                       'context.suspend();',
                       'MELANIA_NATIVE_WAYLAND_INPUT_FORWARDED=true'):
            self.assertIn(needle,RUST)
        # The native seat token must be invalidated *before* input is suspended:
        # an old DRM worker reply may never release Wayland callbacks after VT loss.
        pause=RUST.split('SessionEvent::PauseSession=>{',1)[1].split('SessionEvent::ActivateSession=>{',1)[0]
        self.assertLess(pause.index('state.native_presentation_guard.revoke();'),
                        pause.index('context.suspend();'))

    def test_real_wayland_client_requires_key_and_pointer_press(self):
        for needle in ('wl_display_connect(NULL)',
                       'wl_registry_bind(', 'wl_compositor_create_surface(',
                       'wl_surface_commit(s.surface)',
                       'wl_keyboard_add_listener(',
                       'wl_pointer_add_listener(',
                       'WL_KEYBOARD_KEY_STATE_PRESSED',
                       'WL_POINTER_BUTTON_STATE_PRESSED',
                       'MELANIA_NATIVE_WAYLAND_CLIENT_READY=true',
                       'MELANIA_NATIVE_WAYLAND_CLIENT_INPUT_PASS=%s',
                       'int ok=s.keys>0 && s.buttons>0;'):
            self.assertIn(needle,C)

    def test_qmp_injects_only_into_private_guest_socket(self):
        tree=ast.parse(QMP)
        self.assertTrue(any(isinstance(n,ast.FunctionDef) and n.name=="send_qmp" for n in tree.body))
        self.assertIn('socket.AF_UNIX',QMP)
        self.assertIn('"execute":"input-send-event"',QMP)
        self.assertIn('"type":"qcode","data":"a"',QMP)
        self.assertIn('"down":True,"button":"left"',QMP)
        for forbidden in ('/dev/input','/dev/dri/card0','xdotool','uinput'):
            # /dev/input appears in a comment explaining it is not opened.
            if forbidden != '/dev/input': self.assertNotIn(forbidden,QMP)
        self.assertIn('MELANIA_NATIVE_WAYLAND_CLIENT_READY=true',QMP)
        self.assertIn("MELANIA_QEMU_NATIVE_USB_INPUT_INJECTED=true",QMP)

    def test_guest_and_host_require_real_evidence_and_prior_tests(self):
        self.assertIn('MELANIA_QEMU_NATIVE_WAYLAND_INPUT_PASS=true',GUEST)
        self.assertIn('MELANIA_NATIVE_WAYLAND_INPUT_FORWARDED=true',GUEST)
        self.assertIn("MELANIA_NATIVE_WAYLAND_CLIENT_INPUT_PASS=true",HOST)
        self.assertIn("MELANIA_QEMU_NATIVE_USB_INPUT_INJECTED=true",HOST)
        self.assertIn("MELANIA_QEMU_NATIVE_WAYLAND_RENDER_READY=false",HOST)
        self.assertIn("MELANIA_QEMU_VISIBLE_RESTORE_VERIFIED=true",HOST)
        self.assertIn("MELANIA_QEMU_NATIVE_POST_KMS_VT_PASS=true",HOST)
        self.assertIn("-vga virtio",HOST)
        self.assertIn("accel=tcg",HOST)
        self.assertNotIn("vfio",HOST)
        self.assertIn("MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false",HOST)
        self.assertIn("scripts/melania-qemu-wayland-client.c",CI)
        self.assertIn("-lwayland-client",CI)

    def test_host_stays_noninvasive_and_shell_scripts_valid(self):
        r=subprocess.run(["bash","scripts/run-qemu-melania-native.sh"],
                         cwd=ROOT,env={"PATH":"/usr/bin:/bin"},
                         capture_output=True,text=True,timeout=8)
        self.assertEqual(r.returncode,78,r.stderr)
        for path in ("scripts/run-qemu-melania-native.sh",
                     "scripts/melania-qemu-native-guest.sh"):
            check=subprocess.run(["bash","-n",str(ROOT/path)],
                                 capture_output=True,text=True,timeout=5)
            self.assertEqual(check.returncode,0,check.stderr)

if __name__=="__main__":
    unittest.main()
