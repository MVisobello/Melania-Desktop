#!/usr/bin/env python3
"""Stage36: native drag/resize model connected to real VM libinput events.

The Rust tests exercise the finite grab state and geometry bounds; Python
ensures the Wayland bridge does not fake KMS presentation on pointer-only
updates, and that the guest/native-login guard remains fail-closed.
"""
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
MODEL=(ROOT/"melania-native/src/window_interaction.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
LIB=(ROOT/"melania-native/src/lib.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class NativeWindowInteractionsStage36(unittest.TestCase):
    def test_bounded_model_has_explicit_mouse_grab_and_session_cancel(self):
        for token in (
            "const OUT_W:i32=1280",
            "const OUT_H:i32=800",
            "const MIN_W:i32=96",
            "const MIN_H:i32=72",
            "pub enum InteractionKind {Move, ResizeBottomRight, Resize(ResizeDirection)}",
            "pub fn begin(", "pub fn motion(", "pub fn end(", "pub fn cancel(",
            "self.active.is_some()", "self.x0",  # replaced below
            "fn drag_moves_without_resizing_and_bounds_to_virtual_output()",
            "fn resize_bottom_right_obeys_minimum_and_display_bounds()",
            "fn invalid_client_id_bad_coordinates_and_geometry_are_rejected()",
        ):
            if token=="self.x0":
                self.assertIn("active.x0",MODEL)
            else:
                self.assertIn(token,MODEL)
        self.assertIn("pub mod window_interaction;",LIB)

    def test_real_libinput_updates_model_without_premature_gpu_claim(self):
        for token in (
            "window_interaction:WindowInteraction",
            "fn apply_window_motion(&mut self,point:Point<f64,Logical>)",
            "state.apply_window_motion(point);",
            "self.multi_scene.move_to(key,rect.x,rect.y)",
            "MELANIA_NATIVE_WINDOW_DRAG_MODEL=",
            "MELANIA_NATIVE_WINDOW_RESIZE_PREVIEW=",
            "event.button_code()==0x110",
            "event.state()==ButtonState::Pressed",
            "event.state()==ButtonState::Released",
            "state.window_interaction.begin(",
            "state.window_interaction.end()",
            "state.window_interaction.cancel()",
        ):
            self.assertIn(token,BRIDGE)
        # The existing KMS frames are still derived ONLY from real client
        # wl_shm commits, never from a cursor movement or fake screenshot.
        self.assertIn("fn commit(&mut self,surface:&WlSurface)",BRIDGE)
        self.assertIn("if self.async_kms_mode {",BRIDGE)
        self.assertIn("worker.submit(FrameJob{ticket,ordinal:next,pixels:frame})",BRIDGE)
        start=BRIDGE.index("fn apply_window_motion(")
        end=BRIDGE.index("/// Poll the worker",start)
        self.assertNotIn("complete_presented_frame(",BRIDGE[start:end])
        self.assertNotIn("buffer.release()",BRIDGE[start:end])
        self.assertNotIn("stream.write_all(",BRIDGE[start:end])

    def test_isolation_and_regression_gates_remain(self):
        self.assertIn("test_native_window_interactions_stage36.py",CI)
        self.assertIn("cargo test -p melania-native --all-targets",CI)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        self.assertNotIn("MELANIA_NATIVE_SESSION_READY=true",GUEST)

if __name__=="__main__":
    unittest.main()
