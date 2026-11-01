#!/usr/bin/env python3
"""Stage14: QEMU KMS TEST_ONLY drains live seat/udev event sources at checkpoints."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
KMS=(ROOT/"melania-native/src/kms_test.rs").read_text()
EPOCH=(ROOT/"melania-native/src/device_epoch.rs").read_text()
HELPER=(ROOT/"scripts/smoke-qemu-kms-resources.sh").read_text()

class LiveNativeEvents(unittest.TestCase):
    def test_qemu_preflight_installs_both_real_event_sources(self):
        self.assertIn("pub mod device_epoch;",LIB)
        part=MAIN.split("fn experimental_kms_test()",1)[1]
        self.assertIn("EventLoop::<NativeLifecycle>::try_new()",part)
        self.assertIn("insert_source(notifier,",part)
        self.assertIn("insert_source(udev,",part)
        self.assertIn("state.on_session(event);",part)
        self.assertIn("state.on_udev(event);",part)
        self.assertIn("event_loop.dispatch(Some(Duration::ZERO),&mut state)",part)
        self.assertIn("epoch.verify(&state).is_err()",part)

    def test_drm_preflight_polls_at_guard_checkpoints_and_fails_closed(self):
        self.assertIn("FnMut() -> Result<bool,String>",KMS)
        self.assertIn("if !seat_active()? {",KMS)
        self.assertGreaterEqual(KMS.count("guard.checkpoint(seat_active()?,Instant::now())"),3)
        self.assertIn("guard.verify_after(&after,&resources_after,seat_active()?,Instant::now())",KMS)
        self.assertIn('QEMU preflight aborted without scanout',MAIN)
        self.assertIn('if invalidated {',MAIN)

    def test_epoch_detects_loss_of_seat_gpu_removal_and_reused_device_id(self):
        for name in ("pause_resume_never_revalidates_a_previous_device_epoch",
                     "gpu_hotplug_change_and_remove_invalidate_the_epoch",
                     "reused_id_with_another_device_path_is_never_accepted",
                     "cannot_begin_checkpoints_with_an_inactive_seat"):
            self.assertIn(f"fn {name}()",EPOCH)
        self.assertIn("state.frame_gate().generation()",EPOCH)
        self.assertIn("state.devices().get(&self.id)",EPOCH)

    def test_no_scanout_and_dedicated_vm_gate_remain_enforced(self):
        for forbidden in ("page_flip(", "atomic_commit(", "queue_buffer(", "set_crtc("):
            self.assertNotIn(forbidden,EPOCH)
        self.assertIn("visible_frame_submitted:false",KMS)
        self.assertIn("NATIVE_SESSION_READY=false",MAIN)
        self.assertIn("assert_dedicated_console();\n    assert_qemu_test_guest();",MAIN)
        self.assertIn("MELANIA_KMS_TEST_ONLY",MAIN)
        self.assertIn('45s "$binary" --experimental-kms-test',HELPER)

if __name__=="__main__":
    unittest.main()
