#!/usr/bin/env python3
"""Stage 29: bounded live Smithay -> Rust KMS stream inside disposable QEMU.

Static tests prevent false readiness; the VM must prove 3 REAL client
commits, 3 independently captured QMP images and verified CRTC restore.
"""
import importlib.util
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-client-window.rs").read_text()
KMS=(ROOT/"melania-native/src/bin/qemu-rust-kms-stream.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
spec=importlib.util.spec_from_file_location(
    "capture_live", ROOT/"scripts/capture-qemu-live-kms-stream.py"
)
capture=importlib.util.module_from_spec(spec)
spec.loader.exec_module(capture)

def ppm(frame:int)->bytes:
    w,h=1280,800
    image=bytearray(bytes((27,33,58))*(w*h))
    def pixel(x,y,color):
        p=(y*w+x)*3
        image[p:p+3]=bytes(color)
    pixel(157,120,(65,171,216))
    colors={
        1:((245,108,34),(16,205,155)),
        2:((73,126,237),(247,211,48)),
        3:((222,64,118),(117,238,90)),
    }
    pixel(250,232,colors[frame][0])
    pixel(430,232,colors[frame][1])
    return f"P6\n{w} {h}\n255\n".encode()+image

class NativeLiveStage29(unittest.TestCase):
    def test_three_real_client_frames_are_distinct_and_validated(self):
        images=[ppm(i) for i in (1,2,3)]
        for i,img in enumerate(images,1):
            self.assertEqual(capture.validate(img,i)["left"],
                             {1:(245,108,34),2:(73,126,237),3:(222,64,118)}[i])
            with self.assertRaises(ValueError):
                capture.validate(img,1 if i==3 else i+1)
        self.assertEqual(len(set(images)),3)
        with self.assertRaises(ValueError):
            capture.validate(b"P6\n1 1\n255\n\0\0\0",1)

    def test_smithay_is_live_and_releases_only_after_KMS_ack(self):
        for fragment in (
            "MELANIA_NATIVE_LIVE_STREAM",
            "UnixListener::bind(path)",
            "listener.accept()",
            "stream.write_all(&length.to_le_bytes())",
            "stream.write_all(&frame)",
            "stream.read_exact(&mut ack)",
            "if ack[0]!=next",
            "MELANIA_NATIVE_LIVE_KMS_FRAME_ACK=",
            "MELANIA_NATIVE_KMS_STREAM_THREE_FRAMES=true",
            "pixel_frame_target:if std::env::var",
        ):
            self.assertIn(fragment,BRIDGE)
        # The legacy synchronous probe still releases only after its ACK.
        # The new asynchronous branch has its own earlier *pending* buffer
        # release, which is guarded independently by Stage34 tests.
        read_at=BRIDGE.index("stream.read_exact(&mut ack)")
        release_at=BRIDGE.index("                buffer.release();",read_at)
        self.assertLess(read_at,release_at)
        self.assertIn('MELANIA_NATIVE_THREE_FRAMES',CLIENT)
        self.assertIn('for frame_index in 0..count',CLIENT)
        self.assertIn('MELANIA_NATIVE_CLIENT_FRAME_COMMITTED={}',CLIENT)
        self.assertNotIn("MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=true",BRIDGE)

    def test_Rust_drm_stream_sends_ack_only_after_visible_frame(self):
        for needle in (
            'MELANIA_QEMU_RUST_LIVE_STREAM_KMS',
            'Path::new("/dev/tty1")',
            '"/sys/class/dmi/id/sys_vendor"',
            '"/run/melania-native-qemu/frame-stream.sock"',
            'card.create_dumb_buffer(',
            'card.map_dumb_buffer(',
            'card.add_framebuffer(',
            'card.set_crtc(',
            'card.page_flip(',
            'wait_for_fb(',
            'stream.read_exact(&mut pixels)',
            'thread::sleep(Duration::from_secs(5))',
            'MELANIA_RUST_STREAM_FRAME{}_ACTIVE=true',
            'MELANIA_RUST_STREAM_THREE_CLIENT_FRAMES_PASS=true',
            'MELANIA_NATIVE_SESSION_READY=false',
            'MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false',
        ):
            self.assertIn(needle,KMS)
        self.assertLess(KMS.index("wait_for_fb(&card,crtc_handle,framebuffer)"),
                        KMS.index("stream.write_all(&[index+1])"))
        self.assertLess(KMS.index("rollback.restore()?;"),
                        KMS.index("stream.write_all(&[index+1])"))
        self.assertIn("impl Drop for Restore",KMS)

    def test_host_requires_vm_QMP_three_frames_and_original_restore(self):
        for marker in (
            "MELANIA_NATIVE_LIVE_STREAM=YES_QEMU_VM",
            "MELANIA_NATIVE_THREE_FRAMES=YES_QEMU_VM",
            "MELANIA_QEMU_RUST_LIVE_STREAM_KMS=YES_DISPOSABLE_QEMU",
            "MELANIA_QEMU_NATIVE_LIVE_WAYLAND_KMS_STREAM_PASS=true",
            "MELANIA_QEMU_LIVE_KMS_THREE_CAPTURE_PASS=true",
            "MELANIA_QEMU_LIVE_KMS_FRAME1_PIXELS=true",
            "MELANIA_QEMU_LIVE_KMS_FRAME2_PIXELS=true",
            "MELANIA_QEMU_LIVE_KMS_FRAME3_PIXELS=true",
            "MELANIA_RUST_KMS_RESTORE_VERIFIED=true",
            "MELANIA_VM_LIVE_WAYLAND_STREAM_THREE_FRAMES=true",
        ):
            self.assertIn(marker,HOST+GUEST)
        self.assertIn("accel=tcg",HOST)
        self.assertIn("-vga virtio",HOST)
        self.assertNotIn("vfio",HOST)
        self.assertIn("MELANIA_NATIVE_KMS_ROLLBACK_TESTED=false",HOST)
        self.assertIn("qemu-rust-kms-stream",CI)
        self.assertIn("test_qemu_live_kms_stage29.py",CI)

    def test_fail_closed_and_shell_syntax(self):
        for path in ("scripts/run-qemu-melania-native.sh",
                     "scripts/melania-qemu-native-guest.sh"):
            p=subprocess.run(["bash","-n",str(ROOT/path)],
                capture_output=True,text=True,timeout=8)
            self.assertEqual(p.returncode,0,p.stderr)
        p=subprocess.run(["bash","scripts/run-qemu-melania-native.sh"],
            cwd=ROOT,env={"PATH":"/usr/bin:/bin"},
            capture_output=True,text=True,timeout=8)
        self.assertEqual(p.returncode,78,p.stderr)
        self.assertIn("MELANIA_QEMU_NATIVE_HOST_BLOCKED",p.stderr)

if __name__=="__main__":
    unittest.main()
