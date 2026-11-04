"""Native GPU rendering at real output resolutions: no DRM modeset or login.

The Rust unit tests cover the memory budget. These Python checks protect the
CLI/device safety contract; only a dedicated-console GPU test exercises EGL.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
RENDERER = (ROOT / "melania-native/src/renderer.rs").read_text()
NATIVE = (ROOT / "melania-native/src/main.rs").read_text()

class NativeGbmOutputProbe(unittest.TestCase):
    def test_real_output_sized_gpu_path_reuses_libseat_device(self):
        self.assertIn('fn experimental_drm(render_offscreen: bool, output_sized: bool)', NATIVE)
        self.assertIn('assert_dedicated_console();', NATIVE)
        self.assertIn('session.open(', NATIVE)
        self.assertIn('inspect_connectors(device.device_fd())', NATIVE)
        self.assertIn('recommended_mode_for_budget(', NATIVE)
        self.assertIn('smoke_render_gbm_output_buffers(', NATIVE)
        self.assertIn('device.device_fd().clone(),mode.width,mode.height,2', NATIVE)
        self.assertIn('MELANIA_NATIVE_GBM_OUTPUT_SIZED_SYNCHRONIZED=', NATIVE)
        self.assertIn('No connected monitor mode fits the GPU memory budget', NATIVE)

    def test_not_installable_and_no_kms_mutation_in_probe(self):
        self.assertIn('--experimental-gbm-output', NATIVE)
        self.assertIn('experimental_drm(true,true)', NATIVE)
        self.assertIn('NATIVE_SESSION_READY=false', NATIVE)
        self.assertIn('validate_gbm_output_dimensions(width,height,frames)?;', RENDERER)
        self.assertIn('allocator.create_buffer(', RENDERER)
        self.assertIn('frame.finish()', RENDERER)
        self.assertIn('.wait().map_err(', RENDERER)
        self.assertIn('native_scanout_is_implemented() -> bool {false}', RENDERER)
        for forbidden in ('set_crtc(', 'page_flip(', 'atomic_commit(', 'create_surface('):
            self.assertNotIn(forbidden, RENDERER)

    def test_rust_limits_and_multi_frame_sync_are_verified(self):
        self.assertIn('fn output_sized_gpu_probe_limits_gpu_memory_and_oversized_edid()', RENDERER)
        self.assertIn('96*1024*1024', RENDERER)
        self.assertIn('for index in 0..frames {', RENDERER)
        self.assertIn('width>4096 || height>4096', RENDERER)

if __name__ == "__main__":
    unittest.main()
