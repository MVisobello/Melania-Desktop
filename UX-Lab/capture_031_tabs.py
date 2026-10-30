from pathlib import Path
import tkinter as tk
from PIL import ImageGrab
from melania_aero_live import MelaniaLive
out=Path(__file__).resolve().parent.parent/'Screenshots-v031'
out.mkdir(exist_ok=True)
root=tk.Tk()
app=MelaniaLive(root,out/'events.jsonl')
root.update()
def capture(name):
    root.update()
    ImageGrab.grab(bbox=(0,0,app.width,app.height)).save(out/name)
    print('saved',name)
capture('01-Tabs-contigue-1536.png')
app.action('demo:Nuova scheda')
capture('02-Tabs-contigue-4tabs.png')
root.geometry('1100x720+0+0'); root.update()
capture('03-Tabs-contigue-responsive.png')
root.destroy()
