#!/usr/bin/env python3
"""Stage 30: wl_surface.frame callbacks only after genuine Rust KMS presentation.

Structural gates do not substitute for the guest test: QEMU must send three
real wl_callback.done responses after Rust DRM reports the CRTC frame, and
independently verify three distinct on-screen frames.
"""
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-client-window.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
KMS=(ROOT/"melania-native/src/bin/qemu-rust-kms-stream.rs").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()

class PresentedFrameStage30(unittest.TestCase):
    def test_callback_is_sent_only_after_KMS_ack(self):
        self.assertIn("MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS",BRIDGE)
        self.assertIn("with_surface_tree_downward(",BRIDGE)
        self.assertIn("frame_callbacks.drain(..)",BRIDGE)
        self.assertIn("callback.done(elapsed_ms)",BRIDGE)
        self.assertIn("MELANIA_NATIVE_FRAME_CALLBACKS_PRESENTED_PASS=true",BRIDGE)
        self.assertIn("MELANIA_NATIVE_FRAME_CALLBACK_COUNT={}",BRIDGE)
        # Buffer and frame callbacks must not be released on a failed KMS
        # handoff, and success must be reported only after the ACK.
        start=BRIDGE.index("let handoff:Result<(),Box<dyn Error>>")
        success=BRIDGE.index("match handoff {",start)
        post=BRIDGE.index("complete_presented_frame(",success)
        release=BRIDGE.index("buffer.release();",success)
        self.assertLess(success,post)
        self.assertLess(post,release)
        self.assertIn("stream.read_exact(&mut ack)",BRIDGE[start:success])
        self.assertIn("if ack[0]!=next",BRIDGE[start:success])
        self.assertIn("return;",BRIDGE[success:release])

    def test_client_requests_callbacks_before_each_commit(self):
        self.assertIn("wl_callback::WlCallback",CLIENT)
        self.assertIn("wl_callback::Event::Done{callback_data}",CLIENT)
        self.assertIn("state.frame_done_count=state.frame_done_count.saturating_add(1)",CLIENT)
        self.assertIn("MELANIA_NATIVE_WAIT_PRESENT_CALLBACK",CLIENT)
        request=CLIENT.index("Some(surface.frame(&qh,()))")
        commit=CLIENT.index("surface.commit();",request)
        wait=CLIENT.index("state.frame_done_count==frame_index as u32+1",commit)
        release=CLIENT.index("buffer.destroy();",wait)
        self.assertLess(request,commit)
        self.assertLess(commit,wait)
        self.assertLess(wait,release)
        self.assertIn("MELANIA_NATIVE_CLIENT_FRAME_CALLBACKS_PRESENTED_PASS=true",CLIENT)

    def test_vm_requires_real_three_frame_KMS_and_callback_proof(self):
        for fragment in (
            "MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS=YES_QEMU_VM",
            "MELANIA_NATIVE_WAIT_PRESENT_CALLBACK=YES_QEMU_VM",
            "MELANIA_NATIVE_FRAME_CALLBACK_COUNT=3",
            "MELANIA_NATIVE_FRAME_CALLBACKS_PRESENTED_PASS=true",
            "MELANIA_NATIVE_CLIENT_FRAME_CALLBACKS_PRESENTED_PASS=true",
            "MELANIA_QEMU_NATIVE_PRESENTED_FRAME_CALLBACKS_PASS=true",
            "MELANIA_VM_NATIVE_POST_KMS_FRAME_CALLBACKS_VERIFIED=true",
            "MELANIA_QEMU_LIVE_KMS_THREE_CAPTURE_PASS=true",
            "MELANIA_RUST_KMS_RESTORE_VERIFIED=true",
        ):
            self.assertIn(fragment,HOST+GUEST)
        self.assertIn("wait_for_fb(&card,crtc_handle,framebuffer)",KMS)
        self.assertIn("stream.write_all(&[index+1])",KMS)
        self.assertIn("test_qemu_presented_frame_stage30.py",CI)

    def test_qemu_remains_strictly_isolated_and_host_blocked(self):
        self.assertIn("accel=tcg",HOST)
        self.assertIn("-vga virtio",HOST)
        self.assertNotIn("vfio",HOST)
        self.assertIn("MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false",HOST)
        self.assertIn("MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false",GUEST)
        for path in ("scripts/melania-qemu-native-guest.sh",
                     "scripts/run-qemu-melania-native.sh"):
            run=subprocess.run(["bash","-n",str(ROOT/path)],
                capture_output=True,text=True,timeout=6)
            self.assertEqual(run.returncode,0,run.stderr)
        run=subprocess.run(["bash","scripts/run-qemu-melania-native.sh"],
            cwd=ROOT,env={"PATH":"/usr/bin:/bin"},
            capture_output=True,text=True,timeout=8)
        self.assertEqual(run.returncode,78,run.stderr)
        self.assertIn("MELANIA_QEMU_NATIVE_HOST_BLOCKED",run.stderr)

if __name__=="__main__":
    unittest.main()
