from pathlib import Path
import tkinter as tk
from PIL import ImageGrab
from melania_aero_live import MelaniaLive
out=Path(__file__).resolve().parent.parent/'Screenshots-v033'
out.mkdir(exist_ok=True)
root=tk.Tk(); app=MelaniaLive(root,out/'events.jsonl'); root.update()
def cap(name):
    root.update(); ImageGrab.grab(bbox=(0,0,app.width,app.height)).save(out/name)
cap('01-TopTabs-main.png')
root.geometry('1280x800+0+0'); root.update(); cap('02-TopTabs-1280.png')
root.destroy()
