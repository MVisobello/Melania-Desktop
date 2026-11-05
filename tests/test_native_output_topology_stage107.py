"""Stage107 real connector -> encoder -> CRTC inventory, COSMIC-informed.

Source-level guard assertions do not replace the real QEMU DRM test, which
must observe stable output topology and successful actual kernel scanout.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
TOPO=(ROOT/"melania-native/src/native_output_topology.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
DRM=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
GBM=(ROOT/"melania-native/src/native_gbm_scanout.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()

class RealNativeOutputTopology(unittest.TestCase):
    def test_kernel_topology_is_read_from_active_fd_not_fabricated(self):
        self.assertIn("pub mod native_output_topology;",LIB)
        for check in [
            "card.resource_handles()", "card.get_connector(connector_handle,false)",
            "card.get_encoder(encoder_handle)", "card.get_crtc(crtc_handle)",
            "connector::State::Connected", "info.current_encoder()",
            "crtc_info.framebuffer()", "crtc_info.mode()", "mode.size()",
            "inspect_stable_active_outputs(", "before!=after",
        ]:
            self.assertIn(check,TOPO)
        self.assertNotIn("card.set_crtc(",TOPO)
        self.assertNotIn("card.page_flip(",TOPO)
        self.assertNotIn('"/dev/dri/card0"',TOPO)

    def test_bounded_independent_output_ownership(self):
        for check in [
            "MAX_ACTIVE_DRM_OUTPUTS:usize=8",
            "MAX_CONNECTORS:usize=32", "MAX_CRTCS:usize=32",
            "old.connector==route.connector || old.crtc==route.crtc",
            "active.len()>MAX_ACTIVE_DRM_OUTPUTS",
            "two_independent_outputs_keep_unique_crtcs_and_borrowed_fbs",
            "cloned_crtc_or_duplicate_connector_is_not_silently_shared",
            "rejects_empty_excessive_and_invalid_output_modes",
        ]:
            self.assertIn(check,TOPO)
        self.assertIn("pub original_fb:framebuffer::Handle",TOPO)
        self.assertIn("single_active(&self)",TOPO)

    def test_both_actual_qemu_scanout_paths_require_live_stable_inventory(self):
        for source in [DRM,GBM]:
            self.assertIn("inspect_stable_active_outputs(&card)?",source)
            self.assertIn("topology.single_active()?",source)
            self.assertIn("original.framebuffer()!=Some(route.original_fb)",source)
            self.assertIn("route.connector",source)
            self.assertIn("route.crtc",source)
            self.assertIn("verify_kernel_scanout(",source)
        self.assertIn("MELANIA_RUST_KMS_TOPOLOGY_STABLE_OUTPUTS=1",GUEST)
        self.assertIn("MELANIA_NATIVE_GBM_KMS_TOPOLOGY_STABLE_OUTPUTS=1",GUEST)
        self.assertIn("NATIVE_SESSION_READY=false",DRM)

if __name__ == "__main__":
    unittest.main()
