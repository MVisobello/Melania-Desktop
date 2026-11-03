"""Regression guard: a closed Files tab must not be retained by its title signal.

The GTK title update uses weak references so the signal closure cannot keep
the page, its browser state and directory monitors alive after tab closure.
GTK/Rust build and nested Wayland smoke tests run separately in GitHub Actions.
"""
from pathlib import Path
import unittest


FILES = (Path(__file__).resolve().parents[1] / "melania-files/src/main.rs").read_text()


class FilesTabTitleLifetime(unittest.TestCase):
    def test_label_notify_cannot_own_its_tab(self):
        section = FILES.split("    // The title label is owned by this page:", 1)[1]
        section = section.split("    // Only weak references are captured by tab controls", 1)[0]
        for marker in (
            "let tabs_for_title=notebook.downgrade();",
            "let page_for_title=child.downgrade();",
            "let window_for_title=window.downgrade();",
            "tabs_for_title.upgrade(), page_for_title.upgrade(), window_for_title.upgrade()",
            "if tabs.page_num(&page)==tabs.current_page()",
            "window.set_title(Some(&format!",
        ):
            self.assertIn(marker, section)
        for retained in (
            "let tabs_for_title=notebook.clone();",
            "let page_for_title=child.clone();",
            "let window_for_title=window.clone();",
        ):
            self.assertNotIn(retained, section)

    def test_closing_tabs_still_destroys_the_notebook_page(self):
        close = FILES.split("fn close_files_tab(", 1)[1].split("fn reorder_files_tab(", 1)[0]
        self.assertIn("strip.remove(&heading);", close)
        self.assertIn("headers.borrow_mut().retain(", close)
        self.assertIn("notebook.remove_page(Some(index));", close)


if __name__ == "__main__":
    unittest.main()
