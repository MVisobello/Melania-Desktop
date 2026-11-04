"""Real GBM/EGL DMA-BUF -> DRM/KMS scanout, strictly inside QEMU.

Static contracts supplement (never replace) the actual isolated QEMU
test that validates each GPU framebuffer ID, connector route and rollback.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
KMS=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()

class GpuKmsBridge(unittest.TestCase):
    def test_gpu_dma_bufs_are_actually_imported_as_kernel_framebuffers(self):
        for receipt in (
            "GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT",
            "DmabufAllocator(GbmAllocator::new(",
            "GbmDevice::new(card.clone())",
            "Fourcc::Argb8888",
            "framebuffer_from_dmabuf(&card,&gbm,&a,true,true)",
            "framebuffer_from_dmabuf(&card,&gbm,&b,true,true)",
            "card.set_crtc(",
            "card.page_flip(",
            "verify_kernel_scanout(",
            "MELANIA_NATIVE_GBM_KMS_GPU_FRAME_VISIBLE=",
        ):
            self.assertIn(receipt,KMS)
        self.assertIn("frame.finish()?.wait()?", KMS)
        self.assertIn("validate_gbm_output_dimensions(", KMS)
        self.assertIn("pub mod native_gbm_scanout;", LIB)

    def test_restore_occurs_before_gpu_framebuffer_cleanup(self):
        self.assertIn("struct Restore<'a>",KMS)
        self.assertIn("impl Drop for Restore<'_>",KMS)
        self.assertIn("rollback.submitted=true;",KMS)
        self.assertIn("let restore=rollback.restore();",KMS)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=",KMS)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=",KMS)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_TWO_GPU_FRAMES_PASS=true",KMS)
        self.assertLess(KMS.index("let restore=rollback.restore();"),
                        KMS.index("MELANIA_NATIVE_GBM_KMS_TWO_GPU_FRAMES_PASS=true"))

    def test_standalone_libseat_vm_entrypoint_never_enables_login(self):
        self.assertIn("fn experimental_gbm_kms_scanout()",MAIN)
        start=MAIN.index("fn experimental_gbm_kms_scanout()")
        end=MAIN.index("/// One short-lived atomic TEST_ONLY",start)
        body=MAIN[start:end]
        self.assertIn("assert_dedicated_console();",body)
        self.assertIn("assert_qemu_test_guest();",body)
        self.assertIn('MELANIA_NATIVE_VM_GBM_KMS',body)
        self.assertIn("session.open(",body)
        self.assertIn("fd.try_clone()?",body)
        self.assertIn("session.close(fd)",body)
        self.assertLess(body.index("qemu_two_gpu_frames(cloned)"),
                        body.index("session.close(fd)"))
        self.assertIn("NATIVE_SESSION_READY=false",body)
        self.assertNotIn("NATIVE_SESSION_READY=true",MAIN)
        self.assertIn('--experimental-gbm-kms-scanout',MAIN)

    def test_real_guest_and_host_acceptance_are_mandatory(self):
        for marker in (
            "MELANIA_NATIVE_GBM_KMS_GPU_FRAME_VISIBLE=1",
            "MELANIA_NATIVE_GBM_KMS_GPU_FRAME_VISIBLE=2",
            "MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true",
            "MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true",
            "MELANIA_NATIVE_GBM_KMS_LIBSEAT_RELEASED=true",
            "MELANIA_NATIVE_GBM_KMS_TWO_GPU_FRAMES_PASS=true",
        ):
            self.assertIn(marker,GUEST)
        self.assertIn('MELANIA_NATIVE_VM_GBM_KMS=YES_DISPOSABLE_QEMU',GUEST)
        self.assertIn('"$binary" --experimental-gbm-kms-scanout < /dev/tty1',GUEST)
        self.assertIn("MELANIA_QEMU_NATIVE_GBM_KMS_TWO_REAL_FRAMES_PASS=true",GUEST)
        self.assertIn("! grep -q '^MELANIA_QEMU_NATIVE_GBM_KMS_TWO_REAL_FRAMES_PASS=true'",HOST)
        self.assertEqual(GUEST.count("# Stage48:"),1)

if __name__=="__main__":
    unittest.main()
