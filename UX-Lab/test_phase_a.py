"""Real GUI model tests under Xvfb: DISPLAY=:109 python3 test_phase_a.py."""
from pathlib import Path
import tempfile
import tkinter as tk
import unittest
from melania_aero_live import MelaniaLive, file_columns, TASKBAR_TOP, MONTHS_IT


class TestPhaseA(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root=tk.Tk()
        self.app=MelaniaLive(self.root,Path(self.temp.name)/'events.jsonl')
        self.root.update()

    def tearDown(self):
        self.root.destroy()
        self.temp.cleanup()

    def test_no_overlapping_thumbnails(self):
        a=self.app
        layout=a.file_layout
        self.assertEqual(layout['columns'],3)
        self.assertEqual(len(a.tile_rects),8)
        self.assertLessEqual(layout['sidebar'][2],layout['grid'][0])
        self.assertLessEqual(layout['grid'][2],layout['inspector'][0])
        for x1,y1,x2,y2 in a.tile_rects:
            self.assertGreaterEqual(x1,layout['grid'][0])
            self.assertLessEqual(x2,layout['grid'][2])
            self.assertGreaterEqual(y1,layout['grid'][1])
            self.assertLessEqual(y2,layout['grid'][3])
        for i,a1 in enumerate(a.tile_rects):
            for b in a.tile_rects[i+1:]:
                ox=max(0,min(a1[2],b[2])-max(a1[0],b[0]))
                oy=max(0,min(a1[3],b[3])-max(a1[1],b[1]))
                self.assertEqual(ox*oy,0)

    def test_column_calculation_responsive(self):
        widths=[680,750,819,1000]
        cols=[file_columns(n)[0] for n in widths]
        self.assertEqual(cols[2],3)
        self.assertEqual(cols,sorted(cols))
        self.assertTrue(all(1<=c<=6 for c in cols))

    def test_exclusive_flyouts(self):
        a=self.app
        self.assertEqual((a.start_open,a.quick_open,a.calendar_open),(True,False,False))
        a.action('quick')
        self.assertEqual((a.start_open,a.quick_open,a.calendar_open),(False,True,False))
        a.action('calendar')
        self.assertEqual((a.start_open,a.quick_open,a.calendar_open),(False,False,True))
        a.action('start')
        self.assertEqual((a.start_open,a.quick_open,a.calendar_open),(True,False,False))
        a.action('start')
        self.assertEqual((a.start_open,a.quick_open,a.calendar_open),(False,False,False))

    def test_click_outside_dismisses_panels(self):
        a=self.app
        a.click(type('Event',(),{'x':1490,'y':600})())
        self.assertFalse(a.start_open)
        self.assertTrue(a.file_open)

    def test_icons_from_approved_theme(self):
        a=self.app
        self.assertIsNotNone(a.icon('places','folder-documents',32).getbbox())
        self.assertIsNotNone(a.icon('apps','firefox',32).getbbox())
        self.assertIsNotNone(a.detail_venice)
        self.assertGreaterEqual(a.detail_venice.width,190)

    def test_preview_hover_only(self):
        a=self.app
        self.assertFalse(a.preview_windows)
        a.on_motion(type('Event',(),{'x':100,'y':TASKBAR_TOP+20})())
        self.assertEqual(a.hovered_window,'files')
        self.assertTrue(a.preview_windows)
        a.on_motion(type('Event',(),{'x':340,'y':700})())
        self.assertIsNone(a.hovered_window)
        self.assertFalse(a.preview_windows)

    def test_controls_and_focus(self):
        a=self.app
        a.action('quick-tab:Area rapida')
        self.assertTrue(a.quick_open)
        self.assertEqual(a.quick_tab,'Area rapida')
        a.action('toggle:Wi-Fi')
        self.assertFalse(a.toggles['Wi-Fi'])
        a.action('workspace:2')
        self.assertEqual(a.workspace,2)
        self.assertFalse(a.quick_open)

    def test_notification_clear(self):
        app=self.app
        self.assertEqual(len(app.notices),3)
        app.action('quick-tab:Area rapida')
        self.assertEqual(app.quick_tab,'Area rapida')
        app.action('quick-tab:Notifiche')
        app.action('clear-notices')
        self.assertEqual(len(app.notices),0)
        self.assertTrue(app.quick_open)

    def test_calendar_month_navigation(self):
        app=self.app
        app.action('calendar')
        self.assertTrue(app.calendar_open)
        app.action('calendar-next')
        self.assertEqual(app.calendar_month_offset,1)
        app.action('calendar-prev')
        self.assertEqual(app.calendar_month_offset,0)

    def test_months_italian(self):
        self.assertEqual(MONTHS_IT[9],'ottobre')
        self.assertEqual(MONTHS_IT[0],'gennaio')


if __name__=='__main__':
    unittest.main(verbosity=2)
