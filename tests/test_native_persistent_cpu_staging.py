"""Persistent Rust KMS CPU staging memory contract.

The full QEMU guest proves actual VirtIO CPU KMS scanout, FB_ID/vblank,
source pixel changes, original CRTC restore and staging reuse. These structural
tests guard against reintroducing a 4 MiB heap allocation per frame.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
RUST = (ROOT / "melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GUEST = (ROOT / "scripts/melania-qemu-native-guest.sh").read_text()
GPU = (ROOT / "melania-native/src/native_gbm_scanout.rs").read_text()


class PersistentCpuKmsStaging(unittest.TestCase):
    def test_exactly_two_owned_scene_buffers_are_reused_after_kernel_ack(self):
        start = RUST.index("fn run_stream(")
        stream = RUST[start:]
        self.assertIn("let mut pixels=vec![0u8;W*H*4];", stream)
        self.assertIn("let mut previous_pixels=vec![0u8;W*H*4];", stream)
        self.assertEqual(stream.count("vec![0u8;W*H*4]"), 2)
        self.assertIn("read_finite_exact_guarded(", stream)
        self.assertIn("&mut stream,&mut pixels,", stream)
        self.assertIn("&mut verify_read_drm,", stream)
        self.assertIn("std::mem::swap(&mut pixels,&mut previous_pixels);", stream)
        self.assertNotIn("previous=Some(pixels)", stream)
        self.assertLess(
            stream.index("stream.write_all(&ack)?;"),
            stream.index("std::mem::swap(&mut pixels,&mut previous_pixels);"),
        )
        self.assertLess(
            stream.index("verify_original_encoder_frame_at_baseline("),
            stream.index("stream.write_all(&ack)?;"),
        )

    def test_finite_probe_still_rejects_identical_pixels(self):
        self.assertIn(
            "if !persistent && have_previous_pixels && previous_pixels==pixels",
            RUST,
        )
        self.assertIn("return Err(\"QEMU continuous KMS received identical", RUST)
        self.assertIn("have_previous_pixels=true;", RUST)
        self.assertIn("let staging_allocations=[pixels.as_ptr(),previous_pixels.as_ptr()];", RUST)
        self.assertIn("current_allocations==staging_allocations", RUST)

    def test_real_24_frame_qemu_demands_two_persistent_allocations(self):
        self.assertIn("MELANIA_RUST_KMS_CPU_STAGING_BUFFERS_REUSED=2", RUST)
        self.assertIn(
            "'MELANIA_RUST_KMS_CPU_STAGING_BUFFERS_REUSED=2'",
            GUEST,
        )
        self.assertIn("'MELANIA_RUST_CONTINUOUS_FRAMES_PASS=24'", GUEST)
        self.assertIn("'MELANIA_RUST_KMS_ROLLBACK_ORIGINAL_ENCODER_VERIFIED=true'", GUEST)

    def test_original_encoder_is_pinned_before_cpu_and_gbm_buffers(self):
        # These are the real rust-drm connector IOCTL results, read AFTER
        # inspect_stable_active_outputs. A different encoder aimed at the same
        # CRTC must never be mistaken for the inventoried scanout owner.
        cpu=RUST[RUST.index("fn run_stream("):]
        self.assertEqual(cpu.count("original_encoder!=route.original_encoder"),1)
        self.assertLess(cpu.index("original_encoder!=route.original_encoder"),
                        cpu.index("let mut db1=card.create_dumb_buffer("))
        self.assertLess(cpu.index("original_encoder!=route.original_encoder"),
                        cpu.index("let mut rollback=Restore{"))
        # One check in actual GPU KMS smoke, one in the persistent real
        # Wayland GPU DMA-BUF scanout; neither is an isolated helper-only fix.
        self.assertEqual(GPU.count("original_encoder!=route.original_encoder"),2)
        finite=GPU[GPU.index("fn qemu_gpu_frames("):
                   GPU.index("pub fn qemu_wayland_gpu_scene_stream(")]
        persistent=GPU[GPU.index("pub fn qemu_wayland_gpu_scene_stream("):]
        for backend in (finite,persistent):
            self.assertEqual(backend.count("original_encoder!=route.original_encoder"),1)
            self.assertLess(backend.index("original_encoder!=route.original_encoder"),
                            backend.index("let gbm=GbmDevice::new(card.clone())?;"))
            self.assertIn("card.get_encoder(original_encoder)?.crtc()!=Some(crtc)",backend)
        self.assertIn("Original CPU scanout connector has no encoder",cpu)
        self.assertIn("Native GBM output missing its original connector encoder",GPU)

    def test_qemu_and_native_session_gates_unchanged(self):
        self.assertIn("fn guest_only()", RUST)
        self.assertIn("YES_DISPOSABLE_QEMU", RUST)
        self.assertIn("NATIVE_SESSION_READY=false", RUST)


if __name__ == "__main__":
    unittest.main()
