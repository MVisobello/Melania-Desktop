"""Stage94: the native output dispatch coordinates three independent fail-closed ledgers.

The standalone Rust module does not own a DRM device; real proofs require
the Rust unit tests plus a future privileged hardware backend.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SOURCE = (ROOT / "melania-native/src/native_output_dispatch.rs").read_text()
LIB = (ROOT / "melania-native/src/lib.rs").read_text()


class NativeOutputDispatchStage94(unittest.TestCase):
    def test_module_is_compiled_and_uses_three_existing_ledgers(self):
        self.assertIn("pub mod native_output_dispatch;", LIB)
        for required in (
            "NativeResourceRecovery",
            "ContinuousRedraw",
            "KmsFrameWatchdog",
            "struct NativeOutputDispatch",
        ):
            self.assertIn(required, SOURCE)

    def test_submission_is_issued_only_with_recovery_and_watchdog(self):
        start = SOURCE.index("pub fn dispatch(")
        end = SOURCE.index("pub fn presented(", start)
        block = SOURCE[start:end]
        self.assertIn("RecoveryPhase::Active", block)
        self.assertIn("self.redraw.next_frame()", block)
        self.assertIn("self.resources.next_frame()", block)
        self.assertIn("self.watchdog.arm(", block)
        self.assertLess(block.index("self.watchdog.arm("),
                        block.index("self.inflight = Some(submission)"))
        self.assertIn("self.fail_closed();", block)

    def test_callbacks_require_real_kernel_proof_and_all_acks(self):
        start = SOURCE.index("pub fn presented(")
        end = SOURCE.index("pub fn rejected(", start)
        block = SOURCE[start:end]
        for required in (
            "self.inflight != Some(submission)",
            "!kernel_presented",
            "!self.watchdog.acknowledged(",
            "!self.resources.acknowledge_frame(",
            "!self.redraw.presented(",
            "self.fail_closed();",
        ):
            self.assertIn(required, block)
        self.assertLess(block.index("!self.watchdog.acknowledged("),
                        block.index("!self.resources.acknowledge_frame("))

    def test_pause_and_hotplug_revoke_pending_frames(self):
        pause = SOURCE[SOURCE.index("pub fn pause("):SOURCE.index("pub fn begin_restore(")]
        self.assertIn("self.resources.pause()?", pause)
        self.assertIn("self.redraw.set_seat_active(false)", pause)
        fail = SOURCE[SOURCE.index("pub fn fail_closed("):SOURCE.index("pub fn native_login_authorized(")]
        self.assertIn("self.resources.fail()", fail)
        self.assertIn("self.watchdog.reset(0)", fail)
        self.assertIn("self.redraw.set_seat_active(false)", fail)

    def test_real_native_login_remains_disabled(self):
        self.assertIn("pub fn native_login_authorized(&self) -> bool { false }", SOURCE)
        self.assertIn("fn hundreds_of_confirmed_generations_without_qemu_frame_limit()", SOURCE)
        self.assertIn("fn pause_invalidates_queued_ack_and_resume_requires_full_proof()", SOURCE)
        self.assertIn("fn timeouts_and_unverified_ack_poison_generation()", SOURCE)


if __name__ == "__main__":
    unittest.main()
