"""Generate real Xvfb screenshots from the executable Tk app."""
from pathlib import Path
import tkinter as tk
from PIL import ImageGrab
from melania_aero_live import MelaniaLive
out=Path(__file__).resolve().parent.parent/'Screenshots-v029'
out.mkdir(exist_ok=True)
root=tk.Tk()
app=MelaniaLive(root,out/'events.jsonl')
root.update()
def capture(name):
    root.update()
    ImageGrab.grab(bbox=(0,0,app.width,app.height)).save(out/name)
    print(name,app.width,app.height,app.file_layout.get('columns'))
capture('01-Start-rifinito.png')
app.action('start')
app.action('view-toggle')
capture('02-Files-elenco.png')
app.action('view-toggle')
app.action('quick')
capture('03-Centro-rapido.png')
app.action('quick')
app.on_motion(type('Evt',(),{'x':100,'y':app.taskbar_top+20})())
capture('04-Anteprima-reale.png')
root.geometry('800x600+0+0')
root.update()
app.action('start') if not app.start_open else None
capture('05-Responsive-800x600.png')
root.destroy()
