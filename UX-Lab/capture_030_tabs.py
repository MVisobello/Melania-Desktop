from pathlib import Path
import tkinter as tk
from PIL import ImageGrab
from melania_aero_live import MelaniaLive
out=Path(__file__).resolve().parent.parent/'Screenshots-v030'
out.mkdir(exist_ok=True)
root=tk.Tk()
app=MelaniaLive(root,out/'events.jsonl')
root.update()
def capture(name):
    root.update()
    ImageGrab.grab(bbox=(0,0,app.width,app.height)).save(out/name)
    print('saved',name)
# main default view with new tabs
capture('01-Files-tabs-browser-style.png')
# click new tab twice so we show strip richer
app.action('demo:Nuova scheda')
app.action('demo:Nuova scheda')
capture('02-Files-tabs-multiple.png')
# narrower responsive tab strip
root.geometry('1100x720+0+0')
root.update()
capture('03-Files-tabs-responsive.png')
root.destroy()
