"""Stage90: native scroll must use physical libinput and wl_pointer.axis."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]


class NativeScrollWayland(unittest.TestCase):
    def test_scroll_is_not_swallowed_by_the_qemu_bridge(self):
        source=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
        self.assertIn("InputEvent::PointerAxis {event} => {",source)
        self.assertIn("PointerAxisEvent, Axis, AxisSource",source)
        self.assertIn("AxisFrame::new(event.time_msec()).source(source)",source)
        self.assertIn("event.amount(Axis",source.replace("event.amount(axis)","event.amount(Axis"))
        self.assertIn("event.amount_v120(axis)",source)
        self.assertIn("frame=frame.value(axis,value)",source)
        self.assertIn("frame=frame.v120(axis,steps);",source)
        self.assertIn("pointer.axis(state,frame);",source)
        self.assertIn("pointer.frame(state);",source)

    def test_scroll_obeys_epoch_and_does_not_expose_user_input(self):
        source=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
        self.assertIn("state.native_presentation_guard",source)
        self.assertIn("No physical input reaches a Wayland client after a VT/seat pause",source)
        self.assertIn("state.lifecycle.input_registry_mut()",source)
        self.assertIn("let Some(target)=state.pointer_target(point) else {return;};",source)
        self.assertIn("bounded_v120",source)
        self.assertIn("v.is_finite()",source)
        self.assertIn("state.delivered_scroll_frames=state.delivered_scroll_frames.saturating_add(1);",source)
        self.assertNotIn("MELANIA_NATIVE_SCROLL_TEXT",source)
        self.assertNotIn("NATIVE_SESSION_READY=true",source)


if __name__=="__main__":
    unittest.main()
