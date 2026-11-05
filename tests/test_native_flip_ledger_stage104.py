"""Stage104: real DRM page flip completion gates monotonic output receipts."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
LEDGER=(ROOT/"melania-native/src/native_flip_ledger.rs").read_text()
KERNEL=(ROOT/"melania-native/src/native_drm_flip.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
RECEIVER=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GBM=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()

class NativeKernelFlipLedger(unittest.TestCase):
    def test_actual_kernel_receipt_is_available_not_just_crtc_read(self):
        self.assertIn("pub mod native_flip_ledger;",LIB)
        self.assertIn("-> Result<u32, String>",KERNEL)
        self.assertIn("router.enqueue(done.crtc, done.frame)?",KERNEL)
        self.assertIn("done.crtc==expected_crtc",KERNEL)
        self.assertIn("pub fn begin_flip(",LEDGER)
        self.assertIn("pub fn complete_flip(",LEDGER)
        self.assertIn("pub fn revoke(",LEDGER)
        self.assertIn("frame.wrapping_sub(last)",LEDGER)
        self.assertIn("StaleOrDuplicateVblank",LEDGER)
        self.assertIn("fn virtio_can_report_repeated_or_reset_counters_with_real_events()",LEDGER)
        self.assertIn("fn vm_counter_compatibility_never_invents_an_event_or_rearms_failure()",LEDGER)
        self.assertIn("thousands_of_sequential_presentations_without_qemu_budget",LEDGER)
    def test_real_kms_receiver_checks_completed_kernel_frame_before_ack(self):
        self.assertIn("flip_ledger.begin_flip()",RECEIVER)
        self.assertIn("flip_ledger.complete_vm_flip(kernel_frame)",RECEIVER)
        self.assertLess(RECEIVER.index("flip_ledger.begin_flip()"),
                        RECEIVER.index("card.page_flip("))
        first=RECEIVER.index("let kernel_frame=melania_native::native_drm_flip")
        self.assertLess(first,RECEIVER.index("flip_ledger.complete_vm_flip(kernel_frame)",first))
        self.assertLess(RECEIVER.index("flip_ledger.complete_vm_flip(kernel_frame)",first),
                        RECEIVER.index("verify_kernel_scanout(",first))
        self.assertLess(RECEIVER.index("verify_kernel_scanout(",first),
                        RECEIVER.index("stream.write_all(&ack)?",first))
    def test_real_gbm_scanout_uses_same_lifecycle(self):
        self.assertIn("flip_ledger.begin_flip()",GBM)
        self.assertIn("flip_ledger.complete_vm_flip(kernel_frame)",GBM)
        self.assertLess(GBM.index("flip_ledger.complete_vm_flip(kernel_frame)"),
                        GBM.index("wait_for_scanout(&card,crtc,connector,handles[index])?"))
    def test_qemu_proof_needs_actual_second_and_twelfth_frame_events(self):
        self.assertIn("MELANIA_NATIVE_GBM_KMS_FLIP_LEDGER_CONFIRMED=1",GUEST)
        self.assertIn("MELANIA_RUST_KMS_FLIP_LEDGER_CONFIRMED=11",GUEST)
        self.assertIn("NATIVE_SESSION_READY=false",RECEIVER)

if __name__=="__main__":
    unittest.main()
