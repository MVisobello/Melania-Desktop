"""Non-executable, bounded GIO/GVfs text Quick Look regression guards."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
BACK=(ROOT/"melania-files/src/gio_backend.rs").read_text()
UI=(ROOT/"melania-files/src/gio_remote.rs").read_text()

class GVfsTextQuickLook(unittest.TestCase):
    def test_bounded_native_cancel_and_no_follow(self):
        for token in ("const MAX_QUICKLOOK_BYTES:usize=64*1024",
                      "pub fn text_quicklook(",
                      'gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS',
                      'info.file_type()!=gio::FileType::Regular',
                      'supported_text_mime(&mime)',
                      "with_gio_cancel(cancelled,|provider|",
                      "file.read(Some(provider))",
                      'stream.read(&mut block[..available],Some(provider))',
                      "bytes.contains(&0u8)"):
            self.assertIn(token,BACK)
        self.assertNotIn("std::process::Command",BACK)

    def test_read_only_gtk_preview_and_close_cancellation(self):
        for token in ('fn open_text_quicklook(remote:&Remote,uri:String)',
                      'view.set_editable(false)',
                      'view.set_cursor_visible(false)',
                      'view.set_monospace(true)',
                      'let result=gio_backend::text_quicklook(&uri,&worker_cancel)',
                      'abort.store(true,Ordering::Release)',
                      'quicklook.connect_clicked(move |_|{',
                      'key==gdk::Key::F3'):
            self.assertIn(token,UI)
        self.assertIn('64 KiB',UI)

    def test_real_local_and_sftp_cases(self):
        for name in ('text_quicklook_reads_only_bounded_utf8_and_refuses_binary',
                     'quicklook_is_cancelled_without_remote_provider_access',
                     'let preview=text_quicklook(&file.uri,&stop).unwrap();'):
            self.assertIn(name,BACK)

if __name__=='__main__':
    unittest.main()
