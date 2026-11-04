#!/usr/bin/env python3
"""Synthetic validator unit tests; not fabricated QEMU screenshot evidence."""
import importlib.util
import unittest
from pathlib import Path
p=Path(__file__).resolve().parents[1]/"scripts/capture-qemu-real-layer-panel.py"
spec=importlib.util.spec_from_file_location("layer_qmp",p)
assert spec and spec.loader
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
def picture():
    data=bytearray(m.W*m.H*3)
    for (x,y),rgb in [
        ((10,747),(27,33,58)),((10,748),m.color(12)),
        ((700,780),m.color(12)),((1279,799),m.color(12)),
        ((200,200),m.color(11)),((200,343),m.color(11)),
        ((200,344),(27,33,58)),((1000,700),(27,33,58))
    ]:
        offset=(y*m.W+x)*3
        data[offset:offset+3]=bytes(rgb)
    return b"P6\n1280 800\n255\n"+data
class TestPanelProof(unittest.TestCase):
    def test_correct_pixels(self):self.assertEqual(len(m.verify(picture())),8)
    def test_wrong_edge_fails(self):
        b=bytearray(picture());b[-3:]=b"\x00\x00\x00"
        with self.assertRaises(ValueError):m.verify(b)
    def test_panel_must_not_appear_above_52px_boundary(self):
        b=bytearray(picture());offset=len(b)-m.W*m.H*3+(747*m.W+10)*3
        b[offset:offset+3]=bytes(m.color(12))
        with self.assertRaises(ValueError):m.verify(b)
    def test_invalid_size_fails(self):
        with self.assertRaises(ValueError):m.verify(b"P6\n1 1\n255\n\x00\x00\x00")
if __name__=="__main__":unittest.main()
