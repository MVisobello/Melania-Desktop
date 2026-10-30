"""Phase B1 GUI tests: execute using DISPLAY=:121 python3 -m unittest -v test_b1_responsive."""
from pathlib import Path
import tempfile
import tkinter as tk
import unittest
from melania_aero_live import MelaniaLive, SHELL_ICON

class TestB1Responsive(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory()
        self.root=tk.Tk()
        self.app=MelaniaLive(self.root,Path(self.temp.name)/'events.jsonl')
        self.root.update()

    def tearDown(self):
        self.root.destroy()
        self.temp.cleanup()

    def at_size(self, width, height):
        self.root.geometry(f'{width}x{height}+0+0')
        self.root.update()
        self.assertEqual((self.app.width,self.app.height),(width,height))
        self.app.render()
        self.root.update()

    def test_start_anchored_at_left_and_above_taskbar(self):
        for w,h in [(1536,864),(1366,768),(1280,800),(1024,720)]:
            with self.subTest(res=(w,h)):
                self.at_size(w,h)
                self.app.action('start') if not self.app.start_open else None
                x1,y1,x2,y2=self.app.start_rect
                self.assertEqual(x1,8)
                self.assertEqual(y2,self.app.taskbar_top-8)
                self.assertLess(x2,w//2)
                self.assertGreater(y1,0)

    def test_tray_clock_aligned_to_right_for_all_sizes(self):
        for w,h in [(1536,864),(1366,768),(1280,800),(1024,720)]:
            with self.subTest(res=(w,h)):
                self.at_size(w,h)
                rx=self.app.tray_rect
                cx=self.app.clock_rect
                self.assertGreater(rx[0],w-300)
                self.assertLess(rx[2],cx[0])
                self.assertLessEqual(cx[2],w)
                self.assertEqual(rx[1]-self.app.taskbar_top, 6)

    def test_icon_assets_and_variants(self):
        for variant in ('toolbar','tray'):
            for icon in ('nav-back','nav-forward','nav-up','nav-refresh','wifi','volume','bluetooth','battery','bell'):
                p=SHELL_ICON/variant/f'{icon}-20.png'
                self.assertTrue(p.is_file(),p)
                self.assertIsNotNone(self.app.shell_icon(icon,20,variant).getbbox())

    def test_grid_does_not_cover_inspector_or_leave_window(self):
        for w,h in [(1536,864),(1366,768),(1280,800),(1024,720)]:
            with self.subTest(res=(w,h)):
                self.at_size(w,h)
                lay=self.app.file_layout
                left,top,right,bottom=lay['grid']
                self.assertLessEqual(lay['grid'][2],lay['inspector'][0])
                self.assertLessEqual(right,lay['window'][2])
                self.assertGreaterEqual(lay['window'][1],0)
                self.assertLessEqual(lay['window'][3],self.app.taskbar_top)
                self.assertGreaterEqual(lay['columns'],2)
                for bx1,by1,bx2,by2 in self.app.tile_rects:
                    self.assertGreaterEqual(bx1,left)
                    self.assertGreaterEqual(by1,top)
                    self.assertLessEqual(bx2,right)
                    self.assertLessEqual(by2,bottom)

    def test_tray_and_clock_are_distinct_click_targets(self):
        self.app.action('start')
        x1,y1,x2,y2=self.app.tray_rect
        self.app.click(type('E',(),{'x':x1+36,'y':y1+20})())
        self.assertTrue(self.app.quick_open)
        c=self.app.clock_rect
        self.app.click(type('E',(),{'x':c[0]+30,'y':c[1]+15})())
        self.assertTrue(self.app.calendar_open)
        self.assertFalse(self.app.quick_open)

    def test_navigation_history_has_bounds(self):
        self.app.action('nav:back');self.assertEqual(self.app.nav_cursor,1)
        self.app.action('nav:back');self.assertEqual(self.app.nav_cursor,0)
        self.app.action('nav:back');self.assertEqual(self.app.nav_cursor,0)
        self.app.action('nav:forward');self.assertEqual(self.app.nav_cursor,1)
        self.app.action('nav:up');self.assertEqual(self.app.nav_cursor,0)

if __name__=='__main__': unittest.main(verbosity=2)
