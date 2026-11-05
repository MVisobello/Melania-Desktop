"""Stage114: real xdg_toplevel.resize verified by client pixel commit.

The source contract is not evidence of a physical DRM session; the KMS QEMU
workflow and Rust unit tests must still pass before this can be merged.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
GEOMETRY=(ROOT/"melania-native/src/window_interaction.rs").read_text()

class NativeAuthenticatedResize114(unittest.TestCase):
    def test_client_resize_is_one_shot_and_seat_authenticated(self):
        start=BRIDGE.index("fn resize_request(&mut self,top:ToplevelSurface")
        end=BRIDGE.index("fn maximize_request(",start)
        handler=BRIDGE[start:end]
        for marker in (
            "xdg_resize_direction(edge)",
            "self.managed_xdg_mode",
            "self.dynamic_window_mode",
            "self.lifecycle.seat_active()",
            "self.native_presentation_guard.permanently_revoked()",
            "self.focused.as_ref()!=Some(surface)",
            "take_xdg_pointer_press(&mut self.last_xdg_press,surface,serial)",
            "window.state()==WindowDisplayState::Normal",
            "self.dynamic_scene.geometry(key)",
            "self.window_interaction.begin(",
            "InteractionKind::Resize(direction)",
        ):
            self.assertIn(marker,handler)
        self.assertNotIn("self.dynamic_scene.upsert(",handler)
        self.assertNotIn("buffer.release()",handler)

    def test_all_xdg_edges_bounded_and_correctly_mapped(self):
        for edge in ("Top","Bottom","Left","Right",
                     "TopLeft","TopRight","BottomLeft","BottomRight"):
            self.assertIn("Edge::"+edge+"=>ResizeDirection::"+edge,BRIDGE)
            self.assertIn("ResizeDirection::"+edge,GEOMETRY)
        self.assertIn("eight_xdg_edges_keep_opposite_edges_fixed",GEOMETRY)
        self.assertIn("left_top_resize_cannot_escape_output_or_invert_window",GEOMETRY)
        self.assertIn("MIN_W",GEOMETRY)
        self.assertIn("MIN_H",GEOMETRY)

    def test_configure_does_not_claim_a_visible_frame(self):
        start=BRIDGE.index("fn apply_window_motion(")
        end=BRIDGE.index("fn submit_drag_frame(",start)
        motion=BRIDGE[start:end]
        self.assertIn("self.managed_pending_resize.push((surface.clone(),rect));",motion)
        self.assertIn("self.managed_pending_sizes.push((",motion)
        self.assertIn("pending.states.set(xdg_toplevel::State::Resizing)",motion)
        self.assertIn("top.send_pending_configure()",motion)
        self.assertNotIn("self.dynamic_scene.upsert(",motion[motion.index("Some(InteractionKind::ResizeBottomRight"):])

    def test_client_owned_size_required_before_new_position_is_visible(self):
        start=BRIDGE.index("let expected=self.managed_pending_sizes.iter()")
        end=BRIDGE.index('println!("MELANIA_NATIVE_MANAGED_XDG_CLIENT_BUFFER_ACCEPTED=',start)
        section=BRIDGE[start:end]
        for marker in (
            "accepts_managed_role_buffer(expected,(pixels.width(),pixels.height()))",
            "MELANIA_NATIVE_MANAGED_XDG_STALE_BUFFER_REJECTED=true",
            "self.managed_pending_resize.iter()",
            "self.dynamic_scene.upsert(key,x,y,pixels)",
            "window.update_normal_geometry(WindowRect{x,y,width,height})",
            "self.managed_pending_resize.retain(|(role,_)|role!=surface)",
        ):
            self.assertIn(marker,section)
        self.assertLess(section.index("accepts_managed_role_buffer(expected"),
                        section.index("self.dynamic_scene.upsert(key,x,y,pixels)"))
        self.assertLess(section.index("self.dynamic_scene.upsert(key,x,y,pixels)"),
                        section.index("window.update_normal_geometry("))

    def test_role_switch_revokes_only_its_grab_and_clears_resizing(self):
        begin=BRIDGE.index("fn cancel_surface_grab(&mut self,surface:&WlSurface)")
        end=BRIDGE.index("fn request_managed_maximize(",begin)
        helper=BRIDGE[begin:end]
        self.assertIn("self.known_window_surfaces.id_of(surface)",helper)
        self.assertIn("self.window_interaction.cancel_for_key(key)",helper)
        for start_marker,end_marker in (
            ("fn request_managed_maximize(", "fn request_managed_fullscreen("),
            ("fn request_managed_fullscreen(", "\\n}\\n\\nimpl BridgeState {"),
        ):
            start=BRIDGE.index(start_marker)
            end=BRIDGE.find(end_marker,start)
            self.assertGreater(end,start)
            role=BRIDGE[start:end]
            self.assertIn("if changed { self.cancel_surface_grab(&surface); }",role)
            self.assertIn("pending.states.unset(xdg_toplevel::State::Resizing)",role)
            self.assertLess(role.index("self.cancel_surface_grab(&surface)"),
                            role.index("top.send_pending_configure()"))
        self.assertIn("fn role_transition_cancels_only_matching_grab()",GEOMETRY)
        self.assertIn("pub fn cancel_for_key(&mut self,key:u64)->bool",GEOMETRY)

    def test_releasing_real_mouse_clears_resizing_state(self):
        start=BRIDGE.index("InputEvent::PointerButton {event} =>")
        end=BRIDGE.index("state.delivered_buttons+=1;",start)
        button=BRIDGE[start:end]
        self.assertIn("event.state()==ButtonState::Released",button)
        self.assertIn("state.window_interaction.end()",button)
        self.assertIn("pending.states.unset(xdg_toplevel::State::Resizing)",button)
        self.assertIn("top.send_pending_configure()",button)
        self.assertIn("!(state.managed_xdg_mode && state.dynamic_window_mode)",button)

if __name__=="__main__":
    unittest.main()
