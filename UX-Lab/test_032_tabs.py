"""UI lab tests: real tab close, drag reorder, overflow, keyboard and invariants."""
import unittest
import tkinter as tk
from types import SimpleNamespace
from melania_aero_live import MelaniaLive

class TabTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.root=tk.Tk()
        cls.app=MelaniaLive(cls.root, '/tmp/melania-v032-tests.jsonl', 1280, 800)
        cls.root.update()
    @classmethod
    def tearDownClass(cls): cls.root.destroy()
    def setUp(self):
        self.app.file_tabs=[
            {'title':'Home','path':['Home']},
            {'title':'Immagini','path':['Home','Immagini']},
            {'title':'Viaggi','path':['Home','Immagini','Viaggi']},
        ]
        self.app.active_tab=2
        self.app.nav_path=['Home','Immagini','Viaggi']
        self.app.nav_cursor=2
        self.app.tab_offset=0
        self.app.tab_scroll_manual=False
        self.app.file_open=True
        self.app.file_minimized=False
        self.app.start_open=False
        self.app.render()
        self.root.update()
    def test_close_x_preserves_other_tabs(self):
        geom=self.app.tab_geometry
        idx,x1,y1,x2,y2=geom[1]
        self.app.on_mouse_down(SimpleNamespace(x=x2-8,y=(y1+y2)//2))
        self.assertEqual([t['title'] for t in self.app.file_tabs],['Home','Viaggi'])
        self.assertEqual(self.app.active_tab,1)
    def test_last_tab_remains_usable(self):
        self.app.file_tabs=[{'title':'Solo','path':['Home','Solo']}]
        self.app.active_tab=0
        self.app.close_file_tab(0)
        self.assertEqual([t['title'] for t in self.app.file_tabs],['Home'])
    def test_middle_click_close(self):
        idx,x1,y1,x2,y2=self.app.tab_geometry[0]
        self.app.on_middle_click(SimpleNamespace(x=(x1+x2)//2,y=(y1+y2)//2))
        self.assertEqual(len(self.app.file_tabs),2)
    def test_drag_to_reorder(self):
        idx,x1,y1,x2,y2=self.app.tab_geometry[0]
        last=self.app.tab_geometry[-1]
        y=(y1+y2)//2
        self.app.on_mouse_down(SimpleNamespace(x=x1+40,y=y))
        self.app.on_mouse_drag(SimpleNamespace(x=last[3]+2,y=y))
        self.app.on_mouse_up(SimpleNamespace(x=last[3]+2,y=y))
        self.assertEqual([t['title'] for t in self.app.file_tabs],['Immagini','Viaggi','Home'])
        self.assertEqual(self.app.active_tab,1)
    def test_new_tabs_and_overflow_arrows(self):
        for _ in range(12): self.app.action('file-tab-new')
        self.assertEqual(len(self.app.file_tabs),15)
        visible=[t[0] for t in self.app.tab_geometry]
        self.assertIn(self.app.active_tab, visible)
        self.assertLess(len(visible),len(self.app.file_tabs))
        prior = self.app.tab_offset
        self.app.action('file-tab-scroll-left')
        self.assertEqual(self.app.tab_offset,prior-1)
        self.assertTrue(self.app.tab_scroll_manual)
        self.assertNotIn(self.app.active_tab,[i for i,*_ in self.app.tab_geometry])
        self.app.action('file-tab:0')
        self.assertIn(0,[t[0] for t in self.app.tab_geometry])
    def test_switch_and_navigation_isolation(self):
        self.app.action('file-tab:0')
        self.assertEqual(self.app.nav_path,['Home'])
        self.app.action('file-tab:2')
        self.assertEqual(self.app.nav_path,['Home','Immagini','Viaggi'])
        self.app.action('nav:up')
        self.assertEqual(self.app.file_tabs[2]['path'],['Home','Immagini'])
        self.assertEqual(self.app.file_tabs[0]['path'],['Home'])
    def test_keyboard_next_prev(self):
        self.app.action('file-tab-next')
        self.assertEqual(self.app.active_tab,0)
        self.app.action('file-tab-prev')
        self.assertEqual(self.app.active_tab,2)
    def test_close_active_picks_neighbor(self):
        self.app.action('file-tab-close:2')
        self.assertEqual(self.app.active_tab,1)
        self.assertEqual(self.app.nav_path,['Home','Immagini'])
    def test_reorder_preserves_selected_tab(self):
        self.app.move_tab(2,0)
        self.assertEqual(self.app.active_tab,0)
        self.assertEqual(self.app.nav_path,['Home','Immagini','Viaggi'])
    def test_responsive_compact(self):
        self.root.geometry('800x600+0+0')
        self.root.update()
        self.app.render()
        for _,x1,y1,x2,y2 in self.app.tab_geometry:
            self.assertGreater(x2-x1,70)
            self.assertGreater(y2-y1,17)
        self.root.geometry('1280x800+0+0');self.root.update()

if __name__ == '__main__': unittest.main(verbosity=2)
