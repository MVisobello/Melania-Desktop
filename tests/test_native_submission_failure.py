"""A failed native submission must exit through owned KMS rollback."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
class NativeSubmissionFailure(unittest.TestCase):
    def test_all_callback_submissions_use_the_fatal_latch(self):
        wrapper=BRIDGE.split("fn submit_continuous_frame(",1)[1].split("fn try_submit_continuous_frame(",1)[0]
        self.assertLess(wrapper.index("continuous_submission_failure.check()?"),
                        wrapper.index("self.try_submit_continuous_frame()"))
        self.assertIn("continuous_submission_failure.record(error.to_string())",wrapper)
        self.assertEqual(BRIDGE.count("self.try_submit_continuous_frame()"),1)
    def test_latch_precedes_any_frame_ack_or_client_flush(self):
        poll=BRIDGE.split("fn poll_continuous_kms(",1)[1].split("fn begin_generation_proof(",1)[0]
        self.assertLess(poll.index("continuous_submission_failure.check()?"),poll.index("worker.try_result()"))
        dispatch=BRIDGE.split("display.dispatch_clients(&mut state)?;",1)[1]
        self.assertLess(dispatch.index("state.continuous_submission_failure.check()?"),dispatch.index("state.poll_explicit_acquires()?"))
        self.assertLess(dispatch.index("state.continuous_submission_failure.check()?"),dispatch.index("display.flush_clients()?"))
    def test_fatal_error_path_joins_presenter_before_releasing_drm(self):
        cleanup=BRIDGE.split("if integrated_kms && run_result.is_err()",1)[1]
        self.assertLess(cleanup.index("state.continuous_worker.take()"),cleanup.index("join.join()"))
        self.assertLess(cleanup.index("join.join()"),cleanup.index("owner.release()"))
