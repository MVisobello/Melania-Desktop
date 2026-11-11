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
        # Both selections must share the *same* client, and never expose
        # an old source once libseat ownership is lost or the DRM epoch revoked.
        self.assertIn("let client=if self.lifecycle.seat_active() &&",hook)
        self.assertIn("!self.native_presentation_guard.permanently_revoked()",hook)
        self.assertIn("focused.and_then(|surface|surface.client())",hook)
        self.assertIn("} else {None};",hook)
        self.assertLess(hook.index("!self.native_presentation_guard.permanently_revoked()"),
                        hook.index("focused.and_then(|surface|surface.client())"))
        self.assertLess(hook.index("} else {None};"),
                        hook.index("set_data_device_focus("))
        self.assertIn(
            "set_data_device_focus(&self.display_handle,seat,client.clone());", hook
        )
        self.assertIn("set_primary_focus(&self.display_handle,seat,client);", hook)
        self.assertLess(hook.index("set_data_device_focus("),
                        hook.index("set_primary_focus("))

    def test_both_vt_pause_and_gpu_loss_revoke_primary_source(self):
        # The release operations now occur once inside a shared real Smithay
        # revoker called by VT pause, GPU loss AND failed libinput resume.
        # Check both callsite reachability and the full selection revocation.
        revoker = SOURCE[SOURCE.index("fn revoke_wayland_input_for_lost_seat("):
                         SOURCE.index("/// Keep the original libseat-owned descriptor")]
        for statement in (
            "set_primary_focus(&state.display_handle,&state.seat,None);",
            "clear_primary_selection(&state.display_handle,&state.seat);",
            "set_data_device_focus(&state.display_handle,&state.seat,None);",
        ):
            self.assertEqual(revoker.count(statement), 1)
        pause=SOURCE[SOURCE.index("SessionEvent::PauseSession=>{"):
                     SOURCE.index("SessionEvent::ActivateSession=>{")]
        udev=SOURCE[SOURCE.index("let reason=state.gpu_watch.observe(&change);"):
                    SOURCE.index('MELANIA_NATIVE_DRM_TOPOLOGY_REVOKED=')]
        resume=SOURCE[SOURCE.index("if context.resume().is_err() {"):
                      SOURCE.index("// If libinput cannot resume,")]
        for callback in (pause,udev,resume):
            self.assertIn("state.native_presentation_guard.revoke();",callback)
            self.assertIn("revoke_wayland_input_for_lost_seat(state);",callback)
            self.assertLess(callback.index("state.native_presentation_guard.revoke();"),
                            callback.index("revoke_wayland_input_for_lost_seat(state);"))


    def test_real_wayland_keyboard_and_pointer_focus_released_on_vt_and_gpu_loss(self):
        focus=SOURCE[SOURCE.index("fn revoke_wayland_focus_for_lost_seat("):
                     SOURCE.index("fn revoke_wayland_input_for_lost_seat(")]
        self.assertIn('keyboard.set_focus(state,None,SERIAL_COUNTER.next_serial());',focus)
        self.assertIn('state.focused=None;',focus)
        self.assertIn('pointer.motion(state,None,&MotionEvent{',focus)
        self.assertIn('pointer.frame(state);',focus)
        revoker=SOURCE[SOURCE.index("fn revoke_wayland_input_for_lost_seat("):
                       SOURCE.index("/// Keep the original libseat-owned descriptor")]
        self.assertIn("revoke_wayland_focus_for_lost_seat(state);",revoker)
        self.assertIn("clear_primary_selection(&state.display_handle,&state.seat);",revoker)
        self.assertLess(revoker.index("revoke_wayland_focus_for_lost_seat(state);"),
                        revoker.index("clear_primary_selection("))
        for start,end in (("SessionEvent::PauseSession=>{","SessionEvent::ActivateSession=>{"),
                          ("let reason=state.gpu_watch.observe(&change);",
                           "MELANIA_NATIVE_DRM_TOPOLOGY_REVOKED=")):
            callback=SOURCE[SOURCE.index(start):SOURCE.index(end,SOURCE.index(start))]
            self.assertIn("revoke_wayland_input_for_lost_seat(state);",callback)


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
