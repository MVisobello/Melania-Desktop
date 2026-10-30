"""Screenshots from actual interactive Tk/X11 UX lab on a virtual display."""
from pathlib import Path
import tkinter as tk
from PIL import ImageGrab, Image
from melania_aero_live import MelaniaLive

out = Path(__file__).resolve().parent.parent/'Screenshots-v032'
out.mkdir(exist_ok=True)
root = tk.Tk()
app = MelaniaLive(root, out/'events.jsonl', 1536, 864)
root.update()
app.action('start')  # close Start for visual focus on Melania Files

def shot(name):
    root.update()
    src=ImageGrab.grab(bbox=(0,0,app.width,app.height))
    src.save(out/name)
    print(name,src.size,'visible:',[i for i,*_ in app.tab_geometry], 'active:',app.active_tab)
    return src

img=shot('01-Schede-unite.png')
# This is a crop of the screenshot, not a generated concept.
img.crop((375,53,1230,164)).resize((1710,222),Image.Resampling.LANCZOS).save(out/'01b-Dettaglio-schede.png')

for _ in range(9): app.action('file-tab-new')
shot('02-Molte-schede-e-scorrimento.png')
app.action('file-tab-scroll-left')
shot('03-Scorrimento-a-sinistra.png')
app.action('file-tab:0')
app.move_tab(0,2)
shot('04-Dopo-riordino.png')
app.file_tabs=app.file_tabs[:5]
app.active_tab=2
app.tab_offset=0
app.tab_scroll_manual=False
app.select_file_tab(2)
root.geometry('800x600+0+0');root.update()
shot('05-Responsive-800x600.png')
root.destroy()
