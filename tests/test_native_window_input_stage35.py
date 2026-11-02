#!/usr/bin/env python3
"""Stage35 contract: dynamic two-window geometry and physical pointer targeting.

The Rust unit tests exercise clipping, hit-testing, stacking and identity.
The native VM acceptance still requires real KMS pixel captures and the
previous Stage33/34 ticket and callback checks.
"""
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
SCENE=(ROOT/"melania-native/src/multi_window_scene.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
INJECT=(ROOT/"scripts/inject-qemu-two-window-focus.py").read_text()

class NativeWindowInputStage35(unittest.TestCase):
    def test_scene_has_stable_geometry_and_bounded_hit_test(self):
        for token in (
            "x: i32,", "y: i32,", "pub fn move_to(", "pub fn position(",
            "pub fn hit_test(", "pub fn raise(", "x.is_finite()", "y.is_finite()",
            "self.windows.iter().rev()", "MultiSceneError::InvalidPosition",
            "window.x,window.y", "MAX_WINDOWS: usize = 2",
            "fn move_hit_test_and_raise_preserve_stable_id()",
            "fn invalid_geometry_and_unknown_window_fail_without_mutation()",
        ):
            self.assertIn(token,SCENE)
        self.assertNotIn("let (x,y)=POSITIONS[index];",SCENE)

    def test_actual_wayland_pointer_and_keyboard_are_dispatched_by_hit_test(self):
        self.assertIn("fn pointer_target(&self,point:Point<f64,Logical>)",BRIDGE)
        self.assertIn("self.multi_scene.hit_test(point.x,point.y)",BRIDGE)
        self.assertIn("self.known_window_surfaces.get((key-1) as usize)",BRIDGE)
        self.assertIn("self.multi_scene.position(key)",BRIDGE)
        self.assertIn("event.position_transformed((1280,800).into())",BRIDGE)
        self.assertIn("let target=state.pointer_target(point)",BRIDGE)
        self.assertIn("event.state()==ButtonState::Pressed",BRIDGE)
        self.assertIn("state.multi_scene.raise(id)",BRIDGE)
        self.assertIn("keyboard.set_focus(state,Some(surface.clone())",BRIDGE)
        self.assertIn("MELANIA_NATIVE_WINDOW_POINTER_FOCUS={id}",BRIDGE)
        self.assertIn("state.focused.is_none() && !state.multi_window_mode",BRIDGE)

    def test_old_vm_safety_and_actual_rust_tests_remain_in_place(self):
        self.assertIn("MELANIA_RUST_MULTI_FRAME2_ACTIVE=true",INJECT)
        self.assertIn("MELANIA_RUST_MULTI_FRAME3_ACTIVE=true",INJECT)
        self.assertIn("MELANIA_NATIVE_WINDOW_POINTER_FOCUS=2",INJECT)
        self.assertIn("MELANIA_NATIVE_WINDOW_POINTER_FOCUS=1",INJECT)
        self.assertIn('"type":typ,"data":data',INJECT)
        self.assertIn("qmp-two-window-focus.log",HOST)
        self.assertIn("MELANIA_QEMU_TWO_WINDOW_REAL_INPUT_PASS=true",HOST)
        self.assertIn("MELANIA_NATIVE_WINDOW_POINTER_ABS",BRIDGE)
        self.assertIn("MELANIA_NATIVE_WINDOW_BUTTON=",BRIDGE)
        self.assertIn("for stage,part in",INJECT)
        self.assertIn("time.sleep(.12)",INJECT)
        self.assertIn("test_native_window_input_stage35.py",CI)
        self.assertIn("cargo test -p melania-native --all-targets",CI)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        self.assertNotIn("MELANIA_NATIVE_SESSION_READY=true",GUEST)

if __name__=="__main__":
    unittest.main()
