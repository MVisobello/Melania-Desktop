#!/usr/bin/env python3
"""The first real GLES drawing test must stay offscreen and VM-only."""
from pathlib import Path
import unittest
ROOT=Path(__file__).resolve().parents[1]
RENDER=(ROOT/"melania-native/src/renderer.rs").read_text()
MAIN=(ROOT/"melania-native/src/main.rs").read_text()
CI=(ROOT/".github/workflows/linux-verify.yml").read_text()

class Stage6OffscreenContracts(unittest.TestCase):
    def test_actual_gbm_dma_buf_and_frame(self):
        for source in [
            'fn smoke_render_gbm_offscreen(fd: DrmDeviceFd)',
            'const SIZE: u32=64',
            'GbmDevice::new(fd)',
            'GbmAllocator::new(gbm.clone(),GbmBufferFlags::RENDERING)',
            'DmabufAllocator(',
            'allocator.create_buffer(SIZE,SIZE,Fourcc::Abgr8888,&[Modifier::Linear])',
            'renderer.bind(&mut buffer)',
            'renderer.render(&mut framebuffer,',
            'frame.clear(',
            'frame.finish()',
            '.wait()',
        ]: self.assertIn(source,RENDER)
    def test_display_manager_and_kms_remain_untouched(self):
        for forbidden in ('set_crtc(', 'create_surface(', 'page_flip(', 'scanout(', 'GbmBufferFlags::SCANOUT'):
            self.assertNotIn(forbidden,RENDER)
        self.assertIn('native_scanout_is_implemented() -> bool {false}',RENDER)
        self.assertIn('assert_dedicated_console();',MAIN)
        self.assertIn('experimental_drm(true)',MAIN)
        self.assertIn('MELANIA_EXPERIMENTAL_SEAT=1 cargo run -p melania-native -- --experimental-offscreen',CI)
        self.assertIn("grep -q 'NATIVE_STAGE1_BLOCKED' native-stage6-denied.log",CI)
        self.assertIn('NATIVE_SESSION_READY=false',MAIN)

if __name__=="__main__":
    unittest.main()
