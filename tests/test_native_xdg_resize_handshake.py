"""Stage54: xdg-shell resize is server-configured and client ACKed.

The source-contract checks complement actual QEMU KMS and QMP screenshots.
No test here alone claims a production compositor is available.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-continuous-two-windows.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()

class NativeXdgResizeHandshake(unittest.TestCase):
    def test_server_configure_is_after_actual_sixth_kms_ack(self):
        start=BRIDGE.index("fn poll_continuous_kms(&mut self)")
        end=BRIDGE.index("fn begin_generation_proof(",start)
        code=BRIDGE[start:end]
        verified=code.index("if !self.continuous_redraw.presented(planned)")
        serial=code.index("self.continuous_frame_count=self.continuous_frame_count.saturating_add(1);")
        configure=code.index("if self.dynamic_xdg_resize_mode && self.continuous_frame_count==6")
        self.assertLess(verified,serial)
        self.assertLess(serial,configure)
        self.assertIn("top.with_pending_state(",code)
        self.assertIn("state.size=Some((width,height).into())",code)
        self.assertIn("state.states.set(xdg_toplevel::State::Resizing)",code)
        self.assertIn("top.send_pending_configure()",code)
        self.assertIn("MELANIA_NATIVE_DYNAMIC_XDG_RESIZE_CONFIGURES_SENT=true",code)

    def test_client_acks_real_xdg_surface_serial_before_new_buffer(self):
        for marker in (
            "impl Dispatch<xdg_toplevel::XdgToplevel,u8>",
            "impl Dispatch<xdg_surface::XdgSurface,u8>",
            "surface.ack_configure(serial)",
            "state.resize_acked_a=true",
            "state.resize_acked_b=true",
            "if dynamic && state.dynamic_xdg_resize && (index==7 || index==8)",
            "if done {break;}",
            'state.resize_requested_a.ok_or("unconfigured xdg window A")?',
            'state.resize_requested_b.ok_or("unconfigured xdg window B")?',
            "MELANIA_NATIVE_CLIENT_XDG_RESIZE_ACK_{}=true",
            "let pool=shm.create_pool(file.as_fd(),pixels.len() as i32,&qh,());",
            "surface.damage_buffer(0,0,client_w as i32,client_h as i32)"
        ):
            self.assertIn(marker,CLIENT)
        self.assertLess(CLIENT.index("if done {break;}"),
                        CLIENT.index("let pool=shm.create_pool(file.as_fd()"))
        self.assertIn('get_xdg_surface(&first,&qh,1u8)',CLIENT)
        self.assertIn('get_xdg_surface(&second,&qh,2u8)',CLIENT)
        self.assertIn('let expected=if index==7 {(320,192)} else {(220,180)};',CLIENT)
        self.assertIn('if size!=Some(expected)',CLIENT)
        self.assertIn('return Err("xdg resize configure/ACK roundtrip timed out".into());',CLIENT)
        # The real xdg_surface ACK handler must execute before the next
        # wl_shm buffer attachment; other modes still use their prior path.
        ack=CLIENT.index("surface.ack_configure(serial)")
        attach=CLIENT.index("surface.attach(Some(&buffer),0,0);")
        self.assertLess(ack,attach)

    def test_experiment_remains_explicit_and_guest_requires_both_acks(self):
        self.assertIn('MELANIA_NATIVE_XDG_RESIZE',BRIDGE)
        self.assertIn("dynamic_xdg_resize_mode:bool",BRIDGE)
        self.assertIn("dynamic_toplevels:Vec<ToplevelSurface>",BRIDGE)
        self.assertIn("xdg resize configure is only permitted in disposable QEMU dynamic mode",BRIDGE)
        self.assertIn("self.dynamic_toplevels.retain(",BRIDGE)
        for marker in (
            "MELANIA_NATIVE_XDG_RESIZE=YES_QEMU_VM",
            "MELANIA_QEMU_XDG_RESIZE_CLIENT=YES_QEMU_VM",
            "MELANIA_NATIVE_DYNAMIC_XDG_RESIZE_CONFIGURES_SENT=true",
            "MELANIA_NATIVE_CLIENT_XDG_RESIZE_ACK_A=true",
            "MELANIA_NATIVE_CLIENT_XDG_RESIZE_ACK_B=true",
            "MELANIA_NATIVE_DYNAMIC_TWO_SIZES_12_KMS_FRAMES_PASS=true",
            "MELANIA_RUST_KMS_RESTORE_VERIFIED=true"
        ):
            self.assertIn(marker,GUEST)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_DYNAMIC_PIXELS_VERIFIED=true",HOST)
        self.assertNotIn("NATIVE_SESSION_READY=true",BRIDGE)

if __name__=="__main__":
    unittest.main()
