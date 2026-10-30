"""Take genuine screenshots from the running Tk/X11 UX validation app.

Usage: DISPLAY=:109 python3 capture_phase_a.py
On Linux install Pillow and python3-tk; a display must be available.
"""
from pathlib import Path
import tkinter as tk
from PIL import ImageGrab
from melania_aero_live import MelaniaLive, TASKBAR_TOP

out = Path(__file__).resolve().parent.parent/'Screenshots'
out.mkdir(exist_ok=True)
root=tk.Tk()
app=MelaniaLive(root,out/'actions.jsonl')
root.update()
ImageGrab.grab().save(out/'01-Start-e-Melania-Files.png')
root.event_generate('<Button-1>',x=1288,y=TASKBAR_TOP+25)
root.update()
assert app.quick_open and not app.start_open
ImageGrab.grab().save(out/'02-Centro-rapido.png')
root.event_generate('<Button-1>',x=1460,y=TASKBAR_TOP+25)
root.update()
assert app.calendar_open and not app.quick_open
ImageGrab.grab().save(out/'03-Calendario.png')
root.event_generate('<Motion>',x=100,y=TASKBAR_TOP+20)
root.update()
assert app.preview_windows
ImageGrab.grab().save(out/'04-Anteprima-taskbar.png')
root.destroy()
print(f'Acquisite 4 schermate reali Tk/X11 in {out}')
