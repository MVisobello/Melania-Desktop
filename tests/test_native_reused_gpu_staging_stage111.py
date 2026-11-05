"""Stage111: persistent real GPU scene staging without per-frame Vec allocations."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
GPU=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()

class NativeReusedGpuStaging111(unittest.TestCase):
    def test_exactly_two_staging_allocations_before_unbounded_gpu_loop(self):
        code=GPU[GPU.index("pub fn qemu_wayland_gpu_scene_stream("):]
        start=code.index("let mut prior_identity")
        loop=code.index("loop {",start)
        setup=code[start:loop]
        render=code[loop:code.index("let restored=rollback.restore();",loop)]
        self.assertIn("let mut pixels=vec![0u8;W*H*4];",setup)
        self.assertIn("let mut prior_pixels=vec![0u8;W*H*4];",setup)
        self.assertIn("let mut have_prior_pixels=false;",setup)
        self.assertNotIn("vec![0u8;W*H*4]",render)
        self.assertNotIn("prior_pixels=Some(pixels)",render)
        self.assertIn("stream.read_exact(&mut pixels)?;",render)

    def test_swap_only_after_verified_kernel_scanout_and_client_ack(self):
        code=GPU[GPU.index("pub fn qemu_wayland_gpu_scene_stream("):]
        point=code.index('println!("MELANIA_NATIVE_GPU_WAYLAND_FRAME_VISIBLE={}"')
        ack=code.index("stream.write_all(&ack)?;",point)
        swap=code.index("std::mem::swap(&mut pixels,&mut prior_pixels);",ack)
        self.assertLess(code.index("wait_for_scanout(&card,crtc,connector,framebuffer)?;"),point)
        self.assertLess(ack,swap)
        self.assertLess(code.index("stream.flush()?;",ack),swap)
        self.assertIn("have_prior_pixels=true;",code[swap:swap+130])
        self.assertIn("if have_prior_pixels && prior_pixels==pixels",code)
        self.assertIn("MELANIA_NATIVE_GPU_WAYLAND_FLIP_LEDGER",code)

if __name__=="__main__":
    unittest.main()
