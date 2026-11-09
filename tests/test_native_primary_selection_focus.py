"""Structural guard for real Smithay primary-selection wiring in Melania Native.

Rust/Cargo compilation must also pass. This test does not substitute for
an end-to-end two-client text transfer or physical keyboard/mouse validation.
"""
from pathlib import Path
import unittest

SOURCE = (Path(__file__).resolve().parents[1] /
          "melania-native/src/wayland_bridge.rs").read_text()


class NativePrimarySelectionFocus(unittest.TestCase):
    def test_global_and_smithay_delegation_are_real(self):
        self.assertIn("delegate_primary_selection,", SOURCE)
        self.assertIn("PrimarySelectionHandler,PrimarySelectionState", SOURCE)
        self.assertIn("primary_selection:PrimarySelectionState", SOURCE)
        self.assertIn("PrimarySelectionState::new::<BridgeState>(&handle)", SOURCE)
        self.assertIn("impl PrimarySelectionHandler for BridgeState", SOURCE)
        self.assertIn("fn primary_selection_state(&self)->&PrimarySelectionState", SOURCE)
        self.assertIn("delegate_primary_selection!(BridgeState);", SOURCE)

    def test_keyboard_focus_routes_both_clipboard_and_primary_to_one_client(self):
        hook = SOURCE.split("impl SeatHandler for BridgeState {", 1)[1].split(
            "fn cursor_image(", 1
        )[0]
        self.assertIn("let client=focused.and_then(|surface|surface.client());", hook)
        self.assertIn(
            "set_data_device_focus(&self.display_handle,seat,client.clone());", hook
        )
        self.assertIn("set_primary_focus(&self.display_handle,seat,client);", hook)
        self.assertLess(hook.index("set_data_device_focus("),
                        hook.index("set_primary_focus("))

    def test_both_vt_pause_and_gpu_loss_revoke_primary_source(self):
        self.assertEqual(
            SOURCE.count("set_primary_focus(&state.display_handle,&state.seat,None);"),
            2,
        )
        self.assertEqual(
            SOURCE.count("clear_primary_selection(&state.display_handle,&state.seat);"),
            2,
        )
        self.assertEqual(
            SOURCE.count("set_data_device_focus(&state.display_handle,&state.seat,None);"),
            2,
        )
        self.assertIn("SessionEvent::PauseSession=>{", SOURCE)
        self.assertIn("state.native_presentation_guard.revoke();", SOURCE)

    def test_real_wayland_keyboard_and_pointer_focus_released_on_vt_and_gpu_loss(self):
        helper = SOURCE[SOURCE.index("fn revoke_wayland_focus_for_lost_seat("):
                        SOURCE.index("/// Keep the original libseat-owned descriptor", 
                                     SOURCE.index("fn revoke_wayland_focus_for_lost_seat("))]
        self.assertIn('keyboard.set_focus(state,None,SERIAL_COUNTER.next_serial());', helper)
        self.assertIn('state.focused=None;', helper)
        self.assertIn('pointer.motion(state,None,&MotionEvent{', helper)
        self.assertIn('pointer.frame(state);', helper)
        session = SOURCE[SOURCE.index("SessionEvent::PauseSession=>{"):
                         SOURCE.index("SessionEvent::ActivateSession=>{")]
        gpu_loss = SOURCE[SOURCE.index("if let Some(reason)=reason {",
                                      SOURCE.index("events.handle().insert_source(udev,")):
                          SOURCE.index('eprintln!("MELANIA_NATIVE_DRM_TOPOLOGY_REVOKED=', 
                                       SOURCE.index("events.handle().insert_source(udev,"))]
        for block in (session, gpu_loss):
            self.assertIn("revoke_wayland_focus_for_lost_seat(state);", block)
            self.assertLess(block.index("state.native_presentation_guard.revoke();"),
                            block.index("revoke_wayland_focus_for_lost_seat(state);"))
        self.assertIn("clear_primary_selection(&state.display_handle,&state.seat);", session)

    def test_libseat_reactivation_never_thaws_revoked_drm_or_input(self):
        start = SOURCE.index("SessionEvent::ActivateSession=>{")
        end = SOURCE.index("let active=matches!(event,SessionEvent::ActivateSession);",start)
        branch = SOURCE[start:end]
        self.assertIn("if state.native_presentation_guard.permanently_revoked() {",branch)
        self.assertIn("context.suspend();",branch)
        self.assertIn("state.lifecycle.on_session(SessionEvent::PauseSession);",branch)
        self.assertIn("state.render_scheduler.set_seat_active(false);",branch)
        self.assertIn("state.continuous_redraw.set_seat_active(false);",branch)
        self.assertLess(branch.index("if state.native_presentation_guard.permanently_revoked()"),
                        branch.index("if context.resume().is_err()"))
        self.assertLess(branch.index("return;"),branch.index("if context.resume().is_err()"))



if __name__ == "__main__":
    unittest.main()
