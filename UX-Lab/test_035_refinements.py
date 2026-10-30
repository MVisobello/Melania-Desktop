"""Melania 0.35: focused executable UX Lab regressions; not native GTK tests."""
import unittest
import tkinter as tk
from pathlib import Path
from tempfile import TemporaryDirectory
from melania_aero_live import MelaniaLive

class RefinementTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=TemporaryDirectory()
        cls.root=tk.Tk()
        cls.app=MelaniaLive(cls.root, Path(cls.temp.name)/'035.jsonl', 1280, 800)
        cls.root.update()
    @classmethod
    def tearDownClass(cls):
        cls.root.destroy()
        cls.temp.cleanup()
    def setUp(self):
        a=self.app
        a.file_search_var.set(a.file_placeholder)
        a.file_view='griglia'
        a.file_reverse=False
        a.selected=1
        a.details_enabled=True
        a.file_open=True
        a.start_open=False
        a.render()
    def test_action_shelf_real_actions(self):
        a=self.app
        for name in ('file-tab-new','sort-toggle','view-toggle','details-toggle'):
            self.assertIn(name,a.action_rects)
        a.action('view-toggle')
        self.assertEqual(a.file_layout['view'],'lista')
        a.action('details-toggle')
        self.assertEqual(a.file_layout['inspector'][0],a.file_layout['inspector'][2])
    def test_metadata_consistent(self):
        a=self.app
        self.assertEqual(a.file_layout['selected_count'],a.file_counts[1])
        self.assertEqual(a.file_counts[1],19)
        a.action('select:5')
        self.assertEqual(a.file_layout['selected_count'],47)
    def test_sort_reverse_and_filter(self):
        a=self.app
        self.assertEqual(a.file_layout['visible_names'][0],'Lago di Como')
        a.action('sort-toggle')
        self.assertEqual(a.file_layout['visible_names'][0],'Borghi d’Italia')
        a.file_search_var.set('Ven')
        self.assertEqual(a.file_layout['visible_count'],1)
        self.assertEqual(a.file_layout['selected_count'],19)
    def test_no_overlap_compact(self):
        a=self.app
        self.root.geometry('800x600+0+0')
        self.root.update()
        a.render()
        layout=a.file_layout
        self.assertLessEqual(layout['grid'][2],layout['inspector'][0])
        for x1,y1,x2,y2 in a.tile_rects:
            self.assertGreaterEqual(x1,layout['grid'][0])
            self.assertGreaterEqual(y1,layout['grid'][1])
            self.assertLessEqual(x2,layout['grid'][2])
            self.assertLessEqual(y2,layout['grid'][3])
        self.root.geometry('1280x800+0+0')
        self.root.update()

if __name__=='__main__': unittest.main(verbosity=2)
