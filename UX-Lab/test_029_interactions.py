"""Tests for interactive UX improvements in v0.29 (Tk/X11 only)."""
import tempfile
from pathlib import Path
import unittest
import tkinter as tk
from melania_aero_live import MelaniaLive

class TestV029(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.root=tk.Tk()
        self.app=MelaniaLive(self.root,Path(self.temp.name)/'actions.jsonl')
        self.root.update()

    def tearDown(self):
        self.root.destroy()
        self.temp.cleanup()

    def test_start_filters_programs_and_can_launch_first_result(self):
        a=self.app
        self.assertEqual(len(a.start_programs()),10)
        self.assertEqual(a.start_query(),'')
        a.start_search_var.set('Impos')
        self.assertEqual(a.start_query(),'impos')
        self.assertTrue(a.start_open)
        a.select_first_search_result()
        self.assertTrue(a.settings_open)
        self.assertFalse(a.start_open)

    def test_file_search_filters_not_merely_decorative(self):
        a=self.app
        self.assertEqual(a.file_layout['visible_count'],8)
        a.file_search_var.set('Dolomiti')
        self.assertEqual(a.selected,3)
        self.assertEqual(a.file_layout['visible_count'],1)
        self.assertEqual(len(a.tile_rects),1)
        a.file_search_var.set('non-esiste')
        self.assertEqual(a.file_layout['visible_count'],0)

    def test_grid_and_list_toggle_with_real_state(self):
        a=self.app
        self.assertEqual(a.file_layout['view'],'griglia')
        a.action('view-toggle')
        self.assertEqual(a.file_layout['view'],'lista')
        self.assertEqual(len(a.tile_rects),8)
        a.action('view-toggle')
        self.assertEqual(a.file_layout['view'],'griglia')

    def test_inspector_can_be_toggled(self):
        a=self.app
        self.assertTrue(a.details_enabled)
        a.action('details-toggle')
        self.assertFalse(a.details_enabled)
        self.assertEqual(a.file_layout['inspector'][0],a.file_layout['inspector'][2]-0)
        a.action('details-toggle')
        self.assertTrue(a.details_enabled)
        self.assertLess(a.file_layout['inspector'][0],a.file_layout['inspector'][2])

    def test_taskbar_preview_contains_rendered_window(self):
        a=self.app
        self.assertIsNotNone(a.file_preview_source)
        self.assertGreater(a.file_preview_source.width,300)
        self.assertGreater(a.file_preview_source.height,200)
        a.on_motion(type('E',(),{'x':100,'y':a.taskbar_top+20})())
        self.assertTrue(a.preview_windows)
        a.on_motion(type('E',(),{'x':120,'y':a.taskbar_top-30})())
        self.assertTrue(a.preview_windows)

    def test_window_minimize_restore_maximize(self):
        a=self.app
        a.action('file-min')
        self.assertTrue(a.file_minimized)
        self.assertFalse(a.file_open)
        a.action('files')
        self.assertTrue(a.file_open)
        self.assertFalse(a.file_minimized)
        a.action('file-max')
        self.assertTrue(a.file_maximized)
        self.assertLessEqual(a.file_layout['window'][2],a.width)
        a.action('file-max')
        self.assertFalse(a.file_maximized)

    def test_compact_800x600_has_no_overlap_and_workspaces(self):
        a=self.app
        self.root.geometry('800x600+0+0');self.root.update()
        self.assertEqual((a.width,a.height),(800,600))
        lay=a.file_layout
        self.assertLessEqual(lay['window'][2],a.width)
        self.assertLessEqual(lay['window'][3],a.taskbar_top)
        self.assertLessEqual(lay['grid'][2],lay['inspector'][0])
        self.assertLessEqual(len(a.tile_rects),8)
        a.action('workspace-cycle')
        self.assertEqual(a.workspace,2)
        a.action('workspace-cycle')
        self.assertEqual(a.workspace,3)

    def test_clickable_view_is_real_control(self):
        a=self.app
        x,y,x2,y2=a.file_layout['window']
        # Click the Vista button which calls view-toggle.
        px=x2-52;py=y+78
        a.click(type('E',(),{'x':px,'y':py})())
        self.assertEqual(a.file_view,'lista')

if __name__=='__main__':unittest.main(verbosity=2)
