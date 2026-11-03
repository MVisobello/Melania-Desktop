"""Stage52: dynamic native retained windows, no production KMS activation."""
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
SCENE=(ROOT/"melania-native/src/dynamic_window_scene.rs").read_text()
COMPOSITOR=(ROOT/"melania-native/src/scene.rs").read_text()
LEGACY=(ROOT/"melania-native/src/multi_window_scene.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
SESSION=(ROOT/"scripts/melania-session").read_text()

class NativeDynamicWindowGeometry(unittest.TestCase):
    def test_new_scene_is_exported_without_replacing_proven_vm(self):
        self.assertIn("pub mod dynamic_window_scene;",LIB)
        self.assertIn("pub mod multi_window_scene;",LIB)
        self.assertIn("const WIN_W: usize = 360",LEGACY)
        self.assertIn("const WIN_H: usize = 224",LEGACY)
        self.assertIn("--check-native",SESSION)
        self.assertIn("NATIVE_BACKEND_UNAVAILABLE",SESSION)

    def test_bounded_dynamic_scene_owns_pixels_and_tracks_generation(self):
        for marker in (
            "pub const MAX_DYNAMIC_WINDOWS: usize=16",
            "pub const MAX_TOTAL_CLIENT_BYTES: usize=96*1024*1024",
            "pub const MAX_OUTPUT_BYTES: usize=64*1024*1024",
            "width.checked_mul(height)",
            "data.len()!=size",
            "pub fn resize_output(",
            "pub fn upsert(",
            "pub fn move_to(",
            "pub fn raise(",
            "pub fn remove(",
            "pub fn hit_test(",
            "pub fn composite(",
            "self.dirty();",
            "scene::compose(self.width,self.height,self.background,&surfaces)"
        ):
            self.assertIn(marker,SCENE)

    def test_wayland_argb_is_premultiplied_and_existing_straight_still_supported(self):
        self.assertIn("PremultipliedAlphaBgra",COMPOSITOR)
        self.assertIn("StraightAlphaBgra",COMPOSITOR)
        self.assertIn("u32::from(color[c]).min(a)",COMPOSITOR)
        self.assertIn("pixels:Pixels::PremultipliedAlphaBgra",SCENE)
        self.assertIn("blends_wayland_premultiplied_argb_without_double_alpha",COMPOSITOR)

    def test_rust_functional_contract_tests_cover_geometry_and_retention(self):
        for name in (
            "different_client_sizes_retain_pixels_on_other_client_damage",
            "overlapping_focus_raise_and_real_premultiplied_alpha",
            "move_close_reopen_and_output_change_clip_without_stale_pixels",
            "size_count_and_memory_budgets_fail_before_mutation"
        ):
            self.assertIn("fn "+name+"()",SCENE)
        self.assertNotIn("NATIVE_SESSION_READY=true",SCENE)
        self.assertNotIn("DrmDevice",SCENE)

if __name__=="__main__":
    unittest.main()
