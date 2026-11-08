"""Regression for real drm-rs topology inventory under mid-read KMS takeover.

These assertions protect the production kernel-ioctl read path. They are not
a synthetic hotplug demonstration or acceptance of real physical VT handling.
The repository's Rust build and isolated VirtIO QEMU suite remain mandatory.
"""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
SRC = (ROOT / "melania-native/src/native_output_topology.rs").read_text()


class NativeTopologySnapshotOwnership(unittest.TestCase):
    def test_each_kernel_route_is_rechecked_before_being_advertised(self):
        section = SRC.split("pub fn inspect_active_outputs(", 1)[1].split(
            "pub fn inspect_stable_active_outputs(", 1
        )[0]
        for expected in (
            "let latest_crtc=card.get_crtc(crtc_handle)",
            "latest_crtc.framebuffer()!=Some(original_fb)",
            "latest_crtc.mode()!=Some(mode)",
            "latest_crtc.position()!=crtc_info.position()",
            "origin:crtc_info.position()",
            "let latest_connector=card.get_connector(connector_handle,false)",
            "latest_connector.state()!=connector::State::Connected",
            "latest_connector.current_encoder()!=Some(encoder_handle)",
            "let latest_encoder=card.get_encoder(encoder_handle)",
            "latest_encoder.crtc()!=Some(crtc_handle)",
        ):
            self.assertIn(expected, section)
        self.assertLess(
            section.index("latest_crtc.position()!=crtc_info.position()"),
            section.index("active.push(ActiveOutputRoute{"),
        )
        self.assertLess(
            section.index("latest_encoder.crtc()!=Some(crtc_handle)"),
            section.index("active.push(ActiveOutputRoute{"),
        )
        self.assertGreaterEqual(section.count("return Err("), 5)

    def test_inventory_remains_read_only_and_outer_stability_check_remains(self):
        self.assertNotIn("card.set_crtc(", SRC)
        self.assertNotIn("card.page_flip(", SRC)
        self.assertIn("let before=inspect_active_outputs(card)?;", SRC)
        self.assertIn("let after=inspect_active_outputs(card)?;", SRC)
        self.assertIn("if before!=after", SRC)
        self.assertIn("origin:crtc_info.position()", SRC)
        self.assertIn("fn crtc_origin_takeover_between_full_kernel_scans_is_detected()", SRC)
        self.assertIn("validate_active_routes(&active)?;", SRC)


if __name__ == "__main__":
    unittest.main()
