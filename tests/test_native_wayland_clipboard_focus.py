"""Regression contract for real Smithay wl_data_device clipboard focus.

This is not a fake copy/paste acceptance test: the Rust code must compile in
Linux CI and a separate two-client Wayland transfer will still be required
for full desktop acceptance. The contract catches loss of focus wiring.
"""
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()

class NativeClipboardFocus(unittest.TestCase):
    def test_real_keyboard_focus_controls_wayland_clipboard_recipient(self):
        callback=BRIDGE[BRIDGE.index("impl SeatHandler for BridgeState {"):
                        BRIDGE.index("impl XdgShellHandler for BridgeState {")]
        self.assertIn("fn focus_changed(&mut self,seat:&Seat<Self>,focused:Option<&WlSurface>)",callback)
        self.assertIn("focused.and_then(|surface|surface.client())",callback)
        self.assertIn("set_data_device_focus(&self.display_handle,seat,client.clone());",callback)
        self.assertIn("set_primary_focus(&self.display_handle,seat,client);",callback)
        self.assertLess(callback.index("set_data_device_focus("),
                        callback.index("set_primary_focus("))
        self.assertNotIn("fn focus_changed(&mut self,_:&Seat<Self>,_:Option<&WlSurface>){}",callback)
        self.assertIn("DataDeviceState::new::<BridgeState>(&handle)",BRIDGE)
        self.assertIn("display_handle:handle.clone()",BRIDGE)
        self.assertIn("delegate_data_device!(BridgeState);",BRIDGE)

    def test_revocation_on_real_libseat_and_gpu_events(self):
        pause=BRIDGE[BRIDGE.index("SessionEvent::PauseSession=>{"):
                     BRIDGE.index("SessionEvent::ActivateSession=>{")]
        self.assertIn("set_data_device_focus(&state.display_handle,&state.seat,None);",pause)
        # Revoke frame/seat authorization before any Wayland focus update.
        # Both clipboard and real wl_keyboard/wl_pointer focus must still
        # be cleared before the KMS transport is interrupted and libinput
        # is suspended; the old client cannot retain a usable seat.
        self.assertLess(pause.index("state.native_presentation_guard.revoke();"),
                        pause.index("revoke_wayland_focus_for_lost_seat(state);"))
        self.assertLess(pause.index("revoke_wayland_focus_for_lost_seat(state);"),
                        pause.index("set_data_device_focus("))
        self.assertLess(pause.index("set_data_device_focus("),
                        pause.index("state.continuous_worker.take();"))
        self.assertLess(pause.index("state.continuous_worker.take();"),
                        pause.index("context.suspend();"))
        udev=BRIDGE[BRIDGE.index("let reason=state.gpu_watch.observe(&change);"):
                    BRIDGE.index("let live_stream=std::env::var(")]
        self.assertIn("state.native_presentation_guard.revoke();",udev)
        self.assertIn("set_data_device_focus(&state.display_handle,&state.seat,None);",udev)
        self.assertLess(udev.index("state.native_presentation_guard.revoke();"),
                        udev.index("set_data_device_focus("))
        self.assertLess(udev.index("set_data_device_focus("),
                        udev.index("state.continuous_worker.take();"))

    def test_clipboard_is_client_scoped_not_fabricated_server_selection(self):
        handler=BRIDGE[BRIDGE.index("fn focus_changed(&mut self,seat:&Seat<Self>"):
                       BRIDGE.index("fn cursor_image(",BRIDGE.index("fn focus_changed(&mut self,seat:&Seat<Self>"))]
        self.assertNotIn("set_data_device_selection(",handler)
        self.assertNotIn("Client::",handler)
        self.assertIn("focused.and_then(",handler)

if __name__=="__main__":
    unittest.main()
