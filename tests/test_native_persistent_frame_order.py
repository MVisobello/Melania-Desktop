"""Regression audit for shared ordering in both REAL DRM/KMS presenters.

Rust tests in native_frame_order.rs exercise the ordering policy. These tests
guard the actual CPU and GBM kernel presenter code paths and ensure their
Wayland acknowledgements remain behind verified CRTC scanout.
"""
import unittest
from pathlib import Path
R=Path(__file__).resolve().parents[1]
COMMON=(R/"melania-native/src/native_frame_order.rs").read_text()
LIB=(R/"melania-native/src/lib.rs").read_text()
CPU=(R/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GPU=(R/"melania-native/src/native_gbm_scanout.rs").read_text()

class PersistentFrameOrder(unittest.TestCase):
    def test_runtime_cpu_and_gpu_both_consult_shared_order(self):
        self.assertIn("pub mod native_frame_order;",LIB)
        self.assertIn("pub fn accepted_after(self, previous:Option<Self>, persistent:bool)->bool",COMMON)
        self.assertIn("self.sequence>old.sequence",COMMON)
        self.assertIn("self.epoch==old.epoch",COMMON)
        self.assertIn("(persistent && self.generation==old.generation)",COMMON)
        for code in (CPU,GPU):
            self.assertIn("native_frame_order::FrameIdentity::new(",code)
            self.assertIn("!identity.accepted_after(",code)
            self.assertIn("Some(identity);",code)
        self.assertIn("persistent_repeat_scene_requires_fresh_sequence_and_same_device_epoch",COMMON)
        self.assertIn("thousands_of_persistent_repaints_do_not_require_new_scene_mutations",COMMON)

    def test_finite_frame_proof_stays_strict_on_both_kernel_paths(self):
        # The two persistent CPU scene buffers replace the former
        # per-frame Option<Vec<u8>>. The actual finite-mode unchanged-pixel
        # rejection must still compare the PREVIOUS fully ACKed scene.
        self.assertIn("if !persistent && have_previous_pixels && previous_pixels==pixels",CPU)
        self.assertIn("return Err(\"QEMU continuous KMS received identical",CPU)
        self.assertIn("std::mem::swap(&mut pixels,&mut previous_pixels);",CPU)
        self.assertLess(CPU.index("stream.write_all(&ack)?;"),
                        CPU.index("std::mem::swap(&mut pixels,&mut previous_pixels);"))
        self.assertIn("if have_prior_pixels && prior_pixels==pixels && !persistent",GPU)
        self.assertIn("let total=if persistent {None} else",CPU)
        self.assertIn("if !persistent && frames_seen>=TOTAL {break;}",GPU)

    def test_actual_kernel_scanout_still_precedes_client_ack(self):
        cpu=CPU[CPU.index("fn run_stream("):]
        self.assertLess(cpu.index("wait_for_kms_page_flip_routed("),cpu.index("stream.write_all(&ack)?;"))
        self.assertLess(cpu.index("wait_for_fb(&card,crtc_handle,framebuffer)?;"),cpu.index("stream.write_all(&ack)?;"))
        self.assertLess(cpu.index("verify_kernel_scanout("),cpu.index("stream.write_all(&ack)?;"))
        gpu=GPU[GPU.index("pub fn qemu_wayland_gpu_scene_stream("):]
        self.assertLess(gpu.index("wait_for_scanout(&card,crtc,connector,framebuffer)?;"),
                        gpu.index("stream.write_all(&ack)?;"))
        self.assertLess(gpu.index("stream.flush()?;"),gpu.index("prior_identity=Some(identity);"))
        self.assertIn("let restored=rollback.restore();",gpu)
        self.assertIn("let restored=rollback.restore();",cpu)

if __name__=="__main__":unittest.main()
