"""Ensure the actual DRM scanout ACK rejects a split-read CRTC position change.

This structural guard checks the production drm-rs ioctl path. Real kernel
coverage remains the existing full QEMU guest and eventual physical VT tests.
"""
from pathlib import Path
import unittest

CODE = (Path(__file__).resolve().parents[1] /
        "melania-native/src/native_kms_scanout.rs").read_text()


class KernelScanoutPositionRace(unittest.TestCase):
    def test_real_scanout_ack_rechecks_full_crtc_position(self):
        section = CODE.split("pub fn verify_kernel_scanout(", 1)[1].split(
            "pub fn verify_presenter_ownership(", 1)[0]
        self.assertIn("let current = card.get_crtc(crtc)", section)
        self.assertIn("let latest_crtc = card.get_crtc(crtc)", section)
        self.assertIn("latest_crtc.framebuffer() != Some(expected_fb)", section)
        self.assertIn("latest_crtc.mode() != Some(initial_mode)", section)
        self.assertIn("latest_crtc.position() != current.position()", section)
        self.assertLess(section.index("latest_crtc.position() != current.position()"),
                        section.index("let latest_connector = card.get_connector("))
        self.assertIn("return Ok(false);", section)
        self.assertIn("latest_encoder.crtc() == Some(crtc)", section)

    def test_original_encoder_ack_uses_kernel_scanout_validation(self):
        section = CODE.split("pub fn verify_original_encoder_frame(", 1)[1].split(
            "pub fn verify_rollback_route(", 1)[0]
        self.assertIn("verify_presenter_ownership(", section)
        self.assertIn("original_route(card,crtc,connector,original_encoder)", section)


if __name__ == "__main__":
    unittest.main()
