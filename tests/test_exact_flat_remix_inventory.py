#!/usr/bin/env python3
"""Exact-inventory icon builder smoke test (no third-party artwork included)."""
from __future__ import annotations
import io
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/"tools"))
from build_melania_exact_flat_remix import MAP,build

class ExactInventoryTest(unittest.TestCase):
    def test_preserves_paths_symlinks_and_never_copies_reference_svg(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td)
            archive=root/"reference.tar.xz"
            with tarfile.open(archive,"w:xz") as tar:
                for upstream in MAP:
                    self.add_dir(tar,upstream+"/")
                    index=(f"[Icon Theme]\nName={upstream}\n"
                           "Directories=places/scalable\n"
                           "[places/scalable]\nSize=96\nType=Scalable\nContext=Places\n")
                    self.add_file(tar,upstream+"/index.theme",index)
                self.add_dir(tar,"Flat-Remix-Yellow-Dark/places/")
                self.add_dir(tar,"Flat-Remix-Yellow-Dark/places/scalable/")
                fake_source_svg="<svg xmlns='http://www.w3.org/2000/svg'><!-- DO NOT COPY --></svg>"
                self.add_file(tar,"Flat-Remix-Yellow-Dark/places/scalable/folder.svg",fake_source_svg)
                self.add_link(tar,"Flat-Remix-Yellow-Light/places",
                              "../Flat-Remix-Yellow-Dark/places")
                self.add_link(tar,"Flat-Remix-Yellow-Light-darkPanel/places",
                              "../Flat-Remix-Yellow-Dark/places")
            out=root/"out"
            result=build(archive,out)
            self.assertTrue(result["file_tree_exact"])
            self.assertEqual(result["broken_links"],0)
            self.assertEqual(result["themes"]["Melania-Dark"]["new-svg"],1)
            self.assertTrue((out/"Melania-Light/places/scalable/folder.svg").is_file())
            self.assertTrue((out/"Melania-Light-darkPanel/places/scalable/folder.svg").is_file())
            redrawn=(out/"Melania-Dark/places/scalable/folder.svg").read_text()
            self.assertIn("<svg",redrawn)
            self.assertNotIn("DO NOT COPY",redrawn)
            self.assertIn("Melania-Light", (out/"Melania-Light/index.theme").read_text())
            self.assertEqual(
                (out/"Melania-Light/places").readlink().as_posix(),
                "../Melania-Dark/places"
            )

    @staticmethod
    def add_dir(tar,name):
        info=tarfile.TarInfo(name)
        info.type=tarfile.DIRTYPE
        info.mode=0o755
        tar.addfile(info)

    @staticmethod
    def add_file(tar,name,content):
        raw=content.encode()
        info=tarfile.TarInfo(name)
        info.size=len(raw)
        info.mode=0o644
        tar.addfile(info,io.BytesIO(raw))

    @staticmethod
    def add_link(tar,name,target):
        info=tarfile.TarInfo(name)
        info.type=tarfile.SYMTYPE
        info.linkname=target
        tar.addfile(info)

if __name__=="__main__":
    unittest.main(verbosity=2)
