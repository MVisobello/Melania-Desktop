"""Image Quick Look must be bounded, non-executing and tested over real GVfs SFTP.

These structural checks supplement Rust/GTK and live SFTP integration tests.
"""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BACK=(ROOT/"melania-files/src/gio_backend.rs").read_text()
UI=(ROOT/"melania-files/src/gio_remote.rs").read_text()
SFTP=(ROOT/"scripts/smoke-gvfs-sftp-loopback.sh").read_text()
CARGO=(ROOT/"melania-files/Cargo.toml").read_text()

class ImagePreviewGuards(unittest.TestCase):
    def test_no_unbounded_provider_read(self):
        for anchor in ("MAX_IMAGE_TRANSFER_BYTES:usize=6*1024*1024",
                       "MAX_IMAGE_DIMENSION:u32=2048",
                       "MAX_IMAGE_PIXELS:u64=4_000_000",
                       "gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS",
                       'info.file_type()!=gio::FileType::Regular',
                       "with_gio_cancel(cancelled,|provider|",
                       "stream.read(&mut chunk[..available],Some(provider))",
                       "bytes.len()>MAX_IMAGE_TRANSFER_BYTES",
                       "width>MAX_IMAGE_DIMENSION",
                       "u64::from(width)*u64::from(height)>MAX_IMAGE_PIXELS"):
            self.assertIn(anchor,BACK)
        self.assertIn('default-features = false',CARGO)
        self.assertNotIn("std::process::Command",BACK)

    def test_worker_decodes_before_ui(self):
        for anchor in ("ImageReader::with_format(", "reader.limits(limits)",
                       "decoded.thumbnail(IMAGE_PREVIEW_WIDTH,IMAGE_PREVIEW_HEIGHT)",
                       "open_image_quicklook(remote:&Remote,uri:String)",
                       "std::thread::spawn(move ||{\n        let result=gio_backend::image_quicklook(",
                       "gdk::MemoryTexture::new(", "gdk::MemoryFormat::R8g8b8a8",
                       "picture.set_paintable(Some(&texture))",
                       "quicklook.connect_clicked(move |_|{"):
            self.assertIn(anchor,BACK+"\n"+UI)
        self.assertIn("fn open_text_quicklook(",UI)
        self.assertIn("key==gdk::Key::F3",UI)

    def test_real_provider_and_local_safety_fixtures(self):
        self.assertIn("fn image_preview_scales_png_and_refuses_non_images_and_large_dimensions()",BACK)
        self.assertIn("fn image_preview_cancels_before_contacting_remote_provider()",BACK)
        self.assertIn("let image=image_quicklook(&png,&stop).unwrap();",BACK)
        self.assertIn("preview-test.png",SFTP)

if __name__=="__main__":
    unittest.main()
