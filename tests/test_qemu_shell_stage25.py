#!/usr/bin/env python3
"""Stage 25: QEMU-only xdg-toplevel/layer-shell configure acknowledgement gate.

These tests never modify host GPU, VT, display manager or input devices.
The full protocol handshake is verified in the QEMU guest, not mocked here.
"""
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
RUST=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-shell-client.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class NativeShellProtocolStage25(unittest.TestCase):
    def test_server_exposes_real_shell_roles_and_sends_configures(self):
        for marker in (
            "XdgShellState::new::<BridgeState>",
            "WlrLayerShellState::new::<BridgeState>",
            "delegate_xdg_shell!(BridgeState)",
            "delegate_layer_shell!(BridgeState)",
            "impl XdgShellHandler for BridgeState",
            "impl WlrLayerShellHandler for BridgeState",
            "top.send_configure()",
            "surface.send_configure()",
            "MELANIA_NATIVE_SHELL_PROTOCOLS_READY=true",
            "MELANIA_NATIVE_SHELL_RENDERED=false",
        ):
            self.assertIn(marker,RUST)

    def test_client_uses_real_role_objects_and_acknowledges_server_configure(self):
        for marker in (
            "Connection::connect_to_env()",
            "get_xdg_surface(&window_surface",
            "get_toplevel(&qh",
            "get_layer_surface(",
            "xdg_surface::Event::Configure",
            "zwlr_layer_surface_v1::Event::Configure",
            "surface.ack_configure(serial)",
            "MELANIA_NATIVE_XDG_CONFIGURE_ACK=true",
            "MELANIA_NATIVE_LAYER_CONFIGURE_ACK=true",
            "MELANIA_NATIVE_SHELL_PROTOCOL_CLIENT_PASS=true",
        ):
            self.assertIn(marker,CLIENT)
        self.assertIn("if !state.xdg_acked || !state.layer_acked",CLIENT)

    def test_guest_uses_disposable_vm_and_checks_both_protocols(self):
        self.assertIn("assert_dedicated_console()",MAIN)
        self.assertIn("assert_qemu_test_guest()",MAIN)
        for marker in (
            "MELANIA_NATIVE_WAYLAND_EXPERIMENT=YES_QEMU_VM",
            "MELANIA_NATIVE_SHELL_PROBE=YES_QEMU_VM",
            '"$binary" --experimental-wayland-seat < /dev/tty1',
            "MELANIA_QEMU_NATIVE_SHELL_PROBE_PASS=true",
        ):
            self.assertIn(marker,GUEST)
        for marker in (
            "MELANIA_NATIVE_XDG_CONFIGURE_ACK=true",
            "MELANIA_NATIVE_LAYER_CONFIGURE_ACK=true",
            "MELANIA_NATIVE_SHELL_PROTOCOL_CLIENT_PASS=true",
            "MELANIA_NATIVE_SHELL_PROTOCOLS_READY=true",
            "MELANIA_NATIVE_SHELL_RENDERED=false",
            "MELANIA_QEMU_NATIVE_SHELL_PROBE_PASS=true",
            "MELANIA_QEMU_VISIBLE_RESTORE_VERIFIED=true",
            "MELANIA_QEMU_NATIVE_POST_KMS_VT_PASS=true",
            "MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false",
        ):
            self.assertIn(marker,HOST)
        self.assertIn("accel=tcg",HOST)
        self.assertIn("-vga virtio",HOST)
        self.assertNotIn("accel=kvm",HOST)
        self.assertNotIn("vfio",HOST)

    def test_no_login_or_untested_kms_display_advertised(self):
        self.assertIn("MELANIA_QEMU_NATIVE_WAYLAND_RENDER_READY=false",GUEST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        self.assertIn("MELANIA_NATIVE_VISIBLE_SCANOUT=false",GUEST)
        self.assertIn("MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false",GUEST)
        self.assertIn("cargo build -p melania-native --bins",CI)
        self.assertIn("test -x target/debug/qemu-shell-client",CI)

    def test_shell_syntax_and_host_opt_in(self):
        for path in ("scripts/run-qemu-melania-native.sh",
                     "scripts/melania-qemu-native-guest.sh"):
            run=subprocess.run(["bash","-n",str(ROOT/path)],
                capture_output=True,text=True,timeout=6)
            self.assertEqual(run.returncode,0,run.stderr)
        run=subprocess.run(["bash","scripts/run-qemu-melania-native.sh"],
            cwd=ROOT,env={"PATH":"/usr/bin:/bin"},
            capture_output=True,text=True,timeout=6)
        self.assertEqual(run.returncode,78,run.stderr)
        self.assertIn("MELANIA_QEMU_NATIVE_HOST_BLOCKED",run.stderr)

if __name__=="__main__":
    unittest.main()
