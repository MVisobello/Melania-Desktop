"""Stage57 dirty regions are geometry hints, never KMS presentation ACKs."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
SOURCE=(ROOT/"melania-native/src/damage_regions.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()

class NativeDamageRegions(unittest.TestCase):
    def test_export_and_conservative_bounded_damage(self):
        self.assertIn("pub mod damage_regions;",LIB)
        for marker in (
            "pub const MAX_DAMAGE_RECTS: usize = 64",
            "pub fn clip(&self",
            "saturating_add(i64::try_from(width)",
            "pub fn window_moved(",
            "pub fn z_order_changed(",
            "pub fn resize_output(",
            "self.invalidate_all();",
            "pub fn clear_after_presented(",
            "self.rects.swap_remove(index);",
        ):
            self.assertIn(marker,SOURCE)

    def test_real_kms_ack_not_bypassed_by_damage(self):
        self.assertIn("submit_continuous_frame()",BRIDGE)
        self.assertIn("self.continuous_client_ledger.acknowledged(clients)",BRIDGE)
        self.assertIn("A dirty region is NOT a DRM ACK",SOURCE)
        self.assertNotIn("NATIVE_SESSION_READY=true",SOURCE)
        self.assertNotIn("DrmDevice",SOURCE)

    def test_rust_regressions_cover_old_pixels_and_budget(self):
        for test in (
            "clips_negative_and_extreme_coordinates_without_overflow",
            "moving_and_shrinking_invalidates_both_old_and_new_extents",
            "merge_adjacent_rectangles_and_keep_disjoint_changes",
            "bounded_region_overflow_falls_back_to_full_output",
            "output_resize_invalidates_all_and_rejects_zero_without_mutation",
        ):
            self.assertIn("fn "+test+"()",SOURCE)

if __name__=="__main__":
    unittest.main()
