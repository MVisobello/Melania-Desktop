"""Stage125: real USB Escape -> Smithay XDG popup_done -> kernel CRTC clearance.

Mock only QMP command transport in the unit test. The actual frame2/frame3
kernel/CRTC evidence remains mandatory in the full QEMU workflow.
"""
import json
import runpy
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[1]
POLICY=(ROOT/"melania-native/src/native_popup_layout.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-xdg-popup-client.rs").read_text()
QMP=(ROOT/"scripts/capture-qemu-real-xdg-popup.py").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()

class _FakeIo:
    def __init__(self):
        self.sent=[]
        self.answers=iter([b'{"QMP":{"version":{}}}\n',
                           b'{"return":{}}\n',b'{"return":{}}\n'])
    def readline(self):
        return next(self.answers)
    def write(self,blob):
        self.sent.append(json.loads(blob))
class _FakeSocket:
    def __init__(self):
        self.io=_FakeIo()
    def __enter__(self):
        return self
    def __exit__(self,*_):
        pass
    def settimeout(self,_):
        pass
    def connect(self,_):
        pass
    def makefile(self,*_,**__):
        return self.io

class RealUsbPopupEscapeStage125(unittest.TestCase):
    def test_input_scope_and_real_wayland_protocol_are_required(self):
        for token in ("escape_popup_target(","visited.contains(child)",
                      "hidden_or_unacknowledged_roles_never_intercept_escape"):
            self.assertIn(token,POLICY)
        for token in ("checked_sub(8)==Some(1)",
                      "event.state()==KeyState::Released",
                      "state.popup_escape_held=true",
                      "self.lifecycle.seat_active()",
                      "popup.send_popup_done()",
                      "self.evict_native_popup(popup.wl_surface())",
                      "MELANIA_NATIVE_XDG_POPUP_ESCAPE_DISMISSED="):
            self.assertIn(token,BRIDGE)
        for token in ("xdg_popup::Event::PopupDone",
                      "MELANIA_NATIVE_REAL_POPUP_CLIENT_POPUP_DONE_AFTER_ESCAPE=true",
                      "while !state.popup_done_after_frame2",
                      "popup.destroy()"):
            self.assertIn(token,CLIENT)

    def test_actual_crtc_pixels_must_precede_usb_escape_and_cleanup(self):
        shown=QMP.index("capture_until(args.qmp,args.popup,True")
        pressed=QMP.index('qmp_key(args.qmp,"esc",True)')
        dismissed=QMP.index("MELANIA_NATIVE_REAL_POPUP_CLIENT_POPUP_DONE_AFTER_ESCAPE=true")
        cleared=QMP.index("capture_until(args.qmp,args.cleared,False")
        self.assertLess(shown,pressed)
        self.assertLess(pressed,dismissed)
        self.assertLess(dismissed,cleared)
        for token in ("MELANIA_NATIVE_REAL_POPUP_CLIENT_POPUP_DONE_AFTER_ESCAPE=true",
                      "MELANIA_NATIVE_XDG_POPUP_ESCAPE_DISMISSED=2",
                      "MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true",
                      "MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true"):
            self.assertIn(token,GUEST)
        self.assertIn("MELANIA_QEMU_XDG_POPUP_USB_ESC_POPUP_DONE=true",HOST)
        self.assertIn("MELANIA_QEMU_XDG_POPUP_DESTROY_CRTC_CLEARED=true",HOST)

    def test_minimize_and_parent_close_evict_nested_popups_atomically(self):
        # The real bridge must hide the toplevel *before* repairing focus,
        # remove all child roles, then submit a single KMS scene update.
        self.assertIn("pub fn popup_descendant_teardown_order",POLICY)
        self.assertIn("parent_minimize_clears_all_nested_menus",POLICY)
        lifecycle=BRIDGE[BRIDGE.index("fn minimize_request(&mut self"): ]
        minimize=lifecycle[:lifecycle.index("fn toplevel_destroyed(")]
        close=lifecycle[lifecycle.index("fn toplevel_destroyed("):
                        lifecycle.index("fn grab(&mut self")]
        self.assertLess(minimize.index("self.dynamic_scene.set_visible(key,false)"),
                        minimize.index("self.evict_native_popup_tree(&surface,true,true)"))
        self.assertLess(minimize.index("self.evict_native_popup_tree(&surface,true,true)"),
                        minimize.index("self.submit_continuous_frame()"))
        self.assertIn("scene_changed|=self.evict_native_popup_tree(&surface,true,true)",minimize)
        self.assertIn("if removed || popup_damage {",close)
        self.assertIn("self.note_scene_change();",close)
        self.assertIn("let popup_damage=self.evict_native_popup_tree(&surface,true,true)",close)
        self.assertIn("if self.continuous_kms_mode &&",close)
        self.assertLess(close.index("let popup_damage=self.evict_native_popup_tree("),
                        close.index("self.note_scene_change();"))
        self.assertNotIn("for popup in descendants",close)
        cleanup=BRIDGE[BRIDGE.index("fn evict_native_popup_tree("):
                       BRIDGE.index("/// Esc from the real active libinput seat")]
        self.assertIn("popup_descendant_teardown_order(",cleanup)
        self.assertIn("if scene_changed && !defer_redraw",cleanup)
        self.assertLess(cleanup.index("self.native_popup_roles.retain("),
                        cleanup.index("keyboard.set_focus(self,replacement.clone()"))

    def test_minimize_revokes_only_own_input_and_old_press_serial(self):
        lifecycle=BRIDGE[BRIDGE.index("fn minimize_request(&mut self"): ]
        minimize=lifecycle[:lifecycle.index("fn toplevel_destroyed(")]
        self.assertIn("self.cancel_surface_grab(&surface);",minimize)
        self.assertNotIn("self.window_interaction.cancel();",minimize)
        self.assertIn("self.last_xdg_press=None;",minimize)
        self.assertLess(minimize.index("if !model.minimize(){return;}"),
                        minimize.index("self.cancel_surface_grab(&surface);"))
        self.assertLess(minimize.index("self.last_xdg_press=None;"),
                        minimize.index("self.evict_native_popup_tree(&surface,true,true)"))
        # Rust's actual pointer-grab policy must retain a foreign active grab.
        interaction=(ROOT/"melania-native/src/window_interaction.rs").read_text()
        self.assertIn("fn role_transition_cancels_only_matching_grab()",interaction)
        self.assertIn("self.window_interaction.cancel_for_key(key)",BRIDGE)

    def test_close_redraws_orphan_popup_even_without_parent_scene_slot(self):
        lifecycle=BRIDGE[BRIDGE.index("fn toplevel_destroyed(&mut self"):]
        close=lifecycle[:lifecycle.index("fn grab(&mut self")]
        self.assertIn("let parent_key=self.known_window_surfaces.remove(&surface);",close)
        self.assertIn("} else {false};",close)
        self.assertIn("if removed || popup_damage {",close)
        self.assertLess(close.index("let popup_damage=self.evict_native_popup_tree("),
                        close.index("let parent_key=self.known_window_surfaces.remove("))
        self.assertLess(close.index("if removed || popup_damage {"),
                        close.index("if let Some(key)=parent_key {",close.index("if removed || popup_damage {")))
        self.assertLess(close.index("self.note_scene_change();"),
                        close.index("self.submit_continuous_frame()"))
        self.assertEqual(close.count("self.submit_continuous_frame()"),1)

    def test_qmp_emits_actual_usb_key_press_and_release(self):
        fn=runpy.run_path(str(ROOT/"scripts/capture-qemu-real-xdg-popup.py"))["qmp_key"]
        sockets=[]
        def make_socket(*_):
            sock=_FakeSocket()
            sockets.append(sock)
            return sock
        fake=SimpleNamespace(AF_UNIX=1,SOCK_STREAM=1,socket=make_socket)
        with patch.dict(fn.__globals__,{"socket":fake}):
            fn(Path("/no-actual-QMP"),"esc",True)
            fn(Path("/no-actual-QMP"),"esc",False)
        self.assertEqual(len(sockets),2)
        for down,sock in zip((True,False),sockets):
            self.assertEqual(sock.io.sent[0]["execute"],"qmp_capabilities")
            event=sock.io.sent[1]
            self.assertEqual(event["execute"],"input-send-event")
            payload=event["arguments"]["events"][0]["data"]
            self.assertEqual(payload["key"],{"type":"qcode","data":"esc"})
            self.assertEqual(payload["down"],down)

if __name__=="__main__":
    unittest.main()
