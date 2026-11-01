#!/usr/bin/env python3
"""Stage12: data retention is bounded, read-only and never a restoration claim."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
RESOURCES=(ROOT/"melania-native/src/kms_resources.rs").read_text()
KMS=(ROOT/"melania-native/src/kms_test.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
SHELL=(ROOT/"scripts/smoke-qemu-kms-resources.sh").read_text()
WORKFLOW=(ROOT/".github/workflows/linux-verify.yml").read_text()

class NativeKmsResourcesStage12(unittest.TestCase):
    def test_bounded_read_only_blob_payload_capture(self):
        self.assertIn("pub mod kms_resources;",LIB)
        for text in (
            'MAX_BLOBS: usize = 128',
            'MAX_BLOB_BYTES: usize = 256 * 1024',
            'MAX_ALL_BLOB_BYTES: usize = 2 * 1024 * 1024',
            'MAX_FRAMEBUFFER_REFS: usize = 128',
            '"MODE_ID"', '"GAMMA_LUT"', '"DEGAMMA_LUT"',
            '"CTM"', '"HDR_OUTPUT_METADATA"',
            'fd.get_property_blob(id)',
            'total_bytes.checked_add(bytes.len())',
            'Active CRTC {} has no MODE_ID',
        ):
            self.assertIn(text,RESOURCES)
        for forbidden in ("create_property_blob(", "atomic_commit(", "set_crtc(", "page_flip("):
            self.assertNotIn(forbidden,RESOURCES)

    def test_framebuffer_observations_are_never_claimed_as_owned(self):
        self.assertIn('original_id: property.value',RESOURCES)
        self.assertIn('NEVER resource ownership',RESOURCES)
        self.assertIn('pub fn restoration_implemented(&self) -> bool { false }',RESOURCES)
        self.assertIn('pub fn borrowed_framebuffer_count(&self)',RESOURCES)
        self.assertIn('detects_changes_in_payload_even_if_blob_ids_are_unchanged',RESOURCES)

    def test_vm_preflight_rechecks_blob_bytes_and_framebuffer_links(self):
        self.assertIn('kms_resources::capture_read_only(&fd,&baseline)',KMS)
        self.assertIn('kms_resources::capture_read_only(&fd,&after)',KMS)
        self.assertIn('guard.verify_after(&after,&resources_after,seat_active(),Instant::now())',KMS)
        self.assertIn('resources_unchanged:true',KMS)
        self.assertIn('guard.copied_blobs()',KMS)
        self.assertIn('report.resources_unchanged',MAIN)
        self.assertIn('report.restoration_implemented',MAIN)
        self.assertIn('visible_frame_submitted:false',KMS)
        self.assertIn('NATIVE_SESSION_READY=false',MAIN)

    def test_optional_vm_helper_is_bounded_and_not_host_executable(self):
        for marker in (
            'MELANIA_QEMU_RESOURCE_AUDIT',
            'I_AM_IN_AN_ISOLATED_QEMU_VM',
            'MELANIA_KMS_TEST_ONLY',
            'MELANIA_EXPERIMENTAL_SEAT',
            '45s "$binary" --experimental-kms-test',
            'NATIVE_STAGE12_BLOCKED',
        ):
            self.assertIn(marker,SHELL)
        self.assertIn('native-stage12-denied.log',WORKFLOW)
        self.assertIn("grep -q 'NATIVE_STAGE12_BLOCKED'",WORKFLOW)
        self.assertIn('assert_qemu_test_guest();',MAIN)

if __name__=="__main__":
    unittest.main(verbosity=2)
