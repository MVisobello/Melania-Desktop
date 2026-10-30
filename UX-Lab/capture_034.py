from pathlib import Path
import tkinter as tk
from PIL import ImageGrab
from melania_aero_live import MelaniaLive
out=Path(__file__).resolve().parent.parent/'Screenshots-v034'
out.mkdir(exist_ok=True)
root=tk.Tk()
app=MelaniaLive(root,out/'events.jsonl',width=1536,height=864)
root.update()
def cap(name):
    root.update(); ImageGrab.grab(bbox=(0,0,app.width,app.height)).save(out/name);print(name)
cap('01-Aero-restored-Start.png')
app.action('start')
cap('02-Aero-restored-Files.png')
app.action('quick')
cap('03-Aero-restored-Quick.png')
root.geometry('1280x800+0+0');root.update()
app.action('quick')
cap('04-Aero-restored-1280x800.png')
root.destroy()
