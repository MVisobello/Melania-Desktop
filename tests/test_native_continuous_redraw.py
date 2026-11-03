"""Stage42: bounded no-spin planning for a future continuous native renderer.

This is a code-contract test. Rust unit tests exercise all state transitions.
It does NOT claim a native compositor or unlimited KMS frames already work.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
PLANNER=(ROOT/"melania-native/src/continuous_redraw.rs").read_text()
SCHEDULER=(ROOT/"melania-native/src/render_scheduler.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
CI=(ROOT/".github/workflows/linux-verify.yml").read_text()

class ContinuousRedrawContract(unittest.TestCase):
    def test_real_rust_module_uses_bounded_existing_ticket_scheduler(self):
        self.assertIn("pub mod continuous_redraw;",LIB)
        for token in (
            "pub struct ContinuousRedraw", "scheduler: RenderScheduler",
            "pending: Option<PlannedSceneFrame>",
            "observed_generation: Option<u64>",
            "pub fn scene_changed(&mut self, generation: u64)",
            "self.scheduler.damage()", "self.scheduler.begin_frame()?",
            "self.scheduler.complete_present(frame.ticket)",
            "self.scheduler.fail_present(frame.ticket)",
            "self.scheduler.invalidate()",
            "self.scheduler.set_seat_active(active)",
            "self.pending != Some(frame)",
        ):
            self.assertIn(token,PLANNER)
        self.assertIn("pending: Option<FrameTicket>",SCHEDULER)
        self.assertIn("cargo test -p melania-native --all-targets",CI)
        self.assertNotIn("thread::sleep(",PLANNER)
        self.assertNotIn("UnixStream",PLANNER)
        self.assertNotIn("NATIVE_SESSION_READY=true",PLANNER)

    def test_core_unit_proofs_cover_unlimited_successors_and_revocation(self):
        for test in (
            "idle_does_not_produce_frames_or_busy_loop",
            "one_in_flight_and_many_mutations_collapse_to_newest",
            "handles_more_than_six_frames_without_ordinal_limit",
            "pause_revokes_old_ack_and_resume_repaints_latest",
            "output_hotplug_invalidates_inflight_without_stale_release",
            "failed_flip_retries_latest_damage_and_rejects_wrong_tickets",
        ):
            self.assertIn("fn "+test+"()",PLANNER)
        self.assertIn("for generation in 1..=64",PLANNER)
        self.assertIn("assert!(!pump.presented(stale))",PLANNER)


if __name__=="__main__":
    unittest.main()
