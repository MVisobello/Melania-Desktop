"""Stage89: a libseat pause invalidates in-flight native KMS presentations.

These wiring assertions complement the Rust state-machine tests. The real
QEMU guest still performs the existing DRM FB_ID, screenshot and input checks.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]


class NativeSeatRevocation(unittest.TestCase):
    def test_epoch_ties_presentation_to_live_seat(self):
        guard=(ROOT/"melania-native/src/native_presentation_guard.rs").read_text()
        bridge=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
        lib=(ROOT/"melania-native/src/lib.rs").read_text()
        self.assertIn("pub mod native_presentation_guard;",lib)
        self.assertIn("pub enum PresentationPhase { Cold, Active, Revoked }",guard)
        self.assertIn("pub fn ticket(&self,seat_active:bool)",guard)
        self.assertIn("pub fn accepts(&self,token:PresentationToken,seat_active:bool)",guard)
        self.assertIn("self.epoch=self.epoch.wrapping_add(1);",guard)
        self.assertIn("native_presentation_guard:NativePresentationGuard::default()",bridge)
        self.assertIn("state.native_presentation_guard.arm_initial(session.is_active())",bridge)
        self.assertIn("self.continuous_presentation_token=Some(presentation_token);",bridge)
        self.assertIn("self.native_presentation_guard.accepts(ticket,self.lifecycle.seat_active())",bridge)

    def test_pause_and_resume_cannot_approve_stale_ack(self):
        bridge=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
        guard=(ROOT/"melania-native/src/native_presentation_guard.rs").read_text()
        self.assertIn("SessionEvent::PauseSession=>{",bridge)
        self.assertGreaterEqual(bridge.count("state.native_presentation_guard.revoke();"),2)
        self.assertIn("state.native_presentation_guard.permanently_revoked()",bridge)
        self.assertIn("native QEMU bridge stopped: libseat paused",bridge)
        self.assertIn("native KMS ACK blocked after libseat/VT revocation",bridge)
        self.assertIn("stale native KMS ACK from revoked libseat generation",bridge)
        self.assertIn("fn handle_input(state:&mut BridgeState,event:InputEvent",bridge)
        self.assertIn("No physical input reaches a Wayland client after a VT/seat pause",bridge)
        self.assertIn("if self.phase!=PresentationPhase::Cold",guard)
        self.assertIn("pub fn native_login_authorized(&self)->bool {false}",guard)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",bridge)

    def test_real_drms_and_vm_safety_remain_required(self):
        entry=(ROOT/"melania-native/src/main.rs").read_text()
        self.assertIn("assert_dedicated_console();",entry)
        self.assertIn("assert_qemu_test_guest();",entry)
        self.assertIn("MELANIA_NATIVE_WAYLAND_EXPERIMENT",entry)
        self.assertNotIn("NATIVE_SESSION_READY=true",entry)


if __name__=="__main__":
    unittest.main()
