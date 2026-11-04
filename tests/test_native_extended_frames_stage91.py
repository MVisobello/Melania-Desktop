"""Stage91: extended native Wayland wl_shm and DRM ACK budgets (QEMU only)."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]


class NativeContinuousFrameBudget(unittest.TestCase):
    def test_parser_preserves_12_and_bounds_at_64(self):
        parser=(ROOT/"melania-native/src/native_frame_budget.rs").read_text()
        self.assertIn("pub const DEFAULT_QEMU_FRAMES:u64=12",parser)
        self.assertIn("pub const MAX_QEMU_FRAMES:u64=64",parser)
        self.assertIn("frame_budget(raw:Option<&str>,default:u64)",parser)
        self.assertIn("return Err(FrameBudgetError::InvalidValue)",parser)
        self.assertIn("fn longer_scenarios_within_receiver_capacity()",parser)

    def test_live_bridge_and_client_use_the_same_configured_budget(self):
        bridge=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
        client=(ROOT/"melania-native/src/bin/qemu-continuous-client.rs").read_text()
        receiver=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
        self.assertIn('MELANIA_NATIVE_QEMU_FRAME_TARGET',bridge)
        self.assertIn("native_frame_budget::frame_budget(",bridge)
        self.assertIn('MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES',client)
        self.assertIn("for index in 1..=total",client)
        self.assertIn("MELANIA_NATIVE_CONTINUOUS_CLIENT_12_FRAMES_PASS=true",client)
        self.assertIn("MELANIA_NATIVE_CONTINUOUS_CLIENT_EXTENDED_FRAMES_PASS={total}",client)
        self.assertIn('MELANIA_QEMU_CONTINUOUS_FRAMES',receiver)
        self.assertIn("native_frame_budget::receiver_frame_budget(",receiver)
        self.assertIn("pub fn receiver_frame_budget(",(ROOT/"melania-native/src/native_frame_budget.rs").read_text())
        self.assertIn('extended native frames require plain QEMU continuous KMS',bridge)

    def test_no_native_login_or_host_gpu_opt_out(self):
        bridge=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
        entry=(ROOT/"melania-native/src/main.rs").read_text()
        self.assertIn("state.native_presentation_guard.permanently_revoked()",bridge)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",bridge)
        self.assertNotIn("MELANIA_NATIVE_SESSION_READY=true",bridge)
        self.assertIn("assert_qemu_test_guest();",entry)
        self.assertIn("assert_dedicated_console();",entry)


if __name__=="__main__":
    unittest.main()
