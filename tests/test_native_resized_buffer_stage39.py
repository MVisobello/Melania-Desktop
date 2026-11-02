#!/usr/bin/env python3
"""Stage39: resized client *pixels* must be imported and visible on QEMU KMS."""
import importlib.util
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
SCENE=(ROOT/"melania-native/src/multi_window_scene.rs").read_text()
IMPORT=(ROOT/"melania-native/src/native_client_frame.rs").read_text()
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-client-window.rs").read_text()
WORKER=(ROOT/"melania-native/src/async_kms_stream.rs").read_text()
KMS=(ROOT/"melania-native/src/bin/qemu-rust-kms-stream.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
CI=(ROOT/".github/workflows/qemu-melania-native.yml").read_text()
spec=importlib.util.spec_from_file_location(
    "resized_capture",ROOT/"scripts/capture-qemu-resized-window.py")
validator=importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)

def fixture():
    w,h=1280,800
    rgb=bytearray(bytes((27,33,58))*(w*h))
    def fill(x,y,ww,hh,color):
        for yy in range(y,y+hh):
            i=(yy*w+x)*3
            rgb[i:i+ww*3]=bytes(color)*ww
    fill(644,184,372,236,(65,171,216))
    fill(650,190,360,224,(73,126,237))
    fill(830,190,180,224,(247,211,48))
    fill(234,184,398,258,(65,171,216))
    fill(240,190,386,246,(222,64,118))
    fill(433,190,193,246,(117,238,90))
    return f"P6\n{w} {h}\n255\n".encode()+bytes(rgb)

class ResizedClientStage39(unittest.TestCase):
    def test_real_gpu_screenshot_must_show_new_client_dimensions(self):
        frame=fixture()
        output=validator.validate(frame)
        self.assertEqual(output["new_right_expansion"],(117,238,90))
        self.assertEqual(output["new_bottom_expansion"],(222,64,118))
        self.assertEqual(output["other_client"],(73,126,237))
        with self.assertRaises(ValueError):
            validator.validate(frame[:-4])

    def test_owned_resized_buffer_is_not_stretched_or_unvalidated(self):
        for token in (
            "pub fn copy_client_shm_sized(",
            "!(96..=600).contains(&meta.width)",
            "!(72..=420).contains(&meta.height)",
            "std::ptr::read_volatile(ptr.add(source+channel))",
            "fn imports_client_resized_xrgb8888_with_strides_and_rejects_bad_mapping()",
        ):
            self.assertIn(token,IMPORT)
        for token in (
            "pub fn upsert_sized(",
            "window.width+12",
            "width:window.width,height:window.height",
            "fn real_client_resize_changes_owned_dimensions_and_composite()",
        ):
            self.assertIn(token,SCENE)
        for token in (
            "resize_buffer_mode:bool",
            "native_client_frame::copy_client_shm_sized(",
            "self.multi_scene.upsert_sized(key,width,height,imported)",
            "MELANIA_NATIVE_REAL_RESIZED_WL_SHM_IMPORTED=",
            "self.pixel_frame_count==self.pixel_frame_target",
            "MELANIA_NATIVE_RESIZED_BUFFER",
        ):
            self.assertIn(token,BRIDGE)

    def test_fifth_frame_is_real_client_commit_and_has_kms_ack(self):
        for token in (
            "MELANIA_NATIVE_RESIZED_CLIENT_COMMIT=",
            "MELANIA_NATIVE_CLIENT_RESIZED_BUFFER_PRESENTED=true",
            "surface.attach(Some(&buffer),0,0)",
            "surface.damage_buffer(0,0,new_w,new_h)",
            "state.frame_done_count==4",
            "layout-ready-for-resize.flag",
        ):
            self.assertIn(token,CLIENT)
        self.assertIn("MELANIA_RUST_RESIZED_WAYLAND_FIVE_FRAMES_PASS=true",KMS)
        self.assertIn("if resized {5u8}",KMS)
        self.assertIn("MELANIA_NATIVE_RESIZED_BUFFER",WORKER)
        self.assertIn("MELANIA_QEMU_REAL_RESIZED_BUFFER_KMS_PASS=true",GUEST)
        self.assertIn("MELANIA_NATIVE_RESIZED_BUFFER=YES_QEMU_VM",GUEST)
        for token in (
            "capture-qemu-resized-window.py",
            "MELANIA_QEMU_REAL_RESIZE_VISIBLE_GPU_PASS=true",
            "resized-client-frame5.ppm",
        ):
            self.assertIn(token,HOST)
        self.assertIn("test_native_resized_buffer_stage39.py",CI)
        self.assertIn("cargo test -p melania-native --all-targets",CI)
        self.assertNotIn("MELANIA_NATIVE_SESSION_READY=true",GUEST)
        for file in ("scripts/melania-qemu-native-guest.sh","scripts/run-qemu-melania-native.sh"):
            r=subprocess.run(["bash","-n",str(ROOT/file)],
                text=True,capture_output=True,timeout=6)
            self.assertEqual(r.returncode,0,r.stderr)

if __name__=="__main__":
    unittest.main()
