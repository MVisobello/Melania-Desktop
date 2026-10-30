#!/usr/bin/env python3
"""Regression checks for responsive GTK Settings; no user-wide GTK edits."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]

class ResponsiveSettingsContract(unittest.TestCase):
    def test_sidebar_can_be_collapsed_and_restored(self):
        main=(ROOT/"melania-settings/src/main.rs").read_text()
        css=(ROOT/"melania-settings/assets/gnome-overview.css").read_text()
        for text in ("gtk::Revealer::new()", "SlideRight",
                     "set_reveal_child(!side.reveals_child())",
                     "gtk::ShortcutController::new()", 'parse_string("<Control>b")'):
            self.assertIn(text,main)
        self.assertIn("settings-sidebar-toggle",main)
        self.assertIn("settings-sidebar-toggle",css)
        self.assertIn("root.append(&sidebar_revealer)",main)
        self.assertIn("scroll.set_vexpand(true)",main)

    def test_network_refresh_is_explicit_and_non_destructive(self):
        main=(ROOT/"melania-settings/src/main.rs").read_text()
        pages=(ROOT/"melania-settings/src/gnome_pages.rs").read_text()
        self.assertIn("Aggiorna stato della rete",main)
        self.assertIn("current_network.set_text(&gnome_pages::network_status())",main)
        self.assertIn("network_status()",pages)
        self.assertNotIn('args(["radio", "wifi", "off"])',pages)
        self.assertNotIn('args(["radio", "wifi", "off"])',main)

    def test_approved_fluent_pack_unchanged(self):
        # Local widgets and style additions, no changed image pack entries.
        main=(ROOT/"melania-settings/src/main.rs").read_text()
        self.assertIn('mod symbols;', main)
        self.assertIn("Melania Fluent Light (predefinito)",main)

if __name__=="__main__":
    unittest.main(verbosity=2)
