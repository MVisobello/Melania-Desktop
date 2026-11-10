"""Regression: real QMP USB drag observers must outlive late frame 12.

The isolated VirtIO TCG guest can reach the dynamic two-client, 12-frame
stage after 1350s. This is not license to synthesize USB input or to skip the
13th real CRTC pageflip. The host input injector and both screenshot observers
must remain live in the full QEMU acceptance process.
"""
from pathlib import Path
import re
import unittest

ROOT=Path(__file__).resolve().parents[1]
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
INJECT=(ROOT/"scripts/inject-qemu-dynamic-pointer.py").read_text()

class DynamicPointerObserverBudget(unittest.TestCase):
    def test_all_three_qmp_observers_remain_live_past_late_tcg_frame12(self):
        paths=(
            "capture-qemu-continuous-dynamic.py",
            "inject-qemu-dynamic-pointer.py",
            "capture-qemu-dynamic-pointer.py",
        )
        for path in paths:
            match=re.search(
                rf'python3 "\$root/scripts/{re.escape(path)}"\s*'
                rf'(?P<args>.*?)> "\$report/qmp-[^"]+" 2>&1 &',
                HOST,re.DOTALL,
            )
            self.assertIsNotNone(match,path)
            budget=re.search(r"--timeout (\d+)",match.group("args"))
            self.assertIsNotNone(budget,path)
            self.assertGreaterEqual(int(budget.group(1)),1680,path)
        self.assertIn('timeout --signal=TERM --kill-after=10s 2050s',HOST)
    def test_real_qmp_input_and_crtc_pixel_proofs_are_not_optional(self):
        for marker in (
            'qmp-dynamic-pointer-input.log',
            'qmp-dynamic-pointer-frame13.log',
            'MELANIA_QEMU_DYNAMIC_LIBINPUT_DRAG_FRAME13=true',
            'MELANIA_QEMU_DYNAMIC_POINTER_FRAME13_PIXELS=true',
            'dynamic-pointer-frame13.ppm',
        ):
            self.assertIn(marker,HOST)
        self.assertIn('input-send-event',INJECT)
        self.assertIn('MELANIA_RUST_CONTINUOUS_DYNAMIC_POINTER_FRAME_VISIBLE=13',INJECT)
        self.assertIn('MELANIA_RUST_CONTINUOUS_DYNAMIC_POINTER_FRAME_VISIBLE=13',GUEST)
        self.assertIn('MELANIA_NATIVE_DYNAMIC_POINTER_CLIENT_STAYED_ALIVE=true',GUEST)
        self.assertIn("fatal 'different_sized_real_Wayland_clients_failed'",GUEST)

if __name__=="__main__":
    unittest.main()
