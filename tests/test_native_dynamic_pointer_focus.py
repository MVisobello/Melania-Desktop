"""Stage55: real input focus and move use the CURRENT dynamic window size.

These structural checks complement the Rust geometry tests. The isolated
QEMU scenario currently proves KMS pixels; a future pointer injector must
also prove interactive dynamic dragging through libinput and CRTC ACK.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
DYNAMIC=(ROOT/"melania-native/src/dynamic_window_scene.rs").read_text()
FIXED=(ROOT/"melania-native/src/multi_window_scene.rs").read_text()

class DynamicNativePointerFocus(unittest.TestCase):
    def test_uses_current_client_geometry_for_focus_and_drag_edges(self):
        for marker in (
            "state.dynamic_scene.hit_test(point.x,point.y)",
            "state.dynamic_scene.raise(id)",
            "state.dynamic_scene.geometry(id)",
            ".map(|(x,y,w,h)|(x,y,w as i32,h as i32))",
            "let original=WindowRect {x,y,width,height};",
            "keyboard.set_focus(state,Some(surface.clone()),",
            "state.focused=Some(surface);",
            "self.dynamic_scene.move_to(key,rect.x,rect.y).is_ok()",
            "self.submit_continuous_frame()",
            "state.submit_continuous_frame()"
        ):
            self.assertIn(marker,BRIDGE)
        self.assertIn("pub fn geometry(&self,key:u64)",DYNAMIC)
        self.assertIn("pointer_geometry_uses_updated_owned_client_size",DYNAMIC)

    def test_legacy_fixed_mode_is_still_available(self):
        self.assertIn("state.multi_scene.hit_test(point.x,point.y)",BRIDGE)
        self.assertIn("state.multi_scene.raise(id)",BRIDGE)
        self.assertIn("state.multi_scene.position(id)",BRIDGE)
        self.assertIn("self.multi_scene.move_to(key,rect.x,rect.y).is_ok()",BRIDGE)
        self.assertIn("pub const WIN_W: usize = 360;",FIXED)

    def test_resize_handle_never_changes_owned_pixels_without_xdg_ack(self):
        start=BRIDGE.index("fn apply_window_motion(&mut self")
        end=BRIDGE.index("fn submit_drag_frame",start)
        block=BRIDGE[start:end]
        self.assertIn("InteractionKind::ResizeBottomRight",block)
        self.assertIn("MELANIA_NATIVE_WINDOW_RESIZE_PREVIEW",block)
        self.assertNotIn("dynamic_scene.upsert(",block)
        self.assertNotIn("dynamic_scene.resize_output(",block)
        self.assertIn("continuous_frame_count<self.continuous_frame_target",block)
        self.assertNotIn("NATIVE_SESSION_READY=true",BRIDGE)

if __name__=="__main__":
    unittest.main()
