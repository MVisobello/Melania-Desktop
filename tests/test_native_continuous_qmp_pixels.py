"""Stage47: actual QMP frame 7/12 pixel sampling is mandatory."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
CAPTURE=(ROOT/"scripts/capture-qemu-continuous-frames.py").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
RECEIVER=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()

class ContinuousQmpPixels(unittest.TestCase):
    def test_screenshot_must_come_from_private_live_qmp(self):
        for value in ("qmp_capabilities", '"screendump"',
                      "socket.AF_UNIX", "MELANIA_RUST_CONTINUOUS_FRAME_VISIBLE",
                      "for frame, output in ((7, args.seventh), (12, args.twelfth))",
                      "MELANIA_QEMU_CONTINUOUS_FRAME{frame}_PIXELS_VERIFIED=true",
                      "30 + (frame * 17) % 200", "40 + (frame * 31) % 190",
                      "25 + (frame * 43) % 210"):
            self.assertIn(value,CAPTURE)
        self.assertIn('tuple(map(int, match.groups())) != (W, H)',CAPTURE)
        self.assertIn('abs(got - want) > 18',CAPTURE)

    def test_qa_gate_requires_real_image_files_besides_kms_ack(self):
        self.assertIn('capture-qemu-continuous-frames.py',HOST)
        self.assertIn('continuous-frame7.ppm',HOST)
        self.assertIn('continuous-frame12.ppm',HOST)
        self.assertIn('MELANIA_QEMU_CONTINUOUS_FRAME7_PIXELS_VERIFIED=true',HOST)
        self.assertIn('MELANIA_QEMU_CONTINUOUS_FRAME12_PIXELS_VERIFIED=true',HOST)
        self.assertIn('continuous_capture_code',HOST)
        self.assertIn('tail -n +1 -f -s 0.05 "$continuous_kms_log" > /dev/ttyS0',GUEST)
        self.assertIn('if index==6 || index==11 {2500} else {350}',RECEIVER)
        self.assertIn('wait_for_fb(&card,crtc_handle,framebuffer)?',RECEIVER)

if __name__=="__main__":
    unittest.main()
