"""Service-backed Settings pages must never block GTK for nmcli/BlueZ."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
ASYNC=(ROOT/"melania-settings/src/settings_async.rs").read_text()
CONTROLS=(ROOT/"melania-settings/src/connectivity_controls.rs").read_text()
MAIN=(ROOT/"melania-settings/src/main.rs").read_text()

class NonblockingSettingsServiceTests(unittest.TestCase):
    def test_worker_only_transfers_send_data_and_main_context_updates_ui(self):
        self.assertIn("T:Send+'static",ASYNC)
        self.assertIn("std::thread::spawn(move ||",ASYNC)
        self.assertIn("mpsc::sync_channel(1)",ASYNC)
        self.assertIn("glib::timeout_add_local(Duration::from_millis(40)",ASYNC)
        self.assertIn("rx.try_recv()",ASYNC)
        self.assertIn("started.elapsed()>=Duration::from_secs(4)",ASYNC)
        self.assertIn("if let Some(callback)=completed.take(){callback(Some(value));}",ASYNC)
        self.assertIn("MELANIA_SETTINGS_SERVICE_MS=",ASYNC)

    def test_network_page_is_available_immediately_before_nmcli_returns(self):
        self.assertIn('text("Rilevamento connessioni in corso…"',MAIN)
        self.assertIn('settings_async::run("network-status",gnome_pages::network_status,move |result|',MAIN)
        self.assertIn("network.append(&refresh_network);",MAIN)
        self.assertIn("refresh_network.connect_clicked(move |button|",MAIN)
        self.assertIn("first_button.set_sensitive(true);",MAIN)

    def test_visual_ci_really_retries_gtk_screenshots(self):
        workflow=(ROOT/".github/workflows/settings-visual.yml").read_text()
        self.assertIn('scrot -o -q 92 "$path"',workflow)
        self.assertIn('scrot -o -q 92 "reports/settings-theme-',workflow)
        self.assertIn("ImageStat.Stat(im).stddev[0]>7",workflow)

    def test_wifi_and_bluetooth_discovery_is_async_not_synthetic(self):
        for name,command in (("wifi","wifi_radio"),("bluetooth","bluetooth_power")):
            start=CONTROLS.index("pub fn append_"+name+"(")
            end=CONTROLS.find("pub fn append_",start+4)
            body=CONTROLS[start:end if end>0 else len(CONTROLS)]
            self.assertIn('crate::settings_async::run("'+name+'-detect",'+command+',move |result|',body)
            self.assertIn('crate::settings_async::run("'+name+'-toggle",move ||{',body)
            self.assertIn("button.set_sensitive(false);",body)
            self.assertIn("ready_button.set_sensitive(true);",body)
            self.assertIn("row.set_visible(false);",body)
            self.assertIn("Some((true,Some(actual)))",body)
        self.assertIn('Command::new("nmcli").args(["radio","wifi",next])',CONTROLS)
        self.assertIn('Command::new("bluetoothctl").args(["power",next])',CONTROLS)
        self.assertIn("parse_bluetooth_power",CONTROLS)
        self.assertIn("parse_wifi_radio",CONTROLS)

if __name__=="__main__":
    unittest.main()
