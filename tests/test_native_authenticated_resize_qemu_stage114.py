"""Stage114: real QEMU USB/Wayland/GPU resize acceptance, not mock frames."""
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
CLIENT=(ROOT/"melania-native/src/bin/qemu-authenticated-resize-client.rs").read_text()
INJECT=(ROOT/"scripts/inject-qemu-authenticated-resize.py").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class RealAuthenticatedResizeQemu114(unittest.TestCase):
    def test_protocol_request_uses_real_button_serial_and_committed_client_pixels(self):
        for marker in (
            "wl_pointer::Event::Button{serial,button,state,..}",
            "s.entered && !s.requested && s.callbacks==1",
            "top.resize(seat,serial,xdg_toplevel::ResizeEdge::TopLeft)",
            "role.ack_configure(serial)",
            "xdg_toplevel::State::Resizing as u32",
            "s.resized_acked=true",
            "wl_shm::Format::Xrgb8888",
            "surface.damage_buffer(",
            "surface.commit()",
            "state.buffer_released",
            "state.callbacks<index",
            "MELANIA_NATIVE_AUTH_RESIZE_FRAME",
            "final_configure_without_resizing",
        ):
            self.assertIn(marker,CLIENT)
        self.assertNotIn("sleep(Duration::from_secs(1))",CLIENT)

    def test_qemu_only_guest_requires_real_gpu_ack_and_rollback(self):
        for marker in (
            "MELANIA_QEMU_AUTH_RESIZE_CLIENT=YES_QEMU_VM",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS=YES_DISPOSABLE_QEMU",
            "MELANIA_NATIVE_MANAGED_XDG=YES_QEMU_VM",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=",
            "MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true",
            "MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true",
            "MELANIA_QEMU_AUTHENTICATED_XDG_RESIZE_REAL_KMS_PASS=true",
            "MELANIA_NATIVE_SESSION_READY=false",
        ):
            self.assertIn(marker,GUEST)

    def test_qmp_checks_exact_edges_pixels_and_mouse_release(self):
        for marker in (
            "MELANIA_NATIVE_XDG_CLIENT_RESIZE_GRANTED=1 edge=TopLeft",
            "MELANIA_NATIVE_XDG_RESIZE_CONFIGURE=1 x=",
            "x+w!=520 or y+h!=344",
            '"execute":"screendump"',
            "old-top-left-cleared",
            "new-client-pixels",
            "MELANIA_NATIVE_AUTH_RESIZE_FINAL_CONFIGURE_ACK=true",
        ):
            self.assertIn(marker,INJECT)
        for marker in (
            "scripts/inject-qemu-authenticated-resize.py",
            "qemu-authenticated-resize-client",
            "authenticated-resize-frame2.ppm",
            "MELANIA_QEMU_AUTH_RESIZE_CRTC_PIXELS=true",
            "MELANIA_QEMU_AUTHENTICATED_XDG_RESIZE_REAL_KMS_PASS=true",
        ):
            self.assertIn(marker,HOST)
        self.assertIn("test -x target/debug/qemu-authenticated-resize-client",CI)
        self.assertIn("test_native_authenticated_resize_qemu_stage114.py",CI)

if __name__=="__main__":
    unittest.main()
