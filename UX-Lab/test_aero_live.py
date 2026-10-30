"""Run with DISPLAY=:98 python3 test_aero_live.py; requires a virtual X display."""
import tempfile
from pathlib import Path
import tkinter as tk
from melania_aero_live import MelaniaLive, THEME


def test_visual_interactions():
    with tempfile.TemporaryDirectory() as td:
        root = tk.Tk()
        app = MelaniaLive(root, Path(td)/'actions.jsonl')
        root.update()
        assert app.preview is not None
        assert app.start_open and not app.quick_open and not app.calendar_open
        # Important: use exact approved theme assets, not generic glyph substitutes.
        assert (THEME/'64x64'/'places'/'folder-documents.png').is_file()
        assert (THEME/'64x64'/'apps'/'microsoft-edge.png').is_file()
        icon = app.icon('places','folder-documents',62)
        assert icon.getbbox() is not None and icon.size == (62,62)
        assert len(app.hits) > 40
        app.action('start'); assert app.start_open is False
        app.action('start'); assert app.start_open is True
        app.action('toggle:Wi-Fi'); assert app.toggles['Wi-Fi'] is False
        app.action('toggle:Wi-Fi'); assert app.toggles['Wi-Fi'] is True
        app.action('workspace:3'); assert app.workspace == 3
        app.action('select:4'); assert app.selected == 4
        app.action('settings'); assert app.settings_open
        app.action('close-settings'); assert not app.settings_open
        root.update()
        logs=(Path(td)/'actions.jsonl').read_text(encoding='utf-8')
        assert 'toggle:Wi-Fi' in logs and 'workspace:3' in logs
        root.destroy()
    print('PASS: Melania Aero UX Lab / 14 graphical interaction assertions')

if __name__=='__main__':test_visual_interactions()
