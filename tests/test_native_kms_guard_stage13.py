#!/usr/bin/env python3
"""Stage 13: RAII KMS preflight and lifecycle gate, never visible scanout."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
GUARD=(ROOT/"melania-native/src/preflight_guard.rs").read_text()
KMS=(ROOT/"melania-native/src/kms_test.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
SHELL=(ROOT/"scripts/smoke-qemu-kms-resources.sh").read_text()

class NativeKmsGuardStage13(unittest.TestCase):
    def test_guard_owns_both_immutable_captures_and_is_fail_closed(self):
        self.assertIn("pub mod preflight_guard;",LIB)
        for text in ("before: AtomicBaseline", "resources: KmsResourceInventory",
                     "pub fn checkpoint(", "pub fn invalidate(&mut self)",
                     "pub fn verify_after(", "impl Drop for NativePreflightGuard",
                     "blob.bytes.fill(0)", "MAX_PREFLIGHT: Duration = Duration::from_secs(40)",
                     "if !after.unchanged_since(&self.before)",
                     "if !after_resources.unchanged_since(&self.resources)"):
            self.assertIn(text,GUARD)
        for name in ("SeatInactive", "Expired", "Invalidated",
                     "BaselineChanged", "ResourcesChanged"):
            self.assertIn(name,GUARD)
        self.assertIn("pub fn restoration_implemented(&self) -> bool { false }",GUARD)
        self.assertIn("pub fn visible_frame_allowed(&self) -> bool { false }",GUARD)

    def test_real_qemu_preflight_uses_seat_and_deadline_checkpoints(self):
        self.assertIn("NativePreflightGuard::capture(",KMS)
        self.assertGreaterEqual(KMS.count("guard.checkpoint(seat_active(),Instant::now())"),3)
        self.assertIn("guard.verify_after(&after,&resources_after,seat_active(),Instant::now())",KMS)
        self.assertIn("prepare_and_render_test_only(&mut device,||session.is_active())",MAIN)
        self.assertIn("assert_dedicated_console();",MAIN)
        self.assertIn("assert_qemu_test_guest();",MAIN)
        self.assertIn('timeout --signal=TERM --kill-after=3s 45s',SHELL)

    def test_guard_has_no_kms_or_console_mutation(self):
        for forbidden in ("atomic_commit(", "page_flip(", "queue_buffer(", "set_crtc(",
                          "set_property(", "create_property_blob(", "ioctl(",
                          "kill(", "set_current_vt("):
            self.assertNotIn(forbidden,GUARD)
        self.assertIn("visible_frame_submitted:false",KMS)
        self.assertIn("NATIVE_SESSION_READY=false",MAIN)

    def test_rust_tests_cover_resume_hotplug_timeout_and_changed_state(self):
        for test in ("pause_invalidates_every_pending_guard_even_after_resume",
                     "gpu_hotplug_cannot_reuse_a_stale_snapshot",
                     "bounded_deadline_fails_closed_and_stays_aborted",
                     "detects_changed_atomic_properties_and_blob_payloads",
                     "successful_read_only_comparison_never_authorizes_scanout"):
            self.assertIn(f"fn {test}()",GUARD)

if __name__=="__main__":
    unittest.main()
