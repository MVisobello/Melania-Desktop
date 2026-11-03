"""Regression guards for closed-tab cancellation in Melania Files.

These source checks complement Rust compilation and the real nested Wayland
integration tests. They do not claim to measure VirtualBox frame timings.
"""
from pathlib import Path
import unittest

FILES=(Path(__file__).resolve().parents[1] / "melania-files/src/main.rs").read_text()


class FilesClosedTabCancellation(unittest.TestCase):
    def test_unrealize_invalidates_pending_render_and_search(self):
        cleanup=FILES.split("root.connect_unrealize(move |_| {",1)[1].split(
            "    });\n    refresh(&browser);",1)[0]
        # Make stale render slices, search debounce and monitor callbacks
        # observe their new generation before cancellation releases state.
        for marker in (
            "closing.listing_generation.set(closing.listing_generation.get().wrapping_add(1));",
            "closing.search_revision.set(closing.search_revision.get().wrapping_add(1));",
            "closing.monitor_epoch.set(closing.monitor_epoch.get().wrapping_add(1));",
            "closing.listing_cancel.borrow_mut().take()",
            "closing.thumbnail_cancel.borrow_mut().take()",
            "closing.listing_pending.borrow_mut().take()",
            "closing.thumbnail_pending.borrow_mut().take()",
        ):
            self.assertIn(marker,cleanup)
        self.assertLess(cleanup.index("closing.listing_generation.set("),
                        cleanup.index("closing.listing_cancel.borrow_mut().take()"))

    def test_background_results_and_idle_slices_are_generation_fenced(self):
        worker=FILES.split("fn start_listing_worker(",1)[1].split(
            "fn start_local_thumbnail_worker(",1)[0]
        render=FILES.split("fn render_listing(",1)[1].split(
            "// Keep a bounded navigation history",1)[0]
        self.assertIn("tab.listing_generation.get()==request.generation",worker)
        self.assertIn("browser.listing_generation.get()!=generation",render)

    def test_refresh_reports_only_opt_in_clear_cost(self):
        refresh=FILES.split("fn refresh(browser: &Browser) {",1)[1].split(
            "fn render_listing(",1)[0]
        self.assertIn("let clear_started=Instant::now();",refresh)
        self.assertIn("cleared_tiles+=1;",refresh)
        self.assertIn("if files_profile_enabled() {",refresh)
        self.assertIn("MELANIA_FILES_CLEAR_MS={} tiles={} generation={}",refresh)


if __name__=="__main__":
    unittest.main()
