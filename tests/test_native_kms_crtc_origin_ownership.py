"""Check the production DRM owner path rejects split CRTC origin snapshots.

The full QEMU suite independently exercises actual DRM KMS page flips and
encoder route checks; this structural guard protects the additional origin
invariant without claiming simulated hotplug or physical VT acceptance.
"""
import unittest
from pathlib import Path

SRC=(Path(__file__).resolve().parents[1]/
     "melania-native/src/native_kms_scanout.rs").read_text()

class NativeKmsCrtcOriginOwnership(unittest.TestCase):
    def test_real_presenter_owner_reads_and_compares_crtc_origin(self):
        start=SRC.index("pub fn verify_presenter_ownership(")
        end=SRC.index("pub fn verify_original_encoder_frame(")
        presenter=SRC[start:end]
        self.assertIn("let current=card.get_crtc(crtc)",presenter)
        self.assertIn("let latest_crtc=card.get_crtc(crtc)",presenter)
        self.assertIn("latest_crtc.position()!=current.position()",presenter)
        self.assertIn("latest_crtc.mode()!=Some(initial_mode)",presenter)
        self.assertIn("latest_crtc.framebuffer()!=Some(expected_fb)",presenter)
        self.assertIn("return Ok(false);",presenter)

    def test_original_encoder_ack_still_uses_real_presenter_owner(self):
        start=SRC.index("pub fn verify_original_encoder_frame(")
        end=SRC.index("pub fn verify_rollback_route(")
        presenter=SRC[start:end]
        self.assertIn("verify_presenter_ownership(card,crtc,connector,expected_fb,expected_size)",presenter)
        self.assertEqual(presenter.count("original_route(card,crtc,connector,original_encoder)"),2)

if __name__=="__main__":
    unittest.main()
