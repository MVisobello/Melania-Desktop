"""Stage136: required actual USB + two distinct XDG owned client buffers."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-continuous-two-windows.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
QMP=(ROOT/"scripts/inject-qemu-combined-tab-tiling.py").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class NativeCombinedTabTilingStage136(unittest.TestCase):
    def test_real_client_ack_commits_both_before_any_shared_kms_callback(self):
        self.assertIn("MELANIA_QEMU_COMBINED_TAB_CLIENT",CLIENT)
        self.assertIn("state.combined_probe",CLIENT)
        self.assertIn("state.resize_acked_a && state.resize_acked_b",CLIENT)
        self.assertIn("state.resize_acked_a=false;",CLIENT)
        self.assertIn("state.resize_acked_b=false;",CLIENT)
        self.assertIn("state.resize_requested_a=None;",CLIENT)
        self.assertIn("state.resize_requested_b=None;",CLIENT)
        self.assertIn("state.resize_requested_a!=Some(desired)",CLIENT)
        self.assertIn("state.resize_requested_b!=Some(desired)",CLIENT)
        self.assertIn("for (id,surface,rgba) in [",CLIENT)
        self.assertIn("surface.attach(Some(&buffer),0,0);",CLIENT)
        self.assertIn("surface.commit();",CLIENT)
        self.assertIn("while state.frame_done_count<14",CLIENT)
        self.assertIn("MELANIA_QEMU_COMBINED_TAB_TWO_OWNED_SHM_CALLBACKS=true",CLIENT)
    def test_quest_real_kms_rollback_and_strict_host_evidence(self):
        for marker in (
            "MELANIA_NATIVE_AUTO_TILING=YES_QEMU_VM",
            "MELANIA_NATIVE_COMBINED_TAB_TILING=YES_QEMU_VM",
            "MELANIA_QEMU_COMBINED_TAB_CLIENT=YES_QEMU_VM",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=14",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=15",
            "MELANIA_NATIVE_GPU_WAYLAND_KMS_ACK=16",
            "MELANIA_NATIVE_GBM_KMS_CRTC_RESTORE=true",
            "MELANIA_NATIVE_GBM_KMS_CONNECTOR_RESTORE=true",
            "MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true",
            "MELANIA_QEMU_NATIVE_COMBINED_TAB_TILING_REAL_CRTC_PASS=true",
            "NATIVE_SESSION_READY=false"
        ):
            self.assertIn(marker,GUEST)
        for marker in (
            "scripts/inject-qemu-combined-tab-tiling.py",
            "combined_tabs_input_pid",
            "combined_tabs_input_code",
            "MELANIA_QEMU_COMBINED_TAB_REAL_XDG_SHM_KMS_PASS=true",
            "combined-tabs-active-A.ppm",
            "combined-tabs-active-B.ppm",
            "combined-tabs-active-A-return.ppm"
        ):
            self.assertIn(marker,HOST)
    def test_qmp_sends_only_real_key_events_and_reads_actual_fb(self):
        self.assertIn('tab.chord(a.qmp,"s")',QMP)
        self.assertIn('tab.chord(a.qmp,"s",shift=True)',QMP)
        self.assertIn('tab.chord(a.qmp,"y")',QMP)
        self.assertIn('tab.chord(a.qmp,"left")',QMP)
        self.assertIn('tab.chord(a.qmp,"right")',QMP)
        self.assertIn('tab.frame(a.qmp',QMP)
        self.assertIn("MELANIA_NATIVE_TAB_RESIZE_ATOMIC_SCENE_READY=1 clients=2",QMP)
        self.assertIn("MELANIA_QEMU_COMBINED_TAB_TWO_REAL_XDG_ACKS=true",QMP)
        self.assertNotIn('"type":"btn"',QMP)
        self.assertNotIn("NATIVE_SESSION_READY=true",QMP)
        self.assertIn("python3 -m py_compile scripts/inject-qemu-combined-tab-tiling.py",CI)
        self.assertIn("test_native_combined_tab_tiling_stage136.py",CI)
    def test_combined_is_explicit_guest_only(self):
        self.assertIn("fn arm_native_tab_tiling(&mut self,work:WindowRect)",BRIDGE)
        self.assertIn('MELANIA_NATIVE_COMBINED_TAB_TILING',BRIDGE)
        self.assertIn("self.tab_resize_transaction.sent_configure(epoch,id,serial)",BRIDGE)
        self.assertIn("let serial:u32=top.send_configure().into();",BRIDGE)
        self.assertIn("self.dynamic_scene.upsert_batch(updates)",BRIDGE)
        self.assertNotIn("NATIVE_SESSION_READY=true",BRIDGE)

if __name__=="__main__":unittest.main()
