"""Stage108: real two-slot GBM scanout reuse, kernel confirmed in QEMU.

Python checks gate wiring. Rust unit tests exercise slot ownership. Only
the separate actual VirtIO guest can establish real DMA-BUF/DRM scanout.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
POOL=(ROOT/"melania-native/src/native_gbm_swapchain.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
GBM=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()

class ReusedNativeGbmKms(unittest.TestCase):
    def test_critical_framebuffer_ownership_stays_per_crtc(self):
        self.assertIn("pub mod native_gbm_swapchain;",LIB)
        for part in [
            "pub struct NativeGbmSwapchain", "crtc: crtc::Handle",
            "buffers: [framebuffer::Handle;2]",
            "pub fn reserve_next(", "pub fn confirm_scanned(",
            "self.current", "self.reserved",
            "WrongOutputOrFramebuffer", "self.revoke()",
            "twelve_presentations_reuse_two_owned_kernel_framebuffers",
            "mismatched_monitor_or_stale_framebuffer_poison_epoch",
            "two_outputs_hold_independent_slots_and_epochs",
        ]:
            self.assertIn(part,POOL)
        self.assertIn("let slot=match self.current",POOL)
        self.assertIn("self.reserved.is_some()",POOL)

    def test_real_gpu_buffers_stay_alive_across_all_12_kms_frames(self):
        for part in [
            "GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT",
            "framebuffer_from_dmabuf(&card,&gbm,&a,true,true)?",
            "framebuffer_from_dmabuf(&card,&gbm,&b,true,true)?",
            "let mut swapchain=crate::native_gbm_swapchain::NativeGbmSwapchain::new(",
            "for index in 0..frame_count",
            "swapchain.reserve_next()", "let buffer=if slot==0 {&mut a} else {&mut b}",
            "card.page_flip(crtc,handles[slot],PageFlipFlags::EVENT,None)",
            "wait_for_kms_page_flip_routed(", "flip_ledger.complete_vm_flip(kernel_frame)",
            "wait_for_scanout(&card,crtc,connector,handles[slot])?",
            "swapchain.confirm_scanned(crtc,handles[slot])",
            "let restore=rollback.restore();",
            "MELANIA_NATIVE_GBM_KMS_REUSED_FRAMES_PASS={frame_count}",
            "qemu_gpu_frames(card_fd,12)",
        ]:
            self.assertIn(part,GBM)
        self.assertLess(GBM.index("framebuffer_from_dmabuf(&card,&gbm,&a,true,true)?"),
                        GBM.index("for index in 0..frame_count"))
        self.assertLess(GBM.index("wait_for_scanout(&card,crtc,connector,handles[slot])?"),
                        GBM.index("swapchain.confirm_scanned(crtc,handles[slot])"))
        self.assertLess(GBM.index("let restore=rollback.restore();"),
                        GBM.index("MELANIA_NATIVE_GBM_KMS_REUSED_FRAMES_PASS={frame_count}"))

    def test_qemu_only_libseat_and_both_legacy_and_reuse_modes(self):
        self.assertIn("assert_dedicated_console();",MAIN)
        self.assertIn("assert_qemu_test_guest();",MAIN)
        self.assertIn("MELANIA_NATIVE_VM_GBM_REUSE",MAIN)
        self.assertIn("qemu_reused_gpu_frames(cloned)",MAIN)
        self.assertIn("qemu_two_gpu_frames(cloned)",MAIN)
        self.assertIn("session.close(fd)",MAIN)
        self.assertIn("NATIVE_SESSION_READY=false",MAIN)
        self.assertIn("MELANIA_NATIVE_VM_GBM_REUSE=YES_DISPOSABLE_QEMU",GUEST)
        for marker in [
            "MELANIA_NATIVE_GBM_KMS_REUSED_FRAMEBUFFERS=2",
            "MELANIA_NATIVE_GBM_KMS_REUSE_COUNT=10",
            "MELANIA_NATIVE_GBM_KMS_VBLANK_EVENT_ACK=12",
            "MELANIA_NATIVE_GBM_KMS_FLIP_LEDGER_CONFIRMED=11",
            "MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true",
            "MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true",
            "MELANIA_NATIVE_GBM_KMS_LIBSEAT_RELEASED=true",
            "MELANIA_QEMU_NATIVE_GBM_KMS_REUSED_12_REAL_FRAMES_PASS=true",
        ]:
            self.assertIn(marker,GUEST)
        self.assertIn(
            "! grep -q '^MELANIA_QEMU_NATIVE_GBM_KMS_REUSED_12_REAL_FRAMES_PASS=true'",
            HOST
        )

if __name__=="__main__":
    unittest.main()
