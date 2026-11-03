"""Stage43: real native QEMU Wayland scene mutations share a generation ledger.

The first six presentations still use bounded, real DRM/KMS transport.
ContinuousRedraw now independently audits every actual scene mutation and
validates its ticket only after the kernel FB_ID ACK, before client callbacks.
This is NOT native-login readiness or unlimited GPU presentation.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
PLANNER=(ROOT/"melania-native/src/continuous_redraw.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()

class NativeSceneGenerationAudit(unittest.TestCase):
    def test_real_wayland_bridge_tracks_generations_in_qemu_mode(self):
        self.assertIn("pub mod continuous_redraw;",LIB)
        self.assertIn("continuous_redraw:ContinuousRedraw,",BRIDGE)
        self.assertIn("continuous_pending:Option<PlannedSceneFrame>",BRIDGE)
        self.assertIn("fn note_scene_change(&mut self)",BRIDGE)
        # Stage53 chooses the correct generation ledger for the active VM
        # scene; the verified fixed QEMU path is still available.
        self.assertIn("self.continuous_redraw.scene_changed(generation)",BRIDGE)
        self.assertIn("self.multi_scene.layout_generation()",BRIDGE)
        self.assertIn("self.dynamic_scene.generation()",BRIDGE)
        self.assertIn("self.note_scene_change();",BRIDGE)
        self.assertIn("fn begin_generation_proof(&mut self,expected:u64)",BRIDGE)
        self.assertIn("fn confirm_generation_proof(&mut self,actual:u64)",BRIDGE)
        self.assertIn("self.continuous_redraw.next_frame()",BRIDGE)
        self.assertIn("self.continuous_redraw.presented(candidate)",BRIDGE)
        self.assertIn("MELANIA_NATIVE_SCENE_GENERATION_ACK={}",BRIDGE)
        self.assertIn("if self.async_kms_mode {",BRIDGE)
        self.assertIn("state.continuous_redraw.set_seat_active(active)",BRIDGE)

    def test_client_buffer_is_released_only_after_both_real_ack_checks(self):
        start=BRIDGE.index("fn poll_async_kms(&mut self)")
        callback=BRIDGE.index("complete_presented_frame(",start)
        ticket=BRIDGE.index("if !self.render_scheduler.complete_present(pending.ticket)",start)
        mirror=BRIDGE.index("self.confirm_generation_proof(pending.layout_generation)?;",start)
        release=BRIDGE.index("buffer.release();",callback)
        self.assertLess(ticket,mirror)
        self.assertLess(mirror,callback)
        self.assertLess(callback,release)
        # Distinct paths must be audited; no frame count cut-off in planner.
        self.assertEqual(BRIDGE.count("self.begin_generation_proof(self.multi_scene.layout_generation())"),4)
        self.assertIn("self.begin_generation_proof(self.multi_scene.layout_generation())",BRIDGE)
        self.assertIn("if let Err(error)=self.begin_generation_proof(self.multi_scene.layout_generation())",BRIDGE)
        self.assertIn("self.reject_generation_proof()",BRIDGE)
        self.assertIn("self.note_scene_change();",BRIDGE)
        self.assertIn("for generation in 1..=64",PLANNER)

    def test_no_new_host_gpu_access_or_session_enablement(self):
        self.assertIn("NATIVE_SESSION_READY=false",BRIDGE)
        self.assertNotIn("NATIVE_SESSION_READY=true",BRIDGE)
        self.assertIn("MELANIA_NATIVE_ASYNC_KMS=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_NATIVE_SCENE_GENERATION_ACK=",GUEST)
        self.assertNotIn("std::thread::sleep(",PLANNER)


if __name__=="__main__":
    unittest.main()
