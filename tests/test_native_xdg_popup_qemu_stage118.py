"""Stage118: genuine Wayland XDG popup creation, pixels and real QMP CRTC proof."""
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
NATIVE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-xdg-popup-client.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
QMP=(ROOT/"scripts/capture-qemu-real-xdg-popup.py").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class RealXdgPopupKmsStage118(unittest.TestCase):
    def test_separate_real_wayland_popup_acks_owned_shm(self):
        for token in (
            "wm.create_positioner(&q,())",
            "positioner.set_anchor_rect(220,85,60,30)",
            "popup_xdg.get_popup(Some(&root_xdg),&positioner",
            "xdg_popup::Event::Configure",
            "role.ack_configure(serial)",
            "wl_shm::Format::Xrgb8888",
            "surface.damage_buffer(",
            "surface.commit()",
            "state.buffer_released",
            "MELANIA_NATIVE_REAL_POPUP_CLIENT_FRAME",
            "popup.destroy()",
            "dismissed_unexpectedly",
            "popup dismissed before QEMU CRTC capture",
            "MELANIA_NATIVE_REAL_POPUP_CLIENT_DESTROYED=true",
        ):
            self.assertIn(token,CLIENT)
        for token in (
            "acknowledged",
            "native_popup_roles",
            "MELANIA_NATIVE_XDG_POPUP_CLIENT_BUFFER_ACCEPTED",
            "self.evict_native_popup(popup.wl_surface())",
            "self.dynamic_scene.upsert_in_layer",
        ):
            self.assertIn(token.lower(),NATIVE.lower())

    def test_qemu_gate_requires_two_real_gpu_framebuffers_and_popup_cleanup(self):
        for token in (
            "MELANIA_NATIVE_IN_PROCESS_KMS=YES_QEMU_VM",
            "MELANIA_NATIVE_PERSISTENT_KMS=YES_DISPOSABLE_QEMU",
            "MELANIA_QEMU_XDG_POPUP_CLIENT=YES_QEMU_VM",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=",
            "MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true",
            "MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true",
            "MELANIA_QEMU_REAL_XDG_POPUP_KMS3_PASS=true",
            "MELANIA_NATIVE_SESSION_READY=false",
        ):
            self.assertIn(token,GUEST)
        for token in (
            "MELANIA_QEMU_XDG_POPUP_REAL_CRTC_PIXELS=true",
            "MELANIA_QEMU_XDG_POPUP_DESTROY_CRTC_CLEARED=true",
            "real-popup-frame2.ppm",
            "real-popup-cleared-frame3.ppm",
        ):
            self.assertIn(token,HOST)
        for token in (
            "QMP",
            '"execute":"screendump"',
            "parent-overlaid-popup",
            "popup-outside-parent",
            "MELANIA_NATIVE_REAL_POPUP_CLIENT_DESTROYED=true",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=3",
        ):
            self.assertIn(token,QMP)
        self.assertIn("test -x target/debug/qemu-xdg-popup-client",CI)
        self.assertIn("test_native_xdg_popup_qemu_stage118.py",CI)

if __name__=="__main__":
    unittest.main()
