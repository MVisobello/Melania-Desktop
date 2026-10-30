#!/usr/bin/env python3
"""Melania Aero — interactive, screenshot-able Tk/X11 UX validation app.

NOT the production Rust/GTK/Smithay compositor. It uses the actual approved
Melania Aero Classic FINAL icon theme and draws composited controls on Tk.
The output can be tested on Xvfb without a GPU and is a visual integration lab.
"""
from __future__ import annotations
import argparse
import calendar
import datetime as dt
import json
from pathlib import Path
import tkinter as tk
from PIL import Image, ImageDraw, ImageFont, ImageFilter, ImageTk

BASE = Path(__file__).resolve().parent
THEME = BASE / 'assets' / 'Melania-Aero-Classic'
LOGO = BASE / 'assets' / 'melania-128.png'
WALL = BASE / 'assets' / 'wallpaper.png'
REFERENCE = BASE / 'assets' / 'reference.png'
W, H = 1536, 864
DARK = (7, 22, 42)
AZURE = (30, 147, 249)
SMALL = '#46607b'
FONT_REG = '/usr/share/fonts/truetype/lato/Lato-Regular.ttf'
FONT_BOLD = '/usr/share/fonts/truetype/lato/Lato-Bold.ttf'
FONT_FALLBACK = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'


def font(size: int, bold=False):
    p = FONT_BOLD if bold else FONT_REG
    try: return ImageFont.truetype(p, size)
    except OSError: return ImageFont.truetype(FONT_FALLBACK, size)


def text(d, xy, s, size=13, color=(20, 43, 72), bold=False, anchor=None):
    d.text(xy, str(s), font=font(size, bold), fill=color, anchor=anchor)


def rounded(d, b, radius, fill, outline=None, width=1):
    d.rounded_rectangle(b, radius=radius, fill=fill, outline=outline, width=width)


def line(d, points, fill, width=1): d.line(points, fill=fill, width=width)


def photo_thumb(ix):
    """Small genuine photo fragments from user's supplied design reference."""
    if not REFERENCE.exists(): return None
    ref = Image.open(REFERENCE).convert('RGB')
    # Only photo content; skip UI frames and labels; images supplied by user.
    boxes = [(680,142,754,213),(781,142,853,213),(879,142,954,213),
             (976,142,1052,213),(681,266,753,334),(780,266,852,334),
             (879,266,953,334),(977,266,1050,334)]
    return ref.crop(boxes[ix % len(boxes)])


