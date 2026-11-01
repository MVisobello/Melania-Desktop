#!/usr/bin/env python3
"""Atomic KMS TEST_ONLY is VM/TTY gated and never submits a visible frame."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
KMS=(ROOT/"melania-native/src/kms_test.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
WORKFLOW=(ROOT/".github/workflows/linux-verify.yml").read_text()

class KmsAtomicTestOnly(unittest.TestCase):
    def test_actual_driver_test_and_render_candidate(self):
        self.assertIn("GbmBufferedSurface::new(",KMS)
        self.assertIn("device.create_surface(crtc,mode.clone()",KMS)
        self.assertIn("GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT",KMS)
        self.assertIn("swapchain.next_buffer()",KMS)
        self.assertIn("renderer.render(&mut target",KMS)
        self.assertIn("frame.finish()",KMS)
        self.assertIn(".wait()",KMS)
        self.assertIn("if !device.is_atomic()",KMS)
        self.assertIn("visible_frame_submitted:false",KMS)

    def test_experiment_is_qemu_only_fail_closed(self):
        self.assertIn("fn assert_qemu_test_guest()",MAIN)
        self.assertIn('MELANIA_KMS_TEST_ONLY',MAIN)
        self.assertIn('YES_QEMU_VM',MAIN)
        self.assertIn('/sys/class/dmi/id/sys_vendor',MAIN)
        self.assertIn('assert_dedicated_console();',MAIN)
        self.assertIn('assert_qemu_test_guest();',MAIN)
        self.assertIn('experimental_kms_test()',MAIN)
        self.assertIn('No QEMU DRM device passed atomic TEST_ONLY',MAIN)
        self.assertIn('NATIVE_SESSION_READY=false',MAIN)
        self.assertIn('native-stage9-denied.log',WORKFLOW)

    def test_no_display_commit_or_false_success(self):
        for forbidden in ("queue_buffer(", "page_flip(", "set_crtc(", "commit(", "frame_submitted(", "reset_state("):
            self.assertNotIn(forbidden,KMS)
        self.assertIn('native_scanout_is_implemented() -> bool {false}',
                      (ROOT/"melania-native/src/renderer.rs").read_text())

if __name__=="__main__":
    unittest.main()
