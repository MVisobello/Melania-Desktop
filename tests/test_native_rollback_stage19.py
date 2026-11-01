#!/usr/bin/env python3
"""Stage19 KMS rollback blockers + post-test VT recovery in actual QEMU guest."""
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
RUST=(ROOT/"melania-native/src/rollback_audit.rs").read_text()
KMS=(ROOT/"melania-native/src/kms_test.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
VTPROBE=(ROOT/"scripts/melania-vt-guest.py").read_text()

class RecoveryStage19(unittest.TestCase):
    def test_read_only_rollback_audit_never_claims_ownership(self):
        self.assertIn("pub mod rollback_audit;",LIB)
        for item in (
            "OriginalFramebufferOwnershipNotImplemented",
            "OriginalKernelModeBlobOwnershipNotImplemented",
            "AtomicRollbackNotImplemented",
            "VtRecoveryWithLiveScanoutNotVerified",
            "pub fn visible_kms_commit_allowed(&self) -> bool {false}",
            "pub fn rollback_implemented(&self) -> bool {false}",
            "Missing read-only active mode payload",
            "Old framebuffer binding changed during audit",
            "active_crtcs",
        ):
            self.assertIn(item,RUST)
        for forbidden in ("atomic_commit(", "page_flip(", "set_crtc(", "set_property(",
                          "create_property_blob(", "queue_buffer("):
            self.assertNotIn(forbidden,RUST)
        for unit_test in ("valid_observations_still_have_four_hard_blockers",
                          "inactive_output_does_not_make_a_rollback_legal",
                          "missing_owned_mode_payload_is_rejected",
                          "framebuffer_binding_mismatch_is_rejected",
                          "malformed_active_crtc_is_rejected"):
            self.assertIn(f"fn {unit_test}()",RUST)

    def test_real_kms_test_requires_audit_before_render(self):
        self.assertIn("RollbackAudit::inspect(&baseline,&resource_copies)",KMS)
        self.assertIn("if audit.visible_kms_commit_allowed() || audit.rollback_implemented()",KMS)
        self.assertIn("rollback_blockers:audit.blocker_count()",KMS)
        self.assertIn("report.rollback_blockers",MAIN)
        self.assertIn("restore_ready={} visible={} rollback_blockers={}",MAIN)
        self.assertIn("visible_frame_submitted:false",KMS)

    def test_guest_runs_vt_after_actual_rust_test_and_fails_closed(self):
        native=GUEST.index('"$binary" --experimental-kms-test')
        native_success=GUEST.index("MELANIA_QEMU_NATIVE_KMS_PASS=true")
        vt_check=GUEST.index('python3 "$vt_probe"')
        vt_success=GUEST.index("MELANIA_QEMU_NATIVE_POST_KMS_VT_PASS=true")
        self.assertLess(native,native_success)
        self.assertLess(native_success,vt_check)
        self.assertLess(vt_check,vt_success)
        self.assertIn("post_KMS_linux_VT_recovery_failed",GUEST)
        self.assertIn("MELANIA_QEMU_NATIVE_GUEST_EXIT=0",HOST)
        self.assertIn("MELANIA_QEMU_NATIVE_POST_KMS_VT_PASS=true",HOST)
        self.assertIn("MELANIA_QEMU_VT_SWITCH=true",HOST)
        self.assertIn("MELANIA_QEMU_VT_PASS=true",HOST)
        self.assertIn("rollback_blockers=4",HOST)
        self.assertIn("MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false",HOST)
        self.assertIn("VT_ACTIVATE",VTPROBE)
        self.assertIn("VT_WAITACTIVE",VTPROBE)

    def test_no_host_gpu_or_read_write_drm_api_in_guest_scripts(self):
        for forbidden in ("vfio", "accel=kvm", "/dev/dri/card0", "DRM_IOCTL_MODE_ATOMIC"):
            self.assertNotIn(forbidden,HOST)
        self.assertIn("accel=tcg",HOST)
        self.assertIn("-vga virtio",HOST)
        self.assertIn("sha256sum -c verify.sha256",HOST)
        self.assertIn("MELANIA_QEMU_NATIVE_VM",HOST)
        self.assertIn("MELANIA_NATIVE_VISIBLE_SCANOUT=false",GUEST)
        r=subprocess.run(["bash","scripts/run-qemu-melania-native.sh"],
                         cwd=ROOT,env={"PATH":"/usr/bin:/bin"},
                         capture_output=True,text=True,timeout=5)
        self.assertEqual(r.returncode,78)

if __name__=="__main__":
    unittest.main()
