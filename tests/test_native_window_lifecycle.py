"""Regression guards for native QEMU Wayland window close and slot reuse.

The Rust tests in window_slots.rs check semantics with actual state
transitions. This test ensures the Smithay bridge uses that registry for
commits, input hit testing, and xdg_toplevel teardown.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
REGISTRY=(ROOT/"melania-native/src/window_slots.rs").read_text()
SCENE=(ROOT/"melania-native/src/multi_window_scene.rs").read_text()


class NativeWindowLifetime(unittest.TestCase):
    def test_qemu_scene_uses_stable_bounded_window_ids(self):
        self.assertIn("pub mod window_slots;",LIB)
        self.assertIn("known_window_surfaces:WindowSlots<WlSurface>",BRIDGE)
        self.assertIn("known_window_surfaces:WindowSlots::default()",BRIDGE)
        self.assertIn("known_window_surfaces.register(surface.clone())",BRIDGE)
        self.assertIn("known_window_surfaces.get(key)?.clone()",BRIDGE)
        self.assertIn("const MAX_SLOTS:usize=2;",REGISTRY)
        self.assertIn("pub fn register(&mut self,surface:T)->Result<u64,SlotError>",REGISTRY)
        self.assertIn("pub fn remove(&mut self,surface:&T)->Option<u64>",REGISTRY)
        self.assertIn("fn stale_and_duplicate_destroy_never_affect_new_owner()",REGISTRY)

    def test_destroy_drops_scene_and_cancels_grabs_before_refocus(self):
        code=BRIDGE.split(
            "fn toplevel_destroyed(&mut self,top:ToplevelSurface){",1
        )[1].split("\n    fn grab(",1)[0]
        for marker in (
            "self.known_window_surfaces.remove(&surface)",
            "self.multi_scene.remove(key)",
            "self.window_interaction.active_key()==Some(key)",
            "self.window_interaction.cancel();",
            "self.focused.as_ref()==Some(&surface)",
            "self.focused=None;",
            "self.pointer_target(self.pointer_position)",
            "keyboard.set_focus(self,replacement.clone(),SERIAL_COUNTER.next_serial());",
            "pointer.motion(self,target,&MotionEvent{",
        ):
            self.assertIn(marker,code)
        self.assertLess(code.index("self.multi_scene.remove(key)"),
                        code.index("self.pointer_target(self.pointer_position)"))
        self.assertIn("pub fn remove(&mut self, key: u64) -> bool",SCENE)

    def test_kms_proof_survives_destroyed_toplevel_without_stale_scene_assertions(self):
        # QEMU's client destroys its roles after ACK 3 and while ACK 4 is
        # outstanding. A separate QEMU close-redraw stage may retain a second live window.
        self.assertIn("self.two_window_frame_presented=true;",BRIDGE)
        self.assertIn("self.drag_frame_presented=true;",BRIDGE)
        self.assertIn("if !state.two_window_frame_presented {",BRIDGE)
        self.assertIn("if state.pixel_frame_count<4 || !state.drag_frame_presented {",BRIDGE)
        self.assertNotIn("state.multi_scene.layout_generation()!=state.presented_layout_generation",BRIDGE)
        self.assertNotIn("if state.multi_scene.window_count()!=2 {",BRIDGE)
        ack=BRIDGE.index("if !self.render_scheduler.complete_present(pending.ticket) {")
        flag=BRIDGE.index("self.drag_frame_presented=true;",ack)
        self.assertGreater(flag,BRIDGE.index("self.presented_layout_generation=pending.layout_generation;",ack))
        destroy=BRIDGE.index("fn toplevel_destroyed(&mut self,top:ToplevelSurface){")
        self.assertIn("self.multi_scene.remove(key)",BRIDGE[destroy:])
        self.assertIn('MELANIA_NATIVE_SESSION_READY=false',BRIDGE)

    def test_session_still_cannot_advertise_native_desktop(self):
        for path in ("melania-native/src/wayland_bridge.rs",
                     "melania-native/src/main.rs"):
            text=(ROOT/path).read_text()
            self.assertIn("NATIVE_SESSION_READY=false",text)
            self.assertNotIn("NATIVE_SESSION_READY=true",text)


if __name__=="__main__":
    unittest.main()