class MelaniaLive:
    def __init__(self, root, logpath=None):
        self.root = root
        self.root.title('Melania — Aero UX Lab (Tk/X11, non compositore Wayland)')
        root.geometry(f'{W}x{H}+0+0'); root.resizable(False, False)
        self.canvas = tk.Canvas(root, width=W, height=H, highlightthickness=0)
        self.canvas.pack()
        self.icons = {}
        self.thumb = [photo_thumb(i) for i in range(8)]
        self.logo = Image.open(LOGO).convert('RGBA')
        self.wall = Image.open(WALL).convert('RGB').resize((W,H), Image.Resampling.LANCZOS)
        self.hits = []
        self.preview = None
        self.start_open = True
        self.quick_open = True
        self.calendar_open = True
        self.file_open = True
        self.settings_open = False
        self.selected = 1
        self.category = 'Immagini'
        self.workspace = 1
        self.preview_windows = True
        self.toggles = {'Wi-Fi': True, 'Bluetooth': True, 'Non disturbare': False, 'Risparmio energia': True, 'Modalità scura': False}
        self.logpath = Path(logpath) if logpath else BASE/'events.jsonl'
        self.logpath.parent.mkdir(parents=True, exist_ok=True)
        root.bind('<Button-1>', self.click)
        root.bind('<Escape>', self.close_popovers)
        root.bind('<Control-e>', lambda _ : self.action('files'))
        root.bind('<Super_L>', lambda _ : self.action('start'))
        root.bind('<F5>', lambda _ : self.render())
        self.render()

    def log(self, action):
        with self.logpath.open('a', encoding='utf-8') as f:
            f.write(json.dumps({'event':action, 'at':dt.datetime.now().isoformat()},ensure_ascii=False)+'\n')

    def hit(self, xy, action): self.hits.append((xy,action))

    def click(self, e):
        for (x1,y1,x2,y2), action in reversed(self.hits):
            if x1 <= e.x <= x2 and y1 <= e.y <= y2:
                self.action(action); return

    def close_popovers(self, _=None):
        self.start_open=False; self.quick_open=False; self.calendar_open=False; self.preview_windows=False
        self.render()

    def action(self, action):
        self.log(action)
        if action=='start': self.start_open=not self.start_open
        elif action=='quick': self.quick_open=not self.quick_open
        elif action=='calendar': self.calendar_open=not self.calendar_open
        elif action=='files': self.file_open=not self.file_open
        elif action=='settings': self.settings_open=not self.settings_open
        elif action=='previews': self.preview_windows=not self.preview_windows
        elif action.startswith('toggle:'):
            s=action.partition(':')[2]; self.toggles[s] = not self.toggles[s]
        elif action.startswith('select:'): self.selected=int(action.partition(':')[2])
        elif action.startswith('category:'): self.category=action.partition(':')[2]
        elif action.startswith('workspace:'): self.workspace=int(action.partition(':')[2])
        elif action=='close-file': self.file_open=False
        elif action=='close-settings': self.settings_open=False
        self.render()

    def icon(self, category, name, sz):
        key=(category,name,sz)
        if key in self.icons: return self.icons[key]
        for orig in (sz,128,64,48,32,24,256):
            p=THEME/f'{orig}x{orig}'/category/f'{name}.png'
            if p.is_file():
                im=Image.open(p).convert('RGBA').resize((sz,sz), Image.Resampling.LANCZOS)
                self.icons[key]=im; return im
        if category != 'apps': return self.icon('apps','applications-other',sz)
        return Image.new('RGBA',(sz,sz))

    def paste_icon(self, image, cat, name, xy, size):
        try: im=self.icon(cat,name,size)
        except (FileNotFoundError, KeyError): return
        image.paste(im,xy,im)

    def glass(self, image, xy, radius=17, rgba=(6,24,44,224), border=(138,200,255,115), blur=13):
        # Real pixel blur inside glass areas, computed by Pillow in the executable UI.
        x1,y1,x2,y2=xy
        fragment=image.crop((x1,y1,x2,y2)).filter(ImageFilter.GaussianBlur(blur))
        mask=Image.new('L',(x2-x1,y2-y1)); ImageDraw.Draw(mask).rounded_rectangle((0,0,x2-x1-1,y2-y1-1),radius=radius,fill=255)
        image.paste(fragment,(x1,y1),mask)
        overlay=Image.new('RGBA',(x2-x1,y2-y1),(0,0,0,0))
        od=ImageDraw.Draw(overlay)
        od.rounded_rectangle((0,0,x2-x1-1,y2-y1-1),radius=radius,fill=rgba,outline=border,width=1)
        image.alpha_composite(overlay,(x1,y1))

    def canvas_bg(self):
        im=Image.new('RGBA',(W,H))
        # Overall palette tuned towards the blue lake/landscape in the approved image.
        wall=self.wall.convert('RGBA')
        lift=Image.new('RGBA',(W,H),(11,46,91,38))
        wall=Image.alpha_composite(wall,lift)
        im.alpha_composite(wall)
        return im

    def draw_desktop(self, im, d):
        icons=[('places','user-home','Home'),('places','folder-documents','Documenti'),('places','folder-pictures','Immagini'),('places','computer','Computer'),('places','user-trash','Cestino')]
        for i,(cat,name,label) in enumerate(icons):
            y=30+i*116
            self.paste_icon(im,cat,name,(31,y),62)
            text(d,(64,y+67),label,size=13,color='white',bold=True,anchor='mt')
            self.hit((18,y,116,y+93), 'files')

    def draw_file(self, im,d):
        if not self.file_open: return
        x,y,w,h=371,53,819,579
        self.glass(im,(x,y,x+w,y+h),radius=13,rgba=(19,61,100,165),blur=11)
        # Window header, clear white Explorer content; Aero blur visible at border and top.
        rounded(d,(x+7,y+39,x+w-7,y+h-7),8,fill=(249,252,255,255))
        rounded(d,(x+13,y+50,x+w-13,y+104),7,fill=(242,248,255,255),outline=(208,225,244,255))
        # fake native glass titlebar and tabs
        self.paste_icon(im,'apps','file-manager',(x+14,y+10),23)
        text(d,(x+45,y+15),'Melania Files',14,(235,247,255),True)
        for i,(t,c) in enumerate([('—',0),('□',1),('×',2)]):
            xx=x+w-119+i*34
            rounded(d,(xx,y+10,xx+32,y+32),5, fill=(37,74,107,155) if i<2 else (195,67,69,255),outline=(174,218,248,110))
            text(d,(xx+16,y+12),t,14,'white',True,anchor='mt')
        self.hit((x+w-51,y+10,x+w-8,y+36),'close-file')
        rounded(d,(x+84,y+32,x+231,y+61),6,fill=(244,249,255),outline=(182,211,236))
        self.paste_icon(im,'places','folder',(x+91,y+37),19)
        text(d,(x+119,y+39),'Home',12,(20,69,124),True)
        text(d,(x+208,y+40),'×',12,(100,139,169))
        rounded(d,(x+239,y+37,x+270,y+60),5,fill=(36,98,145,130))
        text(d,(x+255,y+41),'+',16,'white',True,anchor='mt')
        # Toolbar and addressbar
        for i,sym in enumerate(('‹','›','↑')):
            cx=x+29+i*36
            d.ellipse((cx-11,y+65,cx+12,y+88),fill=(250,253,255),outline=(195,217,237))
            text(d,(cx,y+67),sym,18,(52,92,130),True,anchor='mt')
        rounded(d,(x+146,y+66,x+497,y+94),7,fill=(255,255,255),outline=(206,224,243))
        self.paste_icon(im,'places','user-home',(x+154,y+71),17)
        text(d,(x+181,y+71),'Home   ›   Immagini   ›   Viaggi',12,(55,79,110))
        rounded(d,(x+510,y+66,x+w-115,y+94),8,fill=(255,255,255),outline=(206,224,243))
        text(d,(x+526,y+71),'⌕  Cerca in Viaggi...',12,(133,154,176))
        rounded(d,(x+w-105,y+67,x+w-19,y+94),7,fill=(237,243,251),outline=(202,220,237))
        text(d,(x+w-62,y+71),'Dettagli',12,(46,83,126),anchor='mt')
        # Sidebar
        left=x+11; sidebar_y=y+108; side_w=165; foot=y+h-42
        rounded(d,(left,sidebar_y,left+side_w,foot),5,fill=(239,247,255),outline=(209,227,245))
        rows=[('user-home','Home'),('document-open-recent','Recenti'),('emblem-favorite','Preferiti'),('folder-documents','Documenti'),('folder-pictures','Immagini'),('folder-music','Musica'),('folder-videos','Video'),('folder-download','Download'),('user-trash','Cestino')]
        for i,(ic,lab) in enumerate(rows):
            iy=sidebar_y+15+i*34
            if lab==self.category: rounded(d,(left+7,iy-3,left+side_w-8,iy+29),5,fill=(185,222,254),outline=(160,201,248))
            self.paste_icon(im,'places',ic,(left+17,iy+2),19)
            text(d,(left+44,iy+4),lab,12,(36,72,110),lab==self.category)
            self.hit((left+10,iy-3,left+side_w-8,iy+29),'category:'+lab)
        text(d,(left+15,sidebar_y+329),'Dispositivi',11,(101,127,153),True)
        for i,(ic,lab) in enumerate([('computer','Sistema'),('network-server','Rete locale')]):
            iy=sidebar_y+355+i*29
            self.paste_icon(im,'places',ic,(left+17,iy),19)
            text(d,(left+43,iy+2),lab,11,(45,81,121))
        # white grid
        grid_x=left+side_w+4; grid_end=x+w-233
        d.rectangle((grid_x,sidebar_y,grid_end,foot),fill=(255,255,255))
        # Image tiles, crop thumbnails from user's approved reference
        names=['Lago di Como','Venezia','Firenze','Dolomiti','Cinque Terre','Costiera Amalfitana','Tramonto','Borghi d’Italia']
        for i in range(8):
            col=i%4; row=i//4
            px=grid_x+14+col*112; py=sidebar_y+17+row*140
            box=(px-4,py-4,px+101,py+124)
            if i==self.selected: rounded(d,box,7,fill=(209,235,255),outline=(37,139,246),width=2)
            crop=self.thumb[i]
            if crop is not None:
                tile=crop.resize((96,82),Image.Resampling.LANCZOS)
                im.paste(tile,(px,py))
                d.rounded_rectangle((px,py,px+96,py+82),radius=2,outline=(187,216,241),width=1)
            else: self.paste_icon(im,'places','folder-pictures',(px+15,py+10),65)
            text(d,(px+48,py+88),names[i],11,(32,59,92),i==self.selected,anchor='mt')
            text(d,(px+48,py+106),f'{12+i*7} elementi',10,(112,140,169),anchor='mt')
            self.hit((px-4,py-4,px+101,py+124),'select:'+str(i))
        # detail pane
        rx=grid_end+4
        d.rectangle((rx,sidebar_y,x+w-12,foot),fill=(244,249,254))
        if self.thumb[self.selected] is not None:
            im.paste(self.thumb[self.selected].resize((198,146),Image.Resampling.LANCZOS),(rx+9,sidebar_y+12))
        label=names[self.selected]
        text(d,(rx+10,sidebar_y+170),label,16,(20,40,66),True)
        text(d,(rx+10,sidebar_y+201),'Cartella · 28 elementi',11,(95,122,153))
        d.line((rx+10,sidebar_y+227,x+w-18,sidebar_y+227),fill=(218,229,238),width=1)
        for i,(k,v) in enumerate([('Data modifica','26 apr 2026'),('Dimensione','1,4 GB'),('Tipo','Cartella'),('Percorso','/home/utente/Immagini')]):
            yy=sidebar_y+241+i*30
            text(d,(rx+10,yy),k,10,(109,131,158))
            text(d,(rx+105,yy),v,10,(53,76,102))
        text(d,(rx+10,sidebar_y+369),'Etichette',11,(94,123,149))
        for c in [(238,84,114),(148,80,208),(33,161,251),(16,178,155)]:
            i=[(238,84,114),(148,80,208),(33,161,251),(16,178,155)].index(c)
            d.ellipse((rx+102+i*22,sidebar_y+367,rx+115+i*22,sidebar_y+380),fill=c)
        d.rectangle((x+10,foot,x+w-11,y+h-10),fill=(234,244,254))
        text(d,(x+22,foot+8),'8 elementi  •  Vista icone grandi  •  Anteprima abilitata',11,(94,125,157))

    def draw_settings(self,im,d):
        if not self.settings_open:return
        x,y,w,h=675,118,585,444
        self.glass(im,(x,y,x+w,y+h),14,rgba=(23,78,122,180))
        rounded(d,(x+7,y+43,x+w-7,y+h-8),8,fill=(247,251,255))
        self.paste_icon(im,'apps','preferences-system',(x+14,y+8),24)
        text(d,(x+47,y+14),'Impostazioni',14,'white',True)
        text(d,(x+w-25,y+11),'×',18,'white',True)
        self.hit((x+w-46,y+7,x+w,y+38),'close-settings')
        rounded(d,(x+9,y+52,x+167,y+h-14),6,fill=(230,242,254))
        for i,s in enumerate(('Sistema','Bluetooth','Aspetto','Schermi','Applicazioni','Privacy')):
            yy=y+80+i*47
            if s=='Aspetto':rounded(d,(x+18,yy-6,x+157,yy+33),5,fill=(171,215,253))
            text(d,(x+31,yy),s,13,(34,76,120),s=='Aspetto')
        text(d,(x+195,y+79),'Aspetto',21,(25,51,90),True)
        text(d,(x+195,y+118),'Personalizzazione del desktop e delle finestre',12,(105,134,159))
        for i,(head,value) in enumerate([('Tema', 'Melania Aero Classic'),('Trasparenza delle finestre','70%'),('Effetti e animazioni','Attivi'),('Angoli delle finestre','Arrotondati')]):
            yy=y+172+i*50
            rounded(d,(x+190,yy,x+w-22,yy+43),8,fill=(255,255,255),outline=(221,232,242))
            text(d,(x+204,yy+7),head,13,(44,69,105))
            text(d,(x+w-40,yy+9),value,11,(33,128,213),anchor='rt')

    def draw_taskbar(self,im,d):
        y=805
        self.glass(im,(0,y,W,H),radius=0,rgba=(5,20,41,205),border=(165,213,250,120),blur=15)
        d.line((0,y+1,W,y+1),fill=(120,190,243,180),width=1)
        rounded(d,(13,y+6,65,H-7),12,fill=(29,82,144,110),outline=(71,140,219,100))
        logo=self.logo.resize((45,45),Image.Resampling.LANCZOS)
        im.paste(logo,(18,y+7),logo)
        self.hit((5,y,W//13,y+57),'start')
        for i,(cat,n,act) in enumerate([('apps','file-manager','files'),('apps','microsoft-edge','previews'),('apps','firefox','previews'),('apps','utilities-terminal','previews'),('apps','preferences-system','settings')]):
            xx=77+i*59
            if i==0: rounded(d,(xx,y+6,xx+49,y+50),9,fill=(50,114,171,135),outline=(105,167,223,120))
            self.paste_icon(im,cat,n,(xx+8,y+9),37)
            self.hit((xx,y+4,xx+53,y+51),act)
        for i in range(4):
            xx=466+i*30
            rounded(d,(xx,y+17,xx+25,y+43),6,fill=(44,115,187,210) if i+1==self.workspace else (48,71,105,155))
            text(d,(xx+13,y+21),str(i+1),11,'white',True,anchor='mt')
            self.hit((xx,y+17,xx+26,y+44),'workspace:'+str(i+1))
        # tray
        # Small tray glyphs rendered as vectors so unsupported font symbols do not appear as boxes.
        d.line((1230,y+30,1235,y+25,1240,y+30),fill=(222,240,253),width=2)
        d.arc((1250,y+19,1269,y+39),195,345,fill=(222,240,253),width=2)
        d.ellipse((1258,y+32,1262,y+36),fill=(222,240,253))
        rounded(d,(1280,y+20,1297,y+37),4,fill=(36,112,169,180),outline=(161,217,255))
        d.line((1285,y+27,1292,y+27),fill=(238,250,255),width=2)
        d.polygon([(1311,y+26),(1316,y+26),(1322,y+21),(1322,y+37),(1316,y+32),(1311,y+32)],fill=(231,244,255))
        d.arc((1318,y+22,1330,y+38),-65,65,fill=(223,243,255),width=2)
        d.line((1342,y+23,1349,y+30,1342,y+37),fill=(223,243,255),width=2)
        self.hit((1218,y+6,1392,H),'quick')
        text(d,(1488,y+11),dt.datetime.now().strftime('%H:%M'),14,'white',True,anchor='rt')
        text(d,(1488,y+31),dt.datetime.now().strftime('%d/%m/%Y'),12,(190,215,239),anchor='rt')
        self.hit((1398,y+6,W-2,H-1),'calendar')

    def draw_start(self, im,d):
        if not self.start_open:return
        x,y,w,h=82,429,364,371
        self.glass(im,(x,y,x+w,y+h),radius=16,rgba=(5,23,43,225),border=(111,179,234,130),blur=13)
        rounded(d,(x+15,y+14,x+w-15,y+54),12,fill=(38,69,103,210),outline=(98,141,180,100))
        text(d,(x+29,y+24),'⌕  Cerca applicazioni, file, impostazioni...',11,(182,210,232))
        d.line((x+183,y+65,x+183,y+h-43),fill=(73,114,151,170))
        names=[('apps','microsoft-edge','Browser web'),('apps','file-manager','File'),('apps','utilities-terminal','Terminale'),('apps','internet-mail','Posta'),('apps','preferences-system','Impostazioni'),('apps','audio-player','Musica'),('apps','video-player','Video'),('apps','image-viewer','Foto'),('apps','applications-office','Ufficio'),('apps','applications-graphics','Grafica')]
        for i,(cat,ic,lab) in enumerate(names):
            iy=y+66+i*26
            if i==0:rounded(d,(x+12,iy-2,x+181,iy+25),6,fill=(26,90,142,160))
            self.paste_icon(im,cat,ic,(x+17,iy),21)
            text(d,(x+47,iy+3),lab,12,(230,242,255))
            self.hit((x+15,iy,x+180,iy+24),'files' if lab=='File' else 'settings' if lab=='Impostazioni' else 'previews')
        text(d,(x+200,y+71),'Recenti',12,(223,239,252),True)
        for i,(cat,ic,lab) in enumerate([('places','folder-documents','Documenti'),('places','folder-pictures','Immagini'),('places','folder-download','Download'),('places','folder-music','Musica'),('places','folder-videos','Video')]):
            iy=y+97+i*28
            self.paste_icon(im,cat,ic,(x+198,iy),18)
            text(d,(x+225,iy+2),lab,11,(204,229,248))
            self.hit((x+195,iy,x+w-14,iy+24),'files')
        d.line((x+197,y+248,x+w-18,y+248),fill=(73,114,151,165))
        for i,(ic,lab) in enumerate([('computer','Computer'),('network','Rete'),('preferences-system','Dispositivi')]):
            iy=y+260+i*26
            self.paste_icon(im,'places' if ic!='preferences-system' else 'apps',ic,(x+198,iy),18)
            text(d,(x+225,iy+2),lab,11,(213,237,255))
        rounded(d,(x+180,y+h-37,x+w-12,y+h-8),6,fill=(24,52,84,210),outline=(71,115,155,100))
        text(d,(x+192,y+h-30),'⏻  Arresta il sistema',12,(236,245,255))
        text(d,(x+21,y+h-27),'Melania Desktop',11,(169,202,228))

    def draw_previews(self,im,d):
        if not self.preview_windows or not self.file_open: return
        for i,(lab,ic) in enumerate([('Browser web','microsoft-edge'),('Immagini','folder-pictures'),('Documenti','folder-documents')]):
            xx=505+i*191; yy=673
            self.glass(im,(xx,yy,xx+180,yy+120),12,rgba=(10,32,55,223),blur=10)
            self.paste_icon(im,'apps' if i==0 else 'places',ic,(xx+8,yy+6),20)
            text(d,(xx+35,yy+8),lab,12,(242,248,255))
            text(d,(xx+163,yy+7),'×',13,(240,247,255))
            rounded(d,(xx+8,yy+33,xx+172,yy+110),4,fill=(244,250,255))
            if self.thumb[i] is not None:
                im.paste(self.thumb[i].resize((151,65),Image.Resampling.LANCZOS),(xx+15,yy+39))
        self.hit((505,673,1072,793),'previews')

    def draw_quick(self,im,d):
        if not self.quick_open: return
        x,y,w,h=1213,14,306,434
        self.glass(im,(x,y,x+w,y+h),14,rgba=(6,27,48,226),blur=14)
        rounded(d,(x+12,y+9,x+148,y+40),9,fill=(24,118,205,235),outline=(121,189,244,180))
        rounded(d,(x+152,y+9,x+w-13,y+40),9,fill=(26,52,78,170),outline=(71,109,146,120))
        text(d,(x+80,y+17),'Notifiche',12,'white',True,anchor='mt')
        text(d,(x+219,y+17),'Area rapida',12,(198,221,239),anchor='mt')
        for i,(ic,heading,descr,when) in enumerate([('preferences-system','Aggiornamenti disponibili','3 aggiornamenti pronti','10:24'),('internet-mail','Nuovo messaggio','Hai ricevuto una nuova email','09:18'),('image-viewer','Screenshot salvato','Immagine salvata','08:51')]):
            iy=y+53+i*55
            rounded(d,(x+9,iy-3,x+w-9,iy+48),8,fill=(21,52,79,160),outline=(65,106,145,70))
            self.paste_icon(im,'apps',ic,(x+16,iy+6),29)
            text(d,(x+57,iy+3),heading,12,(238,248,255),True)
            text(d,(x+57,iy+23),descr,10,(166,201,228))
            text(d,(x+w-14,iy+3),when,10,(132,168,199),anchor='rt')
        text(d,(x+41,y+223),'Non disturbare',11,(225,240,252))
        rounded(d,(x+11,y+221,x+31,y+232),6,fill=(61,103,143))
        rounded(d,(x+w-106,y+214,x+w-13,y+241),7,fill=(29,65,94),outline=(77,122,161,130))
        text(d,(x+w-60,y+219),'Cancella tutto',10,(214,236,252),anchor='mt')
        buttons=[('Wi-Fi','⌁'),('Bluetooth','ᛒ'),('Non disturbare','☾'),('Risparmio energia','♧'),('Modalità scura','◐')]
        for i,(label,glyph) in enumerate(buttons):
            col=i%2 if i<2 else i-2; row=0 if i<2 else 1
            xx=x+12+col*(w-24)//2 if row==0 else x+12+col*92
            ww=(w-33)//2 if row==0 else 86
            yy=y+250 if row==0 else y+314
            checked=self.toggles[label]
            rounded(d,(xx,yy,xx+ww,yy+(57 if row==0 else 53)),9,fill=(16,128,220,235) if checked else (35,67,95,210),outline=(86,165,231,135))
            cx=xx+ww//2; cy=yy+19
            if label=='Wi-Fi':
                d.arc((cx-17,cy-12,cx+17,cy+13),190,350,fill=(247,251,255),width=3)
                d.arc((cx-10,cy-5,cx+10,cy+11),192,348,fill=(247,251,255),width=3)
                d.ellipse((cx-2,cy+8,cx+2,cy+12),fill=(247,251,255))
            elif label=='Bluetooth':
                d.line((cx,cy-13,cx,cy+13),fill=(247,251,255),width=3)
                d.line((cx-8,cy-6,cx+8,cy+7,cx,cy+13,cx,cy-13,cx+8,cy-7,cx-8,cy+7),fill=(247,251,255),width=2)
            elif label=='Non disturbare' or label=='Modalità scura':
                d.ellipse((cx-10,cy-10,cx+10,cy+10),fill=(248,252,255))
                d.ellipse((cx-3,cy-15,cx+16,cy+7),fill=(35,67,95) if not checked else (16,128,220))
            else:
                d.ellipse((cx-13,cy-11,cx+13,cy+9),fill=(241,252,255))
                d.line((cx-6,cy+10,cx+8,cy-9),fill=(26,128,179),width=2)
            text(d,(xx+ww//2,yy+34),label,10,(246,252,255),anchor='mt')
            self.hit((xx,yy,xx+ww,yy+(57 if row==0 else 53)),'toggle:'+label)
        for i,lab in enumerate(('☼','◖')):
            yy=y+383+i*24
            if i==0:
                d.ellipse((x+17,yy+2,x+27,yy+12),fill=(238,248,255))
                for dx,dy in ((0,-9),(0,9),(-9,0),(9,0)):
                    d.line((x+22+dx//2,yy+7+dy//2,x+22+dx,yy+7+dy),fill=(238,248,255),width=2)
            else:
                d.polygon([(x+14,yy+4),(x+19,yy+4),(x+25,yy-2),(x+25,yy+18),(x+19,yy+12),(x+14,yy+12)],fill=(238,248,255))
                d.arc((x+19,yy-1,x+33,yy+19),-65,65,fill=(238,248,255),width=2)
            rounded(d,(x+43,yy+2,x+w-15,yy+14),6,fill=(18,42,61,245))
            rounded(d,(x+44,yy+3,x+215+i*28,yy+13),5,fill=(28,156,243))
            d.ellipse((x+211+i*28,yy-1,x+227+i*28,yy+17),fill=(122,218,255),outline=(236,248,255))

    def draw_calendar(self,im,d):
        if not self.calendar_open:return
        x,y,w,h=1213,459,306,336
        self.glass(im,(x,y,x+w,y+h),14,rgba=(8,29,50,226),blur=13)
        now=dt.datetime.now(); month=now.month; year=now.year
        text(d,(x+15,y+15),now.strftime('%B %Y').capitalize(),15,(245,251,255),True)
        text(d,(x+185,y+13),'‹      ›',20,(193,227,248))
        days=['L','M','M','G','V','S','D']
        for c,l in enumerate(days):text(d,(x+19+c*27,y+56),l,10,(181,207,228),True)
        for r,week in enumerate(calendar.monthcalendar(year,month)):
            for c,day in enumerate(week):
                if day<=0:continue
                xx=x+16+c*27;yy=y+82+r*28
                if day==now.day:
                    d.ellipse((xx-5,yy-2,xx+22,yy+23),fill=(21,134,238))
                text(d,(xx+8,yy+2),str(day),11,(250,252,255),day==now.day,anchor='mt')
        d.line((x+209,y+48,x+209,y+h-14),fill=(82,126,162,150))
        # analog clock
        cx,cy=x+255,y+118
        d.ellipse((cx-37,cy-37,cx+37,cy+37),fill=(17,51,82),outline=(148,210,251),width=2)
        for i in range(12):
            import math
            a=(i-3)*math.pi/6
            px=cx+30*math.cos(a);py=cy+30*math.sin(a)
            d.ellipse((px-1.4,py-1.4,px+1.4,py+1.4),fill=(215,236,251))
        d.line((cx,cy,cx+14,cy+6),fill=(240,250,255),width=2)
        d.line((cx,cy,cx-2,cy-24),fill=(240,250,255),width=2)
        d.ellipse((cx-3,cy-3,cx+3,cy+3),fill=(242,250,255))
        text(d,(cx,y+181),now.strftime('%A'),12,'white',True,anchor='mt')
        text(d,(cx,y+198),now.strftime('%d %B %Y'),10,(192,222,244),anchor='mt')
        rounded(d,(x+211,y+h-47,x+w-13,y+h-15),8,fill=(40,83,121,170))
        text(d,(x+255,y+h-40),'Eventi oggi  +',11,'white',anchor='mt')

    def render(self):
        self.hits=[]
        im=self.canvas_bg()
        d=ImageDraw.Draw(im)
        self.draw_desktop(im,d)
        self.draw_file(im,d)
        self.draw_settings(im,d)
        self.draw_previews(im,d)
        self.draw_taskbar(im,d)
        self.draw_start(im,d)
        self.draw_quick(im,d)
        self.draw_calendar(im,d)
        self.preview=ImageTk.PhotoImage(im.convert('RGB'))
        self.canvas.delete('all')
        self.canvas.create_image(0,0,anchor='nw',image=self.preview)


def main():
    p=argparse.ArgumentParser()
    p.add_argument('--log',default=None)
    args=p.parse_args()
    root=tk.Tk(); MelaniaLive(root,args.log)
    root.mainloop()

if __name__=='__main__':main()
