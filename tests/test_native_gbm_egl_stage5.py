#!/usr/bin/env python3
"""Native stage5 must initialize GBM/EGL/GLES without enabling KMS scanout."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
CARGO=(ROOT/"melania-native/Cargo.toml").read_text()
RENDERER=(ROOT/"melania-native/src/renderer.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()

class NativeGbmEglStage5(unittest.TestCase):
    def test_feature_gated_backend(self):
        for feature in ("backend_gbm","backend_egl","renderer_gl","backend_session_libseat"):
            self.assertIn(feature,CARGO)
        self.assertIn("pub mod renderer;",LIB)
        self.assertIn("pub fn inspect_gbm_egl(fd: DrmDeviceFd)",RENDERER)
        self.assertIn("GbmDevice::new(fd)",RENDERER)
        self.assertIn("EGLDisplay::new(gbm)",RENDERER)
        self.assertIn("EGLContext::new(&display)",RENDERER)
        self.assertIn("GlesRenderer::new(context)",RENDERER)
        self.assertIn("dmabuf_render_formats().iter().count()",RENDERER)

    def test_hardware_probe_only_after_tty_guard_and_seat_open(self):
        self.assertIn("assert_dedicated_console();",MAIN)
        self.assertIn("session.open(",MAIN)
        self.assertIn("melania_native::renderer::inspect_gbm_egl(device.device_fd().clone())",MAIN)
        self.assertIn("NO scanout",MAIN)
        self.assertNotIn("set_crtc(",RENDERER)
        self.assertNotIn("create_surface(",RENDERER)
        # Stage 5 is still capability-only; stage 6 intentionally allocates
        # a tiny DMA-BUF in a separate, opt-in function.
        capability_only=RENDERER.split("pub fn inspect_gbm_egl(",1)[1].split(
            "pub fn smoke_render_gbm_offscreen(",1)[0]
        self.assertNotIn("create_buffer(",capability_only)
        self.assertIn("pub fn native_scanout_is_implemented() -> bool {false}",RENDERER)
        self.assertIn('NATIVE_SESSION_READY=false',MAIN)

if __name__=="__main__":
    unittest.main()
