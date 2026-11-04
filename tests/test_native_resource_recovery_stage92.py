"""Stage92: static integration checks; Rust unit tests validate state transitions."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]


class NativeRecoveryStage92(unittest.TestCase):
    def test_resource_policy_is_exported_without_login_bypass(self):
        lib = (ROOT / "melania-native/src/lib.rs").read_text()
        policy = (ROOT / "melania-native/src/native_resource_recovery.rs").read_text()
        self.assertIn("pub mod native_resource_recovery;", lib)
        for marker in (
            "pub fn begin_acquire(", "pub fn begin_restore(", "pub fn confirm(",
            "pub fn pause(", "pub fn fail(", "pub fn next_frame(",
            "pub fn acknowledge_frame(", "pub fn failed_frame(",
            "fn repeated_frames_are_unbounded_but_never_concurrent()",
            "fn pause_invalidates_pending_ack_and_requires_full_restoration()",
        ):
            self.assertIn(marker, policy)
        self.assertIn("pub fn native_login_authorized(&self) -> bool { false }", policy)

    def test_receiver_budget_is_shared_and_fail_closed(self):
        parser = (ROOT / "melania-native/src/native_frame_budget.rs").read_text()
        receiver = (ROOT / "melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
        self.assertIn("pub fn receiver_frame_budget(", parser)
        self.assertIn("fn receiver_rejects_mismatch_and_invalid_opt_in()", parser)
        self.assertIn("native_frame_budget::receiver_frame_budget(", receiver)
        self.assertNotIn(".unwrap_or(12);", receiver)
        self.assertIn("let configured=std::env::var", receiver)

    def test_login_script_remains_blocked_until_real_backend(self):
        launcher = (ROOT / "scripts/melania-session").read_text()
        self.assertIn("NATIVE_BACKEND_UNAVAILABLE", launcher)
        self.assertNotIn("/usr/share/wayland-sessions", launcher.split("--check-native", 1)[-1].split("--probe-native", 1)[0])


if __name__ == "__main__":
    unittest.main()
