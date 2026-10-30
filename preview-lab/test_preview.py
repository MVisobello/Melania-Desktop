#!/usr/bin/env python3
"""Tests for the actual file operations of the GTK-independent UX preview."""
import pathlib
import sys
import tempfile
import unittest
sys.path.insert(0, str(pathlib.Path(__file__).parent))
from melania_preview import safe_copy_file, make_demo_dir

class FileOperationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='melania-test-')
        self.addCleanup(self.tmp.cleanup)
        self.base = pathlib.Path(self.tmp.name)
        make_demo_dir(self.base)

    def test_copy_has_identical_bytes(self):
        origin = self.base/'Home/Leggimi.txt'
        dest = safe_copy_file(origin, self.base/'Home/Progetti')
        self.assertEqual(dest.read_bytes(), origin.read_bytes())
        self.assertTrue(origin.is_file())

    def test_never_overwrites_existing_file(self):
        src = self.base/'Home/Leggimi.txt'
        dst = safe_copy_file(src, self.base/'Home/Progetti')
        dst.write_text('Non sovrascrivere', encoding='utf-8')
        with self.assertRaises(FileExistsError):
            safe_copy_file(src, self.base/'Home/Progetti')
        self.assertEqual(dst.read_text(), 'Non sovrascrivere')

    def test_refuses_symlink_sources(self):
        src=self.base/'Home/Leggimi.txt'; link=self.base/'Home/link.txt'
        link.symlink_to(src)
        with self.assertRaises(ValueError):
            safe_copy_file(link,self.base/'Home/Progetti')

    def test_rejects_missing_destination(self):
        with self.assertRaises(ValueError):
            safe_copy_file(self.base/'Home/Leggimi.txt', self.base/'missing')

    def test_cannot_copy_file_to_itself(self):
        with self.assertRaises(FileExistsError):
            safe_copy_file(self.base/'Home/Leggimi.txt',self.base/'Home')

if __name__=='__main__': unittest.main(verbosity=2)
