"""Capture real Tk/X11 window screenshots from an existing display (not generated mockups)."""
from pathlib import Path
import tkinter as tk
from PIL import ImageGrab
from melania_aero_live import MelaniaLive

screens=Path(__file__).resolve().parent.parent/'Screenshots'
screens.mkdir(parents=True, exist_ok=True)
root=tk.Tk()
app=MelaniaLive(root, screens/'events.jsonl')
root.update()

def save(name):
    root.update()
    ImageGrab.grab(bbox=(0,0,app.width,app.height)).save(screens/name)
    print(name,app.width,app.height,app.start_rect,app.tray_rect,app.clock_rect)

save('01-Start-Toolbar-Tray-1536.png')
app.action('quick');save('02-Centro-Rapido-1536.png')
app.action('calendar');save('03-Calendario-1536.png')
for w,h in [(1366,768),(1280,800),(1024,720)]:
    root.geometry(f'{w}x{h}+0+0');root.update()
    app.action('start') if not app.start_open else None
    save(f'04-Responsive-{w}x{h}.png')
root.destroy()
