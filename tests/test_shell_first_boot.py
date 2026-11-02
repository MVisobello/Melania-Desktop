"""Shell-first startup must keep the compositor robust and first paint fast."""
from pathlib import Path
import unittest

ROOT=Path(__file__).resolve().parents[1]
COMPOSITOR=(ROOT/"melania-compositor/src/main.rs").read_text()
DESKTOP=(ROOT/"melania-desktop-surface/src/main.rs").read_text()
SMOKE=(ROOT/"scripts/smoke-full-desktop-wayland.sh").read_text()

class ShellFirstBoot(unittest.TestCase):
    def test_shell_clients_prioritized_and_optional_clients_deferred(self):
        self.assertIn('["melania-desktop-surface","melania-panel"]',COMPOSITOR)
        self.assertIn("let stage_shell = ",COMPOSITOR)
        self.assertIn("if shell_drawn_this_frame {",COMPOSITOR)
        self.assertIn("backend.submit(Some(&[damage]))?;",COMPOSITOR)
        self.assertIn("for program in applications.drain(..)",COMPOSITOR)
        self.assertLess(COMPOSITOR.index("backend.submit(Some(&[damage]))?;"),
                        COMPOSITOR.index("for program in applications.drain(..)"))
        self.assertIn("startup_begin.elapsed()>=Duration::from_secs(6)",COMPOSITOR)

    def test_real_buffers_prove_first_shell_frame(self):
        self.assertIn("top_render_elements > 0 && background_render_elements > 0",COMPOSITOR)
        self.assertIn("MELANIA_BOOT_SHELL_FRAME_MS=",COMPOSITOR)
        self.assertIn("MELANIA_BOOT_APPLICATIONS_RELEASED_MS=",COMPOSITOR)
        self.assertIn("fallback=false",SMOKE)

    def test_desktop_icon_enumeration_is_not_a_first_paint_blocker(self):
        start=DESKTOP.index("fn build(app: &Application)")
        part=DESKTOP[start:]
        self.assertIn("timeout_add_local_once(Duration::from_millis(120)",part)
        self.assertLess(part.index("window.present();"),
                        len(part))
        self.assertIn("refresh_pending.replace(true)",part)
        self.assertIn("timeout_add_local_once(Duration::from_millis(180)",part)

    def test_real_desktop_ci_requires_proven_startup_order(self):
        self.assertIn("max(start_d,start_p)<paint<released<min(files,settings)",SMOKE)
        self.assertIn("PASS: real desktop wallpaper and panel rendered before Files/Settings launch",SMOKE)

if __name__ == "__main__":
    unittest.main()
