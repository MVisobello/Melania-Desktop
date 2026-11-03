"""QEMU scene-position regression: slot identity survives close and reopen.

The Rust unit tests exercise the actual RGBA compositor, with per-pixel
assertions; this source contract keeps them part of the Linux CI entry point.
It does not claim that a new KMS presentation was performed by this test.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
SCENE=(ROOT/"melania-native/src/multi_window_scene.rs").read_text()
LINUX=(ROOT/".github/workflows/linux-verify.yml").read_text()


class NativeReopenPosition(unittest.TestCase):
    def test_slot_to_coordinates_not_visible_count(self):
        block=SCENE.split("pub fn upsert(&mut self, key: u64, pixels: Vec<u8>)",1)[1].split(
            "pub fn remove(&mut self, key: u64)",1
        )[0]
        self.assertIn("usize::try_from(key).ok()",block)
        self.assertIn("(1..=MAX_WINDOWS).contains(n)",block)
        self.assertIn("let (x,y)=POSITIONS[slot];",block)
        self.assertNotIn("let (x,y)=POSITIONS[self.windows.len()];",block)

    def test_real_rust_compositor_pixel_checks(self):
        for name in (
            "fn closed_first_client_can_reopen_without_overlapping_second()",
            "fn left_slot_remains_free_when_right_client_is_first_to_commit()",
        ):
            self.assertIn(name,SCENE)
        self.assertIn("graph.remove(1)",SCENE)
        self.assertIn("graph.composite().unwrap()",SCENE)
        self.assertIn("sample(&removed,200,200)",SCENE)
        self.assertIn("sample(&reopened,700,250)",SCENE)
        self.assertIn("cargo test -p melania-native --all-targets",LINUX)


if __name__=="__main__":
    unittest.main()
