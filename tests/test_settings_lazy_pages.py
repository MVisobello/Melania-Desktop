"""Preserve real Settings controls with lazy category materialization."""
from pathlib import Path
import re
import unittest

ROOT=Path(__file__).resolve().parents[1]
MAIN=(ROOT/"melania-settings/src/main.rs").read_text()
GNOME=(ROOT/"melania-settings/src/gnome_pages.rs").read_text()
XFCE=(ROOT/"melania-settings/src/xfce_pages.rs").read_text()
LAZY=(ROOT/"melania-settings/src/lazy_pages.rs").read_text()

class LazySettingsPages(unittest.TestCase):
    def test_all_25_stable_stack_destinations_remain_registered(self):
        registry=re.search(r'const SETTINGS_NAVIGATION:.*?= \[(.*?)\];',MAIN,re.S)
        keys=set(re.findall(r'^\s*\("[^"]+",\s*"([^"]+)",\s*"[^"]+"\),',registry.group(1),re.M))
        main_keys=set(re.findall(r'stack\.add_titled\([^;]+?Some\("([^"]+)"\)',MAIN,re.S))
        main_keys.update(re.findall(r'lazy_pages::register\(&stack,\s*"([^"]+)"',MAIN))
        gnome_keys=set(re.findall(r'lazy_pages::register\(stack,\s*"([^"]+)"',GNOME))
        xfce_keys=set(re.findall(r'^\s*\("(xfce-[^"]+)",',XFCE,re.M))
        self.assertEqual(len(keys),25)
        self.assertEqual(keys,main_keys|gnome_keys|xfce_keys)
        self.assertEqual(len(gnome_keys),12)
        self.assertEqual(len(xfce_keys),5)
        self.assertIn("network",main_keys)

    def test_heavy_gio_commands_only_in_on_demand_builders(self):
        self.assertIn("pub fn build_page(stack:&Stack, key:&str)->Option<GtkBox>",GNOME)
        self.assertIn("pub fn build_page(key:&str)->Option<gtk::Box>",XFCE)
        self.assertIn('"xfce-mime"=>Some(xfce_defaults::mime_page())',XFCE)
        self.assertIn('"audio" => {',GNOME)
        self.assertIn('"power" => {',GNOME)
        self.assertIn('"bluetooth" => {',GNOME)
        self.assertIn('fn network_page()->GtkBox',MAIN)
        self.assertIn('key=="network" ||',MAIN)

    def test_selected_page_materialized_once_after_window_first_paint(self):
        self.assertIn('HashSet<String>',MAIN)
        self.assertIn('pending_pages.borrow_mut().insert(key.to_owned())',MAIN)
        self.assertIn('Duration::from_millis(45)',MAIN)
        self.assertIn('gtk::glib::timeout_add_local_once',MAIN)
        self.assertLess(MAIN.index('window.present();'),MAIN.index('if let Some(name)=stack.visible_child_name() {activate_page(name.as_str());}'))
        self.assertIn('stack.connect_visible_child_name_notify',MAIN)
        self.assertIn('lazy_pages::fill(&stack,&target,page);',MAIN)
        self.assertIn('MELANIA_SETTINGS_FIRST_WINDOW_MS=',MAIN)
        self.assertIn('MELANIA_SETTINGS_PAGE_BUILD_MS=',MAIN)

    def test_stack_names_survive_real_widget_population(self):
        self.assertIn('stack.add_titled(&placeholder,Some(key),title);',LAZY)
        self.assertIn('stack.child_by_name(key)',LAZY)
        self.assertIn('while let Some(child)=host.first_child(){host.remove(&child);}',LAZY)
        self.assertIn('host.append(&content);',LAZY)
        self.assertNotIn('stack.remove(',LAZY)
        self.assertIn('MELANIA_SETTINGS_PAGES_OK={pages}',MAIN)
        self.assertIn('stack.child_by_name(target).is_some()',MAIN)

if __name__=="__main__":
    unittest.main()
