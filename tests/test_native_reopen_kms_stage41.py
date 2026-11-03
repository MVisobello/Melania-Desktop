"""Stage41: real Wayland client reopens into slot 1; sixth GPU frame proves it.

The QEMU test obtains a REAL screendump while the CRTC displays the
post-close reopened window. These local tests prevent false positives
from synthetic images or stale pixels and verify fail-closed gating.
"""
from pathlib import Path
import importlib.util
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-client-window.rs").read_text()
WORKER=(ROOT/"melania-native/src/async_kms_stream.rs").read_text()
KMS=(ROOT/"melania-native/src/bin/qemu-rust-kms-stream.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
VERIFY=ROOT/"scripts/capture-qemu-reopen-frame.py"


class NativeReopenKmsStage41(unittest.TestCase):
    def test_real_new_client_and_one_bounded_sixth_frame(self):
        for token in (
            "reopen_redraw_mode:bool",
            "reopen_frame_presented:bool",
            "fn submit_reopen_frame(&mut self,surface:&WlSurface,buffer:wl_buffer::WlBuffer)",
            "native_client_frame::copy_client_shm(ptr,len,metadata)",
            "self.known_window_surfaces.register(surface.clone())",
            'if key!=1 { return Err("QEMU reopened surface failed to reuse the freed slot".into()); }',
            "worker.submit(FrameJob{ticket,ordinal:6,pixels:frame})",
            "MELANIA_NATIVE_REOPEN_WINDOW_SLOT={key}",
            "MELANIA_NATIVE_REOPEN_FRAME_SUBMITTED=6",
            "MELANIA_NATIVE_REOPEN_FRAME_ACKED=6",
            "MELANIA_NATIVE_REOPEN_FRAME_VISIBLE=true",
            "self.reopen_frame_presented=true;",
            "if state.reopen_redraw_mode {5} else if state.close_redraw_mode {4}",
        ):
            self.assertIn(token,BRIDGE)
        for token in (
            "reopened=compositor.create_surface(&qh,())",
            "reopened_xdg=wm.get_xdg_surface(&reopened,&qh,())",
            "reopened_top=reopened_xdg.get_toplevel(&qh,())",
            "let _reopen_callback=reopened.frame(&qh,());",
            "MELANIA_NATIVE_CLIENT_REOPENED_XDG_COMMITTED=true",
            "while state.frame_done_count<5 || !state.buffer_released {",
            "MELANIA_NATIVE_CLIENT_REOPEN_CALLBACK_AFTER_KMS=true",
        ):
            self.assertIn(token,CLIENT)
        self.assertIn("!(1..=6).contains(&job.ordinal)",WORKER)
        self.assertIn("MELANIA_RUST_REOPEN_FRAME6_ACTIVE=true",KMS)
        self.assertIn("MELANIA_RUST_REOPEN_FRAME6_PASS=true",KMS)
        self.assertIn("MELANIA_NATIVE_REOPEN_REDRAW=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_QEMU_REOPEN_FRAME=YES_QEMU_VM",GUEST)
        self.assertIn("scripts/capture-qemu-reopen-frame.py",HOST)
        self.assertIn("MELANIA_VM_REAL_WINDOW_REOPEN_SIXTH_KMS_FRAME=true",HOST)
        self.assertIn("MELANIA_NATIVE_SESSION_READY=false",GUEST)
        self.assertNotIn("MELANIA_NATIVE_SESSION_READY=true",GUEST)

    def test_sixth_frame_validator_does_not_accept_closed_or_stale_first_client(self):
        spec=importlib.util.spec_from_file_location("reopen_kms",VERIFY)
        m=importlib.util.module_from_spec(spec)
        spec.loader.exec_module(m)
        w,h=m.W,m.H
        header=f"P6\n{w} {h}\n255\n".encode()
        pixels=bytearray([27,33,58])*(w*h)
        def pixel(x,y,colour):
            start=(y*w+x)*3
            pixels[start:start+3]=bytes(colour)
        points=(
            ((40,40),(27,33,58)),
            ((157,120),(65,171,216)),
            ((250,232),(190,80,160)),
            ((440,232),(245,175,55)),
            ((700,250),(27,33,58)),
            ((737,270),(65,171,216)),
            ((820,380),(73,126,237)),
            ((1000,380),(247,211,48)),
        )
        for (x,y),colour in points:
            pixel(x,y,colour)
        image=header+bytes(pixels)
        self.assertEqual(m.verify(image)["window_A_reopened_left"],(190,80,160))
        pixel(250,232,(27,33,58))
        with self.assertRaisesRegex(ValueError,"window_A_reopened_left"):
            m.verify(header+bytes(pixels))
        pixel(250,232,(245,108,34))
        with self.assertRaisesRegex(ValueError,"window_A_reopened_left"):
            m.verify(header+bytes(pixels))
        with self.assertRaises(ValueError):
            m.verify(image[:-4])


if __name__=="__main__":
    unittest.main()
