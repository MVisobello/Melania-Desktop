#!/usr/bin/env python3
"""Native DRM stage3 must remain a read-only connector and mode inventory."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
OUTPUTS=(ROOT/"melania-native/src/outputs.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
CARGO=(ROOT/"melania-native/Cargo.toml").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()

class NativeOutputs(unittest.TestCase):
    def test_real_device_resource_inventory(self):
        self.assertIn('drm = "0.14"',CARGO)
        self.assertIn("pub mod outputs;",LIB)
        self.assertIn("fd.resource_handles()",OUTPUTS)
        self.assertIn("resources.connectors()",OUTPUTS)
        self.assertIn("fd.get_connector(*handle,false)",OUTPUTS)
        self.assertIn("fd.get_encoder(*handle)",OUTPUTS)
        self.assertIn("resources.filter_crtcs(encoder.possible_crtcs())",OUTPUTS)
        self.assertIn("plan_crtc_routes(&routes)",MAIN)
        planes=(ROOT/"melania-native/src/planes.rs").read_text()
        self.assertIn("device.planes(crtc)",planes)
        self.assertIn("planes.primary.len()",planes)
        self.assertIn("format.code==Fourcc::Abgr8888",planes)
        self.assertIn("format.modifier==Modifier::Linear",planes)
        self.assertNotIn("set_crtc(",planes)
        self.assertNotIn("create_surface(",planes)
        self.assertIn("melania_native::planes::inspect_primary_planes(&device)",MAIN)
        self.assertIn("two_monitors_never_share_a_crtc",OUTPUTS)
        self.assertIn("planner_does_not_invent_crtcs_for_unrouteable_monitors",OUTPUTS)
        self.assertIn("order_of_discovery_does_not_change_mapping",OUTPUTS)
        self.assertIn("info.modes()",OUTPUTS)
        self.assertIn("info.interface_id()",OUTPUTS)
        self.assertIn("info.state()==connector::State::Connected",OUTPUTS)

    def test_read_only_and_hotplug_safe(self):
        self.assertNotIn("set_crtc(",OUTPUTS)
        self.assertNotIn("reset_state(",OUTPUTS)
        self.assertNotIn("create_surface(",OUTPUTS)
        self.assertIn("Err(error)=>eprintln!",OUTPUTS)
        self.assertIn("advertised_modes.sort_unstable_by_key",OUTPUTS)
        self.assertIn("advertised_modes.dedup()",OUTPUTS)
        self.assertIn("melania_native::outputs::inspect_connectors(device.device_fd())",MAIN)
        self.assertIn('NOT applied',MAIN)

    def test_recommendation_never_applied(self):
        for name in ("pick_largest_resolution_then_refresh_without_mutating_modes",
                     "zero_bogus_and_empty_modes_do_not_create_fake_outputs",
                     "valid_connected_monitor_is_candidate_only_not_activated"):
            self.assertIn(name,OUTPUTS)
        self.assertIn("!crate::NativeLifecycle::default().can_present_kms()",OUTPUTS)

if __name__=="__main__":
    unittest.main()
