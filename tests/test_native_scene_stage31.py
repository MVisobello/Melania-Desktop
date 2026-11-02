#!/usr/bin/env python3
"""Stage 31: QEMU's real client pixels traverse the bounded layer compositor.

Rust unit tests exercise actual pixel mixing, clipping, stride validation,
stable layer ordering and oversized-frame rejection. This structural gate
ensures those tests are wired into the active VM workflow.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
SCENE=(ROOT/"melania-native/src/scene.rs").read_text()
FRAME=(ROOT/"melania-native/src/native_client_frame.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class NativeSceneStage31(unittest.TestCase):
    def test_scene_module_is_used_for_real_wayland_client_pixels(self):
        self.assertIn("pub mod scene;",LIB)
        self.assertIn("scene::compose(OUT_W,OUT_H",FRAME)
        self.assertIn("Pixels::OpaqueBgra{data:&client,stride:WIN_W*4}",FRAME)
        self.assertIn("Pixels::Solid([216,171,65,0])",FRAME)
        self.assertIn("Layer::Bottom",FRAME)
        self.assertIn("Layer::Normal",FRAME)
        self.assertIn("with_buffer_contents(&buffer", (ROOT/"melania-native/src/wayland_bridge.rs").read_text())
        self.assertIn("cargo test -p melania-native --all-targets",CI)

    def test_scene_bounds_ordering_and_alpha_have_rust_tests(self):
        for needle in (
            "MAX_FRAME_BYTES",
            "checked_mul(height)",
            "checked_mul(*stride)",
            "SceneError::InvalidSurfaceBuffer",
            "Layer::Background, Layer::Bottom, Layer::Normal",
            "Pixels::StraightAlphaBgra",
            "fn honors_layers_and_stable_order()",
            "fn clips_negative_origin_and_right_bottom_edges()",
            "fn reads_correct_rows_when_clipped_and_stride_padded()",
            "fn blends_straight_alpha_without_changing_xrgb_high_byte()",
            "fn rejects_truncation_malformed_stride_and_huge_output()",
        ):
            self.assertIn(needle,SCENE)

    def test_vm_only_without_native_login_flag(self):
        self.assertIn("NATIVE_SESSION_READY=false",
            (ROOT/"melania-native/src/bin/qemu-rust-kms-stream.rs").read_text())
        self.assertNotIn("MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=true",FRAME+SCENE)
        self.assertIn("test_native_scene_stage31.py",CI)

if __name__=="__main__":
    unittest.main()
