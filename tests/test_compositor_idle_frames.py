"""Runtime-safe compositor dirty-frame suppression contracts."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
SRC=(ROOT/"melania-compositor/src/main.rs").read_text()
SMOKE=(ROOT/"scripts/smoke-full-desktop-wayland.sh").read_text()

class IdleFrameSafety(unittest.TestCase):
    def test_commit_and_lifecycle_changes_mark_scene_dirty(self):
        for part in (
            "fn commit(&mut self, surface: &WlSurface)",
            "fn new_toplevel(&mut self, top: ToplevelSurface)",
            "fn toplevel_destroyed(&mut self, top: ToplevelSurface)",
            "fn layer_destroyed(&mut self, surface: WlrLayerSurface)",
        ):
            section=SRC[SRC.index(part):SRC.index(part)+260]
            self.assertIn("self.visual_dirty=true;",section)

    def test_idle_egl_and_swap_suppressed_without_disabling_protocol(self):
        self.assertIn("if render_needed {",SRC)
        self.assertIn("if render_needed {backend.submit(Some(&[damage]))?;}",SRC)
        self.assertIn("should_render_scene(state.visual_dirty",SRC)
        self.assertIn("frame_done(top.wl_surface()",SRC)
        self.assertIn("display.dispatch_clients(&mut state)?;",SRC)
        self.assertIn("Duration::from_secs(2)",SRC)

    def test_control_list_does_not_force_fullscreen_gpu_render(self):
        self.assertIn("Ok(Some(ControlCommand::Preview(id)))",SRC)
        self.assertIn("pending_previews.push((client,id))",SRC)
        self.assertIn("ControlCommand::List|ControlCommand::Status",SRC)
        self.assertIn("pending_previews.is_empty()",SRC)

    def test_real_wayland_gui_exercises_idle_suppression(self):
        self.assertIn("MELANIA_COMPOSITOR_FPS",SRC)
        self.assertIn("idle_skipped=",SRC)
        self.assertIn("MELANIA_PROFILE_COMPOSITOR=1",SMOKE)

if __name__=="__main__":
    unittest.main()
