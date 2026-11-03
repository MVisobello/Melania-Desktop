"""Guard GUI responsiveness without bypassing real nested integration CI."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
COMPOSITOR=(ROOT/"melania-compositor/src/main.rs").read_text()
PANEL=(ROOT/"melania-panel/src/main.rs").read_text()
FILES=(ROOT/"melania-files/src/main.rs").read_text()

class ResponsivenessContracts(unittest.TestCase):
    def test_compositor_paces_full_frame_rendering(self):
        self.assertIn("fn frame_idle_budget(", COMPOSITOR)
        self.assertIn("Duration::from_nanos(16_666_667).saturating_sub(frame_elapsed)",COMPOSITOR)
        self.assertIn("if !budget.is_zero() {thread::sleep(budget);}",COMPOSITOR)
        self.assertIn("MELANIA_COMPOSITOR_FPS=",COMPOSITOR)

    def test_window_list_ipc_does_not_block_gtk(self):
        section=PANEL[PANEL.index("fn running_window_strip("):PANEL.index("/// A single taskbar icon")]
        self.assertIn("thread::spawn(move ||", section)
        self.assertIn("sync_channel(1)",section)
        self.assertIn("receiver.try_recv()", section)
        self.assertIn("refresh(windows)",section)

    def test_hover_previews_are_bounded_and_not_gtk_blocking(self):
        section=PANEL[PANEL.index("let preview_inflight=Rc::new(Cell::new(false));"):]
        self.assertIn("take(6)",section)
        self.assertIn("std::sync::mpsc::channel::<(usize,PathBuf)>()",section)
        self.assertIn("thread::spawn(move ||",section)
        self.assertIn("receiver.try_recv()",section)
        self.assertIn("picture.set_filename(Some(&image_path))",section)

    def test_search_is_debounced_to_avoid_full_gtk_widget_rebuild(self):
        self.assertIn("search_revision: Rc<Cell<u64>>",FILES)
        self.assertIn("search_revision: Rc::new(Cell::new(0))",FILES)
        self.assertIn("browser.suppress_search_refresh.set(true)",FILES)
        self.assertIn("Duration::from_millis(150)",FILES)
        self.assertIn("if current_revision.get()==ticket {refresh(&tab);}",FILES)

if __name__=="__main__":
    unittest.main()
