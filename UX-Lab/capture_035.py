"""Capture real Tk/X11 UX lab screenshots with Xvfb >=1600x900."""
from pathlib import Path
import tkinter as tk
from PIL import ImageGrab
from melania_aero_live import MelaniaLive
out=Path(__file__).resolve().parent.parent/'Screenshots-v035'
out.mkdir(exist_ok=True)
root=tk.Tk()
app=MelaniaLive(root, out/'events.jsonl',width=1536,height=864)
root.update()
def capture(filename):
    root.update()
    ImageGrab.grab(bbox=(0,0,app.width,app.height)).save(out/filename)
    print(filename,app.width,app.height,app.file_layout.get('columns'),flush=True)
app.start_open=False
app.render()
capture('01-Files-Aero-toolbar.png')
app.start_open=True
app.render()
capture('02-Aero-Start.png')
app.start_open=False
app.render()
root.geometry('1024x720+0+0');root.update()
capture('03-Responsive-1024x720.png')
root.destroy()
