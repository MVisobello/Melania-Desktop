"""Stage108 real GBM two-buffer reuse and idle-safe persistent Wayland DRM.

The assertions protect source and guest scripts. Only actual QEMU execution
with the virtual DRM device proves pageflips and restored connector routing.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
KMS=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
RECEIVER=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()

class ReusableGpuNativeStage108(unittest.TestCase):
    def test_persistent_receiver_cannot_idle_timeout_on_a_healthy_gpu(self):
        self.assertIn("let idle_persistent=std::env::var(\"MELANIA_NATIVE_PERSISTENT_KMS\")",RECEIVER)
        self.assertIn("stream.set_read_timeout(if idle_persistent {",RECEIVER)
        self.assertIn("None\n    } else {\n        Some(Duration::from_secs(12))",RECEIVER)
        self.assertIn("stream.set_write_timeout(Some(Duration::from_secs(7)))?",RECEIVER)
        self.assertIn("if stream.read(&mut header[..1])?==0",RECEIVER)
        begin=GUEST.index("# A fixed-length receiver would have exited after frame 24")
        segment=GUEST[begin:GUEST.index("kill -TERM \"$persistent_server_pid\"",begin)]
        self.assertIn("sleep 15",segment)
        self.assertIn('kill -0 "$persistent_server_pid"',segment)
        self.assertIn("MELANIA_QEMU_PERSISTENT_IDLE_OVER_12_SECONDS_PASS=true",segment)

    def test_twenty_four_actual_gpu_frames_reuse_two_dma_buffers(self):
        self.assertIn("pub fn qemu_stress_gpu_frames(card_fd:OwnedFd,frames:u32)",KMS)
        self.assertIn("if frames!=24",KMS)
        self.assertIn("fn qemu_gpu_frames(card_fd:OwnedFd,total_frames:u32)",KMS)
        self.assertIn("for index in 0..total_frames",KMS)
        self.assertIn("let buffer=if index%2==0 {&mut a} else {&mut b};",KMS)
        self.assertIn("frame.finish()?.wait()?",KMS)
        self.assertIn("PageFlipFlags::EVENT",KMS)
        self.assertIn("wait_for_kms_page_flip_routed(",KMS)
        self.assertIn("flip_ledger.complete_vm_flip(kernel_frame)",KMS)
        self.assertIn("wait_for_scanout(&card,crtc,connector,handles[index as usize%2])?",KMS)
        self.assertIn("let restore=rollback.restore();",KMS)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_REUSED_DMA_BUFS=2",KMS)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_24_GPU_FRAMES_PASS=true",KMS)

    def test_opt_in_and_old_two_frame_probe_are_preserved(self):
        self.assertIn("pub fn qemu_two_gpu_frames(card_fd:OwnedFd)",KMS)
        self.assertIn("qemu_gpu_frames(card_fd,2)",KMS)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_TWO_GPU_FRAMES_PASS=true",KMS)
        start=MAIN.index("fn experimental_gbm_kms_scanout()")
        end=MAIN.index("/// One short-lived atomic TEST_ONLY",start)
        section=MAIN[start:end]
        self.assertIn("assert_dedicated_console();",section)
        self.assertIn("assert_qemu_test_guest();",section)
        self.assertIn("MELANIA_NATIVE_VM_GBM_STRESS",section)
        self.assertIn("qemu_stress_gpu_frames(cloned,24)",section)
        self.assertIn("qemu_two_gpu_frames(cloned)",section)
        self.assertIn("NATIVE_SESSION_READY=false",section)

    def test_real_vm_guest_requires_all_kernel_flips_and_rollback(self):
        segment=GUEST[GUEST.index("# Stage108: prove that the same GPU/GBM KMS path"):
            GUEST.index("MELANIA_QEMU_NATIVE_GBM_KMS_24_GPU_FRAMES_PASS=true")]
        self.assertIn("MELANIA_NATIVE_VM_GBM_STRESS=YES_DISPOSABLE_QEMU",segment)
        self.assertIn("seq 1 24",segment)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_VBLANK_EVENT_ACK=24",segment)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_FLIP_LEDGER_CONFIRMED=23",segment)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_REUSED_DMA_BUFS=2",segment)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true",segment)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true",segment)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_LIBSEAT_RELEASED=true",segment)

if __name__=="__main__":
    unittest.main()
