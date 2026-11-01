#!/usr/bin/env python3
"""Stage 11: KMS atomic baseline must be read-only, complete and fail-closed.

Real DRM/GBM operations are covered by cargo test and a dedicated VM, not
faked by these source-integrity checks.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
RESTORE = (ROOT / "melania-native/src/kms_restore.rs").read_text()
KMS = (ROOT / "melania-native/src/kms_test.rs").read_text()
MAIN = (ROOT / "melania-native/src/main.rs").read_text()
LIB = (ROOT / "melania-native/src/lib.rs").read_text()


class NativeKmsRollbackBaseline(unittest.TestCase):
    def test_read_only_atomic_snapshot_includes_all_display_objects(self):
        self.assertIn("pub mod kms_restore;", LIB)
        for required in ("fd.resource_handles()", "fd.plane_handles()",
                         "resources.crtcs()", "resources.connectors()",
                         'capture_object(fd, "plane", handle)',
                         "fd.get_properties(handle)", "fd.get_property(id)"):
            self.assertIn(required, RESTORE)

    def test_mandatory_bindings_and_bounded_capture(self):
        for required in ("MAX_CRTCS", "MAX_CONNECTORS", "MAX_PLANES",
                         "MAX_PROPERTIES_PER_OBJECT", '"ACTIVE"', '"MODE_ID"',
                         '"CRTC_ID"', '"FB_ID"', '"SRC_X"', '"CRTC_H"'):
            self.assertIn(required, RESTORE)
        self.assertIn("Duplicate KMS object", RESTORE)
        self.assertIn("Missing {} atomic property", RESTORE)

    def test_kms_test_must_fail_when_observed_state_changes(self):
        self.assertEqual(KMS.count("kms_restore::capture_read_only(&fd)"), 2)
        self.assertIn('guard.verify_after(&after,&resources_after,seat_active(),Instant::now())', KMS)
        self.assertIn("KMS baseline changed during TEST_ONLY preflight", KMS)
        self.assertIn("baseline_unchanged:true", KMS)
        self.assertIn("restoration_implemented:baseline.restoration_implemented()", KMS)
        self.assertIn("report.baseline_unchanged", MAIN)
        self.assertIn("report.restoration_implemented", MAIN)

    def test_no_restore_or_kms_submission_is_claimed(self):
        self.assertIn("pub fn restoration_implemented(&self) -> bool { false }", RESTORE)
        for forbidden in ("set_property(", "atomic_commit(", "queue_buffer(",
                          "page_flip(", "set_crtc(", "reset_state("):
            self.assertNotIn(forbidden, RESTORE)
        self.assertIn("visible_frame_submitted:false", KMS)
        self.assertIn("NATIVE_SESSION_READY=false", MAIN)

    def test_vm_only_gate_is_preserved(self):
        self.assertIn("assert_dedicated_console();\n    assert_qemu_test_guest();", MAIN)
        self.assertIn('YES_QEMU_VM', MAIN)
        self.assertIn('MELANIA_EXPERIMENTAL_SEAT', MAIN)
        self.assertIn('No QEMU DRM device passed atomic TEST_ONLY', MAIN)


if __name__ == "__main__":
    unittest.main()
