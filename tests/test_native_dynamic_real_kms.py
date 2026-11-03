"""Stage53: two DIFFERENT real Wayland sizes must reach a QEMU CRTC."""
from pathlib import Path
import importlib.util
import unittest

ROOT=Path(__file__).resolve().parents[1]
BRIDGE=(ROOT/"melania-native/src/wayland_bridge.rs").read_text()
CLIENT=(ROOT/"melania-native/src/bin/qemu-continuous-two-windows.rs").read_text()
RECEIVER=(ROOT/"melania-native/src/bin/qemu-rust-kms-continuous.rs").read_text()
SCENE=(ROOT/"melania-native/src/dynamic_window_scene.rs").read_text()
GUEST=(ROOT/"scripts/melania-qemu-native-guest.sh").read_text()
HOST=(ROOT/"scripts/run-qemu-melania-native.sh").read_text()
QMP_FILE=ROOT/"scripts/capture-qemu-continuous-dynamic.py"
spec=importlib.util.spec_from_file_location("dynamic_kms_capture",QMP_FILE)
CAPTURE=importlib.util.module_from_spec(spec)
spec.loader.exec_module(CAPTURE)

class RealDynamicKms(unittest.TestCase):
    def test_real_veneer_imports_client_pixels_only_inside_validated_callback(self):
        for marker in (
            'MELANIA_NATIVE_DYNAMIC_SCENE',
            'state.dynamic_window_mode',
            'continuous KMS requires a dedicated QEMU probe',
            'dynamic KMS requires dedicated two-window QEMU proof',
            'self.dynamic_scene.composite()',
            'self.dynamic_scene.upsert(key,x,y,pixels)',
            'self.dynamic_scene.remove(key)',
            'self.dynamic_scene.hit_test(point.x,point.y)',
            'self.dynamic_scene.position(key)',
            'melania_native::owned_shm::copy_bounded_client_shm(ptr,len,metadata)',
            'WindowPixels::from_owned_shm(image)',
            'self.continuous_client_ledger.acknowledged(clients)',
            'self.continuous_redraw.presented(planned)',
        ):
            self.assertIn(marker,BRIDGE)
        self.assertIn("pub fn from_owned_shm",SCENE)
        self.assertIn("AlphaType::Premultiplied=>DynamicAlpha::Premultiplied",SCENE)
        self.assertIn("native_client_frame::copy_client_shm(ptr,len,metadata)",BRIDGE)
        self.assertIn("self.multi_scene.upsert(key,pixels)",BRIDGE)

    def test_two_differently_sized_real_xdg_client_buffers(self):
        for marker in (
            "MELANIA_QEMU_CONTINUOUS_DYNAMIC_CLIENT",
            "(320usize,192usize)", "(220usize,180usize)",
            "MELANIA_NATIVE_CONTINUOUS_DYNAMIC_SIZES=320x192;220x180",
            "MELANIA_NATIVE_CONTINUOUS_DYNAMIC_SHRINK=400x240->320x192;280x200->220x180",
            "(400usize,240usize)", "(280usize,200usize)",
            "if index<7", "index<8",
            "continuous-dynamic-client-{index}.bgra",
            "for index in 1..=12u32",
            "surface.damage_buffer(0,0,client_w as i32,client_h as i32)",
            "MELANIA_NATIVE_DYNAMIC_TWO_SIZES_12_KMS_FRAMES_PASS=true",
            "MELANIA_NATIVE_DYNAMIC_PREMULTIPLIED_ARGB_CLIENT_PASS=true",
            "wl_shm::Format::Argb8888",
            "premul(blue),premul(green),premul(red),128"
        ):
            self.assertIn(marker,CLIENT)

    def test_qemu_demands_real_fb_and_restored_crtc(self):
        self.assertIn("MELANIA_RUST_CONTINUOUS_DYNAMIC_FRAME_VISIBLE=",RECEIVER)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_DYNAMIC=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_NATIVE_DYNAMIC_SCENE=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_DYNAMIC_CLIENT=YES_QEMU_VM",GUEST)
        self.assertIn("MELANIA_RUST_KMS_RESTORE_VERIFIED=true",GUEST)
        self.assertIn("MELANIA_NATIVE_CONTINUOUS_WAYLAND_KMS_FRAMES_PASS=12",GUEST)
        self.assertIn("MELANIA_QEMU_NATIVE_DYNAMIC_TWO_SIZES_PASS=true",HOST)
        self.assertIn("MELANIA_QEMU_CONTINUOUS_DYNAMIC_PIXELS_VERIFIED=true",HOST)
        self.assertIn("dynamic-window-frame12.ppm",HOST)
        self.assertIn("MELANIA_VM_NATIVE_VARIABLE_CLIENT_SIZES_GPU_QMP=true",HOST)

    def test_actual_qmp_sample_requires_both_interiors_and_exact_edges(self):
        width,height=CAPTURE.W,CAPTURE.H
        data=bytearray(bytes([27,33,58])*(width*height))
        for x0,y0,w,h,index in ((160,120,320,192,11),(650,190,220,180,12)):
            pixel=bytes(CAPTURE.blended_b(index) if index==12 else CAPTURE.color(index))
            for y in range(y0,y0+h):
                start=(y*width+x0)*3
                data[start:start+w*3]=pixel*w
        header=f"P6\n{width} {height}\n255\n".encode()
        CAPTURE.verify(header+data)
        # Changing only one boundary pixel makes the real QMP proof fail.
        outside=(200*width+480)*3
        data[outside:outside+3]=bytes(CAPTURE.color(11))
        self.assertRaises(ValueError,CAPTURE.verify,header+data)

if __name__=="__main__":
    unittest.main()
