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
from PIL import Image, ImageDraw, ImageFont, ImageFilter, ImageOps, ImageTk

BASE = Path(__file__).resolve().parent
THEME = BASE / 'assets' / 'Melania-Aero-Classic'
LOGO = BASE / 'assets' / 'melania-128.png'
WALL = BASE / 'assets' / 'wallpaper-aero.png'
REFERENCE = BASE / 'assets' / 'reference.png'
SHELL_ICON = BASE / 'assets' / 'shell-icons'
W, H = 1536, 864
TASKBAR_TOP = H - 52
MONTHS_IT = ('gennaio','febbraio','marzo','aprile','maggio','giugno','luglio','agosto','settembre','ottobre','novembre','dicembre')
DAYS_IT = ('lunedì','martedì','mercoledì','giovedì','venerdì','sabato','domenica')
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


def ellipsize(d, label: str, max_width: int, size: int=11) -> str:
    """Ellipsize captions by their *rendered* width, not arbitrary length."""
    f=font(size)
    if d.textbbox((0,0),label,font=f)[2] <= max_width:
        return label
    while label and d.textbbox((0,0),label+'…',font=f)[2] > max_width:
        label=label[:-1]
    return label+'…'


def file_columns(window_w: int, sidebar_w=165, detail_w=224, tile_w=118, gap=12):
    """Pure size-dependent tile layout; all coordinates are relative to window."""
    content_width = window_w - 22 - sidebar_w - 4 - detail_w - 4
    usable = content_width - 22
    count = max(1, (usable + gap) // (tile_w + gap))
    return int(count), int(content_width)


class MelaniaLive:
    def __init__(self, root, logpath=None, width=1536, height=864):
        self.root = root
        self.root.title('Melania — Aero UX Lab (Tk/X11, non compositore Wayland)')
        self.width, self.height = width, height
        self.taskbar_top = height - 52
        root.geometry(f'{width}x{height}+0+0')
        root.minsize(800, 600)
        root.resizable(True, True)
        self.canvas = tk.Canvas(root, width=width, height=height, highlightthickness=0)
        self.canvas.pack(fill='both', expand=True)
        self.icons = {}
        self.shell_cache = {}
        self.nav_path = ['Home','Immagini','Viaggi']
        self.file_tabs = [
            {'title':'Home','path':['Home']},
            {'title':'Immagini','path':['Home','Immagini']},
            {'title':'Viaggi','path':['Home','Immagini','Viaggi']},
        ]
        self.active_tab = 2
        self.tab_offset = 0  # first visible tab when the strip overflows
        self.tab_scroll_manual = False
        self.tab_geometry = []  # (index, x1, y1, x2, y2), for drag and close
        self.tab_drag = None  # (original index, press_x, press_y, current_x, dragging)
        self.tab_drop_slot = None
        self.file_names=['Lago di Como','Venezia','Firenze','Dolomiti','Cinque Terre','Costiera Amalfitana','Tramonto','Borghi d’Italia']
        self.file_counts=[12,19,26,33,40,47,54,61]  # demo data shared by list, tiles and inspector
        self.file_sort='Nome'
        self.file_reverse=False
        self.action_rects={}
        self.file_view='griglia'
        self.details_enabled=True
        self.file_minimized=False
        self.file_maximized=False
        self.file_preview_source=None
        self.toast_message=''
        self.file_search_rect=None
        self.search_rect=None
        self.nav_cursor = 2
        self.start_rect = None
        self.tray_rect = None
        self.clock_rect = None
        self.thumb = [photo_thumb(i) for i in range(8)]
        # Use the larger Venice preview already supplied in the approved
        # reference; avoid enlarging a tiny navigation thumbnail 3x.
        self.detail_venice = None
        if REFERENCE.is_file():
            self.detail_venice = Image.open(REFERENCE).convert('RGB').crop((1069,127,1268,271))
        self.logo = Image.open(LOGO).convert('RGBA')
        self.wall_source = Image.open(WALL).convert('RGB')
        self.wall = ImageOps.fit(self.wall_source,(self.width,self.height), method=Image.Resampling.LANCZOS)
        self.hits = []
        self.preview = None
        self.start_open = True
        self.quick_open = False
        self.calendar_open = False
        self.file_open = True
        self.settings_open = False
        self.selected = 1
        self.category = 'Immagini'
        self.workspace = 1
        self.preview_windows = False
        self.quick_tab = 'Notifiche'
        self.calendar_month_offset = 0
        self.notices = [
            ('preferences-system','Aggiornamenti disponibili','3 aggiornamenti pronti','10:24'),
            ('internet-mail','Nuovo messaggio','Hai ricevuto una nuova email','09:18'),
            ('image-viewer','Screenshot salvato','Immagine salvata','08:51'),
        ]
        self.hovered_window = None
        self.hovered_menu = None
        self.file_layout = {}
        self.tile_rects = []
        self.toggles = {'Wi-Fi': True, 'Bluetooth': True, 'Non disturbare': False, 'Risparmio energia': True, 'Modalità scura': False}
        # Real, keyboard-editable Tk controls placed over the raster UI.
        # The search boxes work in the executable prototype, not just as art.
        self.start_placeholder='Cerca applicazioni, file, impostazioni...'
        self.file_placeholder='Cerca in Viaggi...'
        self.start_search_var=tk.StringVar(value=self.start_placeholder)
        self.file_search_var=tk.StringVar(value=self.file_placeholder)
        self.start_search=tk.Entry(root, textvariable=self.start_search_var,
                                   bg='#26496d', fg='#f2f8ff', insertbackground='white',
                                   relief='flat', borderwidth=0, font=('DejaVu Sans', 10))
        self.file_search=tk.Entry(root, textvariable=self.file_search_var,
                                  bg='#ffffff', fg='#233f61', insertbackground='#185ca4',
                                  relief='flat', borderwidth=0, font=('DejaVu Sans', 10))
        self.start_search_var.trace_add('write', lambda *_: self.render())
        self.file_search_var.trace_add('write', lambda *_: self.render())
        self.start_search.config(fg='#bdcfe2')
        self.file_search.config(fg='#7e93ac')
        self.start_search.bind('<FocusIn>', lambda _: self.entry_focus_in(self.start_search,self.start_search_var,self.start_placeholder))
        self.file_search.bind('<FocusIn>', lambda _: self.entry_focus_in(self.file_search,self.file_search_var,self.file_placeholder))
        self.start_search.bind('<FocusOut>', lambda _: self.entry_focus_out(self.start_search,self.start_search_var,self.start_placeholder))
        self.file_search.bind('<FocusOut>', lambda _: self.entry_focus_out(self.file_search,self.file_search_var,self.file_placeholder))
        self.start_search.bind('<Escape>', self.close_popovers)
        self.file_search.bind('<Escape>', lambda e: (self.file_search_var.set(''), self.canvas.focus_set()))
        self.start_search.bind('<Return>', lambda _: self.select_first_search_result())
        self.logpath = Path(logpath) if logpath else BASE/'events.jsonl'
        self.logpath.parent.mkdir(parents=True, exist_ok=True)
        root.bind('<ButtonPress-1>', self.on_mouse_down)
        root.bind('<B1-Motion>', self.on_mouse_drag)
        root.bind('<ButtonRelease-1>', self.on_mouse_up)
        root.bind('<Button-2>', self.on_middle_click)
        root.bind('<Control-t>', lambda e: self.action('file-tab-new'))
        root.bind('<Control-w>', lambda e: self.action('file-tab-close:'+str(self.active_tab)))
        root.bind('<Control-Shift-Tab>', lambda e: self.action('file-tab-prev'))
        root.bind('<Control-Tab>', lambda e: self.action('file-tab-next'))
        root.bind('<Escape>', self.close_popovers)
        root.bind('<Control-e>', lambda _ : self.action('files'))
        root.bind('<Super_L>', lambda _ : self.action('start'))
        root.bind('<F5>', lambda _ : self.render())
        root.bind('<Control-f>', lambda _: self.focus_file_search())
        root.bind('<Control-l>', lambda _: self.focus_file_search())
        root.bind('<Control-Shift-n>', lambda _: self.action('details-toggle'))
        root.bind('<Control-Shift-v>', lambda _: self.action('view-toggle'))
        root.bind('<Motion>', self.on_motion)
        root.bind('<Configure>', self.on_resize)
        root.bind('<Alt-Left>', lambda _ : self.action('nav:back'))
        root.bind('<Alt-Right>', lambda _ : self.action('nav:forward'))
        root.bind('<Alt-Up>', lambda _ : self.action('nav:up'))
        self.render()

    @staticmethod
    def entry_focus_in(widget,var,placeholder):
        if var.get()==placeholder:
            var.set('')
            widget.config(fg='#203d5f' if widget.cget('bg')=='#ffffff' else '#f2f8ff')

    @staticmethod
    def entry_focus_out(widget,var,placeholder):
        if not var.get().strip():
            var.set(placeholder)
            widget.config(fg='#7e93ac' if widget.cget('bg')=='#ffffff' else '#bdcfe2')

    def start_query(self):
        q=self.start_search_var.get().strip()
        return '' if q==self.start_placeholder else q.casefold()

    def file_query(self):
        q=self.file_search_var.get().strip()
        return '' if q==self.file_placeholder else q.casefold()

    def focus_file_search(self):
        if not self.file_open: self.action('files')
        self.file_search.focus_set()
        self.file_search.select_range(0,'end')

    def select_first_search_result(self):
        q=self.start_query()
        if not q: return
        for label,action in self.start_programs():
            if q in label.casefold():
                self.action(action)
                return

    def start_programs(self):
        return [(lab,act) for _,_,lab,act in self.start_items()]

    @staticmethod
    def start_items():
        return [('apps','microsoft-edge','Browser web','demo:Browser web'),
                ('apps','file-manager','File','files'),
                ('apps','utilities-terminal','Terminale','demo:Terminale'),
                ('apps','internet-mail','Posta','demo:Posta'),
                ('apps','preferences-system','Impostazioni','settings'),
                ('apps','audio-player','Musica','demo:Musica'),
                ('apps','video-player','Video','demo:Video'),
                ('apps','image-viewer','Foto','demo:Foto'),
                ('apps','applications-office','Ufficio','demo:Ufficio'),
                ('apps','graphics-editor','Grafica','demo:Grafica')]

    def on_resize(self, event):
        if event.widget is not self.root: return
        new_width, new_height = event.width, event.height
        if new_width < 800 or new_height < 600: return
        if (new_width,new_height)==(self.width,self.height): return
        self.width,self.height=new_width,new_height
        self.taskbar_top=new_height-52
        self.wall=ImageOps.fit(self.wall_source,(new_width,new_height),method=Image.Resampling.LANCZOS)
        self.canvas.config(width=new_width,height=new_height)
        self.render()

    def shell_icon(self, name, size=20, variant='toolbar'):
        key=(variant,name,size)
        if key not in self.shell_cache:
            path=SHELL_ICON/variant/f'{name}-{size}.png'
            if not path.exists():
                # pick 24 px rather than falling back to font glyphs
                path=SHELL_ICON/variant/f'{name}-24.png'
            self.shell_cache[key]=Image.open(path).convert('RGBA').resize((size,size),Image.Resampling.LANCZOS)
        return self.shell_cache[key]

    def paste_shell(self, image, name, xy, size=20, variant='toolbar'):
        icon=self.shell_icon(name,size,variant)
        image.paste(icon,xy,icon)

    def log(self, action):
        with self.logpath.open('a', encoding='utf-8') as f:
            f.write(json.dumps({'event':action, 'at':dt.datetime.now().isoformat()},ensure_ascii=False)+'\n')

    def hit(self, xy, action): self.hits.append((xy,action))

    def on_motion(self, e):
        # Show only the preview of the real hovered taskbar application.
        over_taskbar = self.taskbar_top <= e.y < self.height and 76 <= e.x <= 130
        over_preview = (self.preview_windows and self.file_open and
                        77 <= e.x <= 300 and self.taskbar_top-144 <= e.y < self.taskbar_top-12)
        hover = 'files' if (self.file_open or self.file_minimized) and (over_taskbar or over_preview) else None
        menu_hover = None
        if self.start_open and self.start_rect is not None:
            sx,sy,ex,ey=self.start_rect
            if sx+14<=e.x<=sx+183 and sy+64<=e.y<=ey-45:
                menu_hover = min(9,max(0,(e.y-sy-66)//26))
        if self.hovered_window != hover or self.hovered_menu != menu_hover:
            self.hovered_window = hover
            self.hovered_menu = menu_hover
            self.preview_windows = hover is not None
            self.render()

    def tab_hit(self, x, y):
        for i,x1,y1,x2,y2 in self.tab_geometry:
            if x1 <= x <= x2 and y1 <= y <= y2:
                return i, (x >= x2-27)
        return None, False

    def on_mouse_down(self, e):
        index, on_close = self.tab_hit(e.x, e.y)
        if index is not None:
            if on_close:
                self.action(f'file-tab-close:{index}')
            else:
                self.tab_drag = (index, e.x, e.y, e.x, False)
            return
        self.click(e)

    def on_mouse_drag(self, e):
        if self.tab_drag is None:
            return
        index, start_x, start_y, _, was_dragging = self.tab_drag
        moving = was_dragging or abs(e.x-start_x) > 9
        self.tab_drag = (index, start_x, start_y, e.x, moving)
        self.tab_drop_slot = self.tab_insertion_index(e.x) if moving else None
        if moving:
            self.render()

    def on_mouse_up(self, e):
        if self.tab_drag is None:
            return
        index, _, _, _, was_dragging = self.tab_drag
        self.tab_drag = None
        self.tab_drop_slot = None
        if was_dragging:
            destination = self.tab_insertion_index(e.x)
            # Insertion slots are between tabs; take removal into account.
            if destination > index:
                destination -= 1
            destination = max(0, min(destination, len(self.file_tabs)-1))
            self.move_tab(index, destination)
        else:
            self.action(f'file-tab:{index}')
        self.render()

    def on_middle_click(self, e):
        index, _ = self.tab_hit(e.x, e.y)
        if index is not None:
            self.action(f'file-tab-close:{index}')

    def tab_insertion_index(self, x):
        for i,x1,_,x2,_ in self.tab_geometry:
            if x < (x1+x2)/2:
                return i
        return self.tab_geometry[-1][0]+1 if self.tab_geometry else len(self.file_tabs)

    def move_tab(self, origin, destination):
        if not (0 <= origin < len(self.file_tabs) and 0 <= destination < len(self.file_tabs)):
            return
        item = self.file_tabs.pop(origin)
        self.file_tabs.insert(destination, item)
        if self.active_tab == origin:
            self.active_tab = destination
        elif origin < self.active_tab <= destination:
            self.active_tab -= 1
        elif destination <= self.active_tab < origin:
            self.active_tab += 1
        self.show_active_tab()

    def show_active_tab(self):
        # Autoscroll the tab rail when selection changes. The exact visible
        # capacity is recomputed during drawing, so no fixed 4-tab ceiling.
        self.tab_scroll_manual = False
        self.tab_offset = max(0, min(self.tab_offset, self.active_tab))

    def select_file_tab(self, index):
        if not (0 <= index < len(self.file_tabs)):
            return
        self.active_tab = index
        self.nav_path = list(self.file_tabs[index]['path'])
        self.nav_cursor = len(self.nav_path)-1
        self.show_active_tab()
        self.file_placeholder = f"Cerca in {self.nav_path[-1]}..."
        self.file_search_var.set(self.file_placeholder)
        self.file_search.config(fg='#7e93ac')

    def new_file_tab(self):
        self.file_tabs.append({'title':'Home', 'path':['Home']})
        self.select_file_tab(len(self.file_tabs)-1)
        self.tab_offset = max(0, len(self.file_tabs)-3)

    def close_file_tab(self, index):
        if not (0 <= index < len(self.file_tabs)):
            return
        if len(self.file_tabs) == 1:
            # Keep one usable Home tab instead of closing the whole window.
            self.file_tabs[0] = {'title':'Home','path':['Home']}
            self.select_file_tab(0)
            return
        self.file_tabs.pop(index)
        if self.active_tab == index:
            self.active_tab = min(index, len(self.file_tabs)-1)
        elif self.active_tab > index:
            self.active_tab -= 1
        self.tab_offset = min(self.tab_offset, max(0,len(self.file_tabs)-1))
        self.select_file_tab(self.active_tab)

    def click(self, e):
        for (x1,y1,x2,y2), action in reversed(self.hits):
            if x1 <= e.x <= x2 and y1 <= e.y <= y2:
                self.action(action); return
        if self.start_open or self.quick_open or self.calendar_open:
            self.close_popovers()

    def close_popovers(self, _=None):
        self.start_open=False
        self.quick_open=False
        self.calendar_open=False
        self.preview_windows=False
        self.hovered_window=None
        self.render()

    def action(self, action):
        self.log(action)
        if action in ('start','quick','calendar'):
            opening=not getattr(self, action+'_open')
            self.start_open=False; self.quick_open=False; self.calendar_open=False
            setattr(self, action+'_open', opening)
            self.preview_windows=False
            self.hovered_window=None
        elif action.startswith('quick-tab:'):
            self.quick_tab=action.split(':',1)[1]
            self.quick_open=True
            self.start_open=self.calendar_open=False
        elif action == 'file-tab-new':
            self.new_file_tab()
        elif action.startswith('file-tab-close:'):
            self.close_file_tab(int(action.split(':',1)[1]))
        elif action == 'file-tab-next':
            self.select_file_tab((self.active_tab+1) % len(self.file_tabs))
        elif action == 'file-tab-prev':
            self.select_file_tab((self.active_tab-1) % len(self.file_tabs))
        elif action == 'file-tab-scroll-left':
            self.tab_scroll_manual = True
            self.tab_offset = max(0, self.tab_offset-1)
        elif action == 'file-tab-scroll-right':
            self.tab_scroll_manual = True
            self.tab_offset = min(len(self.file_tabs)-1, self.tab_offset+1)
        elif action.startswith('file-tab:'):
            self.select_file_tab(int(action.split(':',1)[1]))
        elif action=='files':
            self.file_open=True
            self.file_minimized=False
            self.start_open=False
        elif action=='settings':
            self.settings_open=True
            self.start_open=False
        elif action=='previews':
            self.preview_windows=not self.preview_windows
        elif action=='view-toggle':
            self.file_view='lista' if self.file_view=='griglia' else 'griglia'
        elif action=='details-toggle':
            self.details_enabled=not self.details_enabled
        elif action=='sort-toggle':
            self.file_reverse=not self.file_reverse
        elif action=='file-action-info':
            self.toast_message='Comandi reali disponibili in Melania Files GTK (da collaudare)'
        elif action=='file-min':
            self.file_minimized=True
            self.file_open=False
            self.preview_windows=False
        elif action=='file-max':
            self.file_maximized=not self.file_maximized
        elif action.startswith('demo:'):
            name=action.split(':',1)[1]
            if name=='Nuova scheda':
                self.new_file_tab()
            else:
                self.toast_message=f'{name}: dimostrazione grafica, app non avviata'
                self.start_open=False
        elif action=='clear-notices':
            self.notices.clear()
        elif action=='calendar-prev':
            self.calendar_month_offset-=1
        elif action=='calendar-next':
            self.calendar_month_offset+=1
        elif action.startswith('toggle:'):
            name=action.split(':',1)[1]
            self.toggles[name]=not self.toggles[name]
        elif action.startswith('nav:'):
            direction=action.split(':',1)[1]
            if direction in ('back','up'):
                self.nav_cursor=max(0,self.nav_cursor-1)
            elif direction=='forward':
                self.nav_cursor=min(len(self.nav_path)-1,self.nav_cursor+1)
            elif direction=='home':
                self.nav_cursor=0
            # Keep the active tab in sync with navigation, so the strip behaves like a real browser/file-manager hybrid.
            current_path = self.nav_path[:self.nav_cursor+1]
            if current_path and 0 <= self.active_tab < len(self.file_tabs):
                self.file_tabs[self.active_tab]['path'] = list(current_path)
                self.file_tabs[self.active_tab]['title'] = current_path[-1]
                self.file_placeholder = f'Cerca in {current_path[-1]}...'
                self.file_search_var.set(self.file_placeholder)
            # 'refresh', 'search', 'details' are layout-safe no-op demo actions.
        elif action.startswith('select:'):
            self.selected=int(action.split(':',1)[1])
        elif action.startswith('category:'):
            self.category=action.split(':',1)[1]
        elif action=='workspace-cycle':
            self.workspace=(self.workspace % 4)+1
        elif action.startswith('workspace:'):
            self.workspace=int(action.split(':',1)[1])
            self.close_popovers()
            return
        elif action=='close-file':
            self.file_open=False
            self.file_minimized=False
            self.preview_windows=False
        elif action=='close-settings':
            self.settings_open=False
        elif action=='quit-demo':
            # Only closes the Tk UX lab, never powers off the host.
            self.root.destroy()
            return
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
        # True *prototype* backdrop sampling and blur with an atmospheric upper-edge
        # reflection. No claims about the unfinished Wayland compositor's renderer.
        x1,y1,x2,y2=xy
        if x2<=x1 or y2<=y1: return
        fragment=image.crop((x1,y1,x2,y2)).filter(ImageFilter.GaussianBlur(blur))
        ww,hh=x2-x1,y2-y1
        mask=Image.new('L',(ww,hh))
        ImageDraw.Draw(mask).rounded_rectangle((0,0,ww-1,hh-1),radius=radius,fill=255)
        image.paste(fragment,(x1,y1),mask)
        overlay=Image.new('RGBA',(ww,hh),(0,0,0,0))
        od=ImageDraw.Draw(overlay)
        od.rounded_rectangle((0,0,ww-1,hh-1),radius=radius,fill=rgba,outline=border,width=1)
        # Reflections stay subtle; glass must not become opaque or cartoonish.
        shine_height=min(72,hh)
        for iy in range(1,shine_height):
            a=int(30*((1-iy/shine_height)**2))
            if a:
                od.line((5,iy,ww-6,iy),fill=(222,244,255,a),width=1)
        if ww>8:
            od.line((7,2,ww-8,2),fill=(233,249,255,85),width=1)
        image.alpha_composite(overlay,(x1,y1))

    def canvas_bg(self):
        im=Image.new('RGBA',(self.width,self.height))
        # Overall palette tuned towards the blue lake/landscape in the approved image.
        wall=self.wall.convert('RGBA')
        lift=Image.new('RGBA',(self.width,self.height),(12,65,116,20))
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
        w = min(860,max(650,round(self.width*.59)))
        w = min(w,self.width-32)
        x = min(self.width-w-18,max(115,round((self.width-w)*.55)))
        x = max(14,x)
        y = max(30,round(self.height*.061))
        h = min(579,self.taskbar_top-y-65)
        if self.file_maximized:
            x,y,w,h=12,12,self.width-24,self.taskbar_top-24
        # Draw a soft halo behind the native-looking window; retain transparent margins.
        shadow=Image.new('RGBA',im.size,(0,0,0,0))
        sd=ImageDraw.Draw(shadow)
        sd.rounded_rectangle((x-6,y-4,x+w+7,y+h+10),radius=18,fill=(3,16,40,135))
        shadow=shadow.filter(ImageFilter.GaussianBlur(15))
        im.alpha_composite(shadow)
        self.glass(im,(x,y,x+w,y+h),radius=13,rgba=(23,92,143,104),border=(203,234,255,190),blur=17)
        # Approved Aero top tabs; workspace retains a subtle satin-blue tool shelf.
        rounded(d,(x+7,y+38,x+w-7,y+h-7),8,fill=(251,253,255,255))
        rounded(d,(x+13,y+50,x+w-13,y+103),7,fill=(242,248,255,248),outline=(194,219,240,255))
        d.line((x+9,y+39,x+w-10,y+39),fill=(245,252,255,150),width=1)
        # Native-like top tab bar: tabs live directly in the title area,
        # without a separate 'Melania Files' title block.
        for i,name in enumerate(('window-min','window-max','window-close')):
            xx=x+w-119+i*34
            rounded(d,(xx,y+10,xx+32,y+32),5, fill=(94,149,190,132) if i<2 else (201,62,71,235),outline=(223,241,255,170))
            self.paste_shell(im,name,(xx+7,y+13),19,'tray')
        self.hit((x+w-119,y+10,x+w-87,y+36),'file-min')
        self.hit((x+w-85,y+10,x+w-53,y+36),'file-max')
        self.hit((x+w-51,y+10,x+w-8,y+36),'close-file')
        # Browser-native contiguous tabs: inactive tabs form one continuous rail,
        # while only the active tab receives an elevated rounded surface.
        rail_left, rail_top = x+12, y+8
        rail_right = x+w-132
        rail_bottom = y+36
        width_available = rail_right - rail_left
        plus_width, arrows_width, min_width, max_width = 32, 52, 90, 178
        count = len(self.file_tabs)
        # Fit all tabs by reducing their width; only overflow below min_width.
        room_no_arrows = width_available - plus_width - 14
        overflow = count * min_width > room_no_arrows
        room = width_available - plus_width - 16 - (arrows_width if overflow else 0)
        if not overflow:
            tab_w = min(max_width,max(min_width,room//count))
            self.tab_offset = 0
        else:
            tab_w = max(min_width, min(136,room//max(1, min(4, count))))
            capacity = max(1, room//tab_w)
            if not self.tab_scroll_manual:
                if self.active_tab < self.tab_offset:
                    self.tab_offset = self.active_tab
                elif self.active_tab >= self.tab_offset + capacity:
                    self.tab_offset = self.active_tab-capacity+1
            self.tab_offset = max(0,min(self.tab_offset,count-capacity))
        capacity = max(1, room//tab_w)
        first = self.tab_offset
        shown = list(range(first, min(count,first+capacity)))
        rounded(d,(rail_left,rail_top,rail_right,rail_bottom),7,
                fill=(176,213,240,62),outline=(198,233,255,92))
        # Selectors below the tabs are a single uninterrupted rail.
        self.tab_geometry=[]
        for idx in shown:
            tx = rail_left + 3 + (idx-first)*tab_w
            active = idx == self.active_tab
            tw = tab_w
            top = rail_top+1 if active else rail_top+3
            bottom = rail_bottom if active else rail_bottom-3
            if active:
                rounded(d,(tx,top,tx+tw,bottom),8,fill=(250,253,255,255),outline=(189,225,252,255))
                line(d,[(tx+2,bottom),(tx+tw-2,bottom)],fill=(248,252,255,255),width=2)
                line(d,[(tx+11,top+2),(tx+tw-11,top+2)],fill=(57,153,242,210),width=2)
            else:
                # Inactive tabs remain part of one contiguous, translucent blue rail.
                rounded(d,(tx,top,tx+tw,bottom),4,fill=(57,108,155,48))
                if idx > first:
                    line(d,[(tx,top+7),(tx,bottom-7)],fill=(224,243,255,120),width=1)
                if self.tab_drag and self.tab_drag[0] == idx and self.tab_drag[4]:
                    rounded(d,(tx+2,top,tx+tw-2,bottom),7,fill=(200,227,251,230),outline=(64,144,223,220))
            icon_name = 'folder' if self.file_tabs[idx]['path'][-1]=='Home' else 'folder-pictures'
            self.paste_icon(im,'places',icon_name,(tx+11,top+7),16)
            label = ellipsize(d,self.file_tabs[idx]['title'],tw-60,12)
            text(d,(tx+33,top+7),label,12,(23,67,109) if active else (240,248,255),active)
            # The close glyph gets its own 26px hit target, not the whole tab.
            cx=tx+tw-19
            if active:
                rounded(d,(cx-8,top+5,cx+10,top+24),5,fill=(226,240,251,255))
            text(d,(cx,top+6),'×',14,(58,98,134) if active else (221,236,248),False,anchor='mt')
            self.tab_geometry.append((idx,tx,top,tx+tw,bottom))
        # '+' follows the visible tab group with no decorative gap.
        plus_x = rail_left + 3 + len(shown)*tab_w + 3
        plus_x = min(plus_x,rail_right-36-(arrows_width if overflow else 0))
        rounded(d,(plus_x,rail_top+4,plus_x+28,rail_top+28),6,
                fill=(25,70,112,155),outline=(146,196,235,120))
        text(d,(plus_x+14,rail_top+6),'+',17,'white',True,anchor='mt')
        self.hit((plus_x,rail_top+4,plus_x+28,rail_top+28),'file-tab-new')
        if overflow:
            bx = rail_right-53
            for dx,sign,action,enabled in ((0,-1,'file-tab-scroll-left',first>0),
                                           (25,1,'file-tab-scroll-right',first+len(shown)<count)):
                rounded(d,(bx+dx,rail_top+5,bx+dx+23,rail_top+27),5,
                        fill=(235,246,255,240) if enabled else (216,230,242,125),
                        outline=(167,193,216,175))
                cx=bx+dx+11
                coords=[(cx+4*sign,rail_top+11),(cx-3*sign,rail_top+16),(cx+4*sign,rail_top+21)]
                d.line(coords,fill=(36,90,145,255) if enabled else (141,162,182,170),width=2)
                if enabled:
                    self.hit((bx+dx,rail_top+5,bx+dx+23,rail_top+27),action)
        # Drag insertion marker, rendered above the contiguous rail.
        if self.tab_drag and self.tab_drag[4] and self.tab_drop_slot is not None:
            marker = rail_left+3+(self.tab_drop_slot-first)*tab_w
            marker = max(rail_left+3,min(marker,rail_left+3+len(shown)*tab_w))
            line(d,[(marker,rail_top+1),(marker,rail_bottom-2)],fill=(0,130,255,255),width=3)
        # Cohesive 20px navigation: each icon is a standalone SVG/PNG from the
        # Melania micro-icon set. No font-dependent arrow characters.
        nav_names=['nav-back','nav-forward','nav-up']
        for i,name in enumerate(nav_names):
            cx=x+29+i*34
            rounded(d,(cx-13,y+64,cx+14,y+93),9,fill=(250,253,255),outline=(196,218,237))
            if (name=='nav-back' and self.nav_cursor==0) or (name=='nav-forward' and self.nav_cursor==len(self.nav_path)-1):
                # Disabled arrows appear visually muted and are not clickable.
                tint=Image.new('RGBA',(19,19),(247,251,255,0))
                sym=self.shell_icon(name,19).copy()
                sym.putalpha(sym.getchannel('A').point(lambda v:int(v*.3)))
                im.paste(sym,(cx-9,y+69),sym)
            else:
                self.paste_shell(im,name,(cx-9,y+69),19)
                self.hit((cx-13,y+64,cx+14,y+93),'nav:'+name.split('-')[1])
        crumb_x=x+133
        available=x+w-18
        detail_w=83
        # Prefer useful search width without crushing the breadcrumb on small screens.
        search_w=max(126,min(228,round(w*.255)))
        search_x=available-detail_w-search_w-12
        crumb_end=search_x-10
        rounded(d,(crumb_x,y+66,crumb_end,y+94),7,fill=(255,255,255),outline=(206,224,243))
        self.paste_shell(im,'nav-home',(crumb_x+6,y+70),19)
        self.hit((crumb_x+5,y+67,crumb_x+28,y+94),'nav:home')
        location='  ›  '.join(self.nav_path[:self.nav_cursor+1])
        text(d,(crumb_x+31,y+72),ellipsize(d,location,max(35,crumb_end-crumb_x-40),12),12,(55,79,110))
        rounded(d,(search_x,y+66,search_x+search_w,y+94),8,fill=(255,255,255),outline=(206,224,243))
        self.paste_shell(im,'nav-search',(search_x+7,y+71),18)
        self.file_search_rect=(search_x+29,y+69,search_x+search_w-8,y+92)
        if not self.file_search_var.get() and self.root.focus_get() is not self.file_search:
            # Hint appears only before editing (entry itself is transparent-white).
            pass
        rounded(d,(available-detail_w+7,y+67,available,y+94),7,fill=(237,243,251),outline=(202,220,237))
        self.paste_shell(im,'nav-details',(available-detail_w+16,y+71),18)
        text(d,(available-detail_w+39,y+73),'Lista' if self.file_view=='griglia' else 'Griglia',11,(46,83,126))
        self.hit((available-detail_w+7,y+67,available,y+94),'view-toggle')
        # Refresh has an independent 24px hit target in the address row.
        self.paste_shell(im,'nav-refresh',(crumb_end-24,y+71),17)
        self.hit((crumb_end-27,y+66,crumb_end,y+94),'nav:refresh')
        # Compact adaptive *working* action shelf below the address row.
        # Actions in the UX lab affect tab/navigation/view state; no real files
        # are created, moved or deleted by this simulated interface.
        shelf_y=y+107
        rounded(d,(x+13,shelf_y,x+w-13,shelf_y+33),6,
                fill=(237,246,254,255),outline=(202,224,242,255))
        d.line((x+23,shelf_y+32,x+w-23,shelf_y+32),fill=(207,225,244),width=1)
        actions=[('Nuova scheda','file-tab-new',121,'nav-plus'),
                 ('Inverti ordine','sort-toggle',127,'nav-refresh'),
                 ('Elenco' if self.file_view=='griglia' else 'Griglia','view-toggle',95,'nav-grid'),
                 ('Dettagli','details-toggle',92,'nav-details')]
        self.action_rects={}
        bx=x+20
        room_right=x+w-19
        for label,action,preferred,icon_name in actions:
            if bx+preferred > room_right: break
            bw=preferred
            selected = (action=='details-toggle' and self.details_enabled)
            if selected: rounded(d,(bx,shelf_y+3,bx+bw,shelf_y+29),6,fill=(206,233,254),outline=(172,211,244))
            self.paste_shell(im,icon_name,(bx+7,shelf_y+8),16)
            text(d,(bx+28,shelf_y+9),label,11,(32,76,119),selected)
            rect=(bx,shelf_y+3,bx+bw,shelf_y+29)
            self.hit(rect,action)
            self.action_rects[action]=rect
            bx += bw+5
        # Sidebar and content start below the action shelf.
        left=x+11; sidebar_y=y+145; side_w=165 if w>=760 else 144; foot=y+h-42
        rounded(d,(left,sidebar_y,left+side_w,foot),5,fill=(239,247,255),outline=(209,227,245))
        rows=[('user-home','Home'),('document-open-recent','Recenti'),('emblem-favorite','Preferiti'),('folder-documents','Documenti'),('folder-pictures','Immagini'),('folder-music','Musica'),('folder-videos','Video'),('folder-download','Download'),('user-trash','Cestino')]
        side_space=foot-sidebar_y
        compact=side_space<395
        side_step=28 if compact else 34
        for i,(ic,lab) in enumerate(rows):
            iy=sidebar_y+13+i*side_step
            if lab==self.category:
                rounded(d,(left+7,iy-3,left+side_w-8,iy+side_step-4),5,fill=(185,222,254),outline=(160,201,248))
            self.paste_icon(im,'places',ic,(left+17,iy+1),18)
            text(d,(left+44,iy+3),lab,11 if compact else 12,(36,72,110),lab==self.category)
            self.hit((left+10,iy-3,left+side_w-8,iy+side_step-4),'category:'+lab)
        devices_y=sidebar_y+13+len(rows)*side_step+11
        if devices_y+78 < foot:
            text(d,(left+15,devices_y),'Dispositivi',11,(101,127,153),True)
            for i,(ic,lab) in enumerate([('computer','Sistema'),('network-server','Rete locale')]):
                iy=devices_y+23+i*27
                self.paste_icon(im,'places',ic,(left+17,iy),18)
                text(d,(left+43,iy+2),lab,11,(45,81,121))
        # Three real non-overlapping zones: sidebar | adaptive grid | inspector.
        # The number of columns comes from the available grid width, never a
        # hard-coded 4-column row that can disappear behind the inspector.
        grid_x=left+side_w+4
        show_inspector=w>=730 and self.details_enabled
        detail_width=min(224,max(188,round(w*.26))) if show_inspector else 0
        grid_end=x+w-12-detail_width-(4 if show_inspector else 0)
        rx=grid_end+(4 if show_inspector else 0)
        tile_cols,_=file_columns(w,side_w,detail_width)
        usable=grid_end-grid_x
        tile_width=118; gap=12
        occupied=tile_cols*tile_width + (tile_cols-1)*gap
        pad=max(8,(usable-occupied)//2)
        matching_count=sum(1 for name in self.file_names if self.file_query() in name.casefold())
        rows=max(1,(matching_count+tile_cols-1)//tile_cols)
        # Fit all thumbnail rows inside the reduced content height at 800x600.
        row_step=min(130,max(68,(foot-sidebar_y-30)//rows))
        thumb_h=max(39,min(78,row_step-41))
        d.rectangle((grid_x,sidebar_y,grid_end,foot),fill=(255,255,255))
        d.line((grid_end+1,sidebar_y,grid_end+1,foot),fill=(213,228,242),width=1)
        names=self.file_names
        visible=[(i,label) for i,label in enumerate(names) if self.file_query() in label.casefold()]
        if self.file_reverse: visible.reverse()
        # An active filter must not leave the inspector showing a hidden item.
        if visible and self.selected not in {i for i,_ in visible}:
            self.selected=visible[0][0]
        self.tile_rects=[]
        if self.file_view=='griglia':
            for index, (i,label) in enumerate(visible):
                col=index%tile_cols; row=index//tile_cols
                px=grid_x+pad+col*(tile_width+gap)
                py=sidebar_y+17+row*row_step
                box=(px-3,py-4,px+tile_width+3,py+row_step-7)
                self.tile_rects.append(box)
                if i==self.selected:
                    rounded(d,box,7,fill=(209,235,255),outline=(37,139,246),width=2)
                crop=self.thumb[i]
                thumb_box=(px+5,py,px+tile_width-5,py+thumb_h)
                if crop is not None:
                    tile=ImageOps.fit(crop,(thumb_box[2]-thumb_box[0],thumb_box[3]-thumb_box[1]),method=Image.Resampling.LANCZOS)
                    im.paste(tile,thumb_box[:2])
                    rounded(d,thumb_box,3,fill=None,outline=(187,216,241),width=1)
                else:
                    self.paste_icon(im,'places','folder-pictures',(px+24,py+6),64)
                text(d,(px+tile_width//2,py+thumb_h+6),ellipsize(d,label,tile_width-8,11),11,(32,59,92),i==self.selected,anchor='mt')
                text(d,(px+tile_width//2,py+thumb_h+24),f'{self.file_counts[i]} elementi',10,(112,140,169),anchor='mt')
                self.hit(box,'select:'+str(i))
        else:
            list_row_h=37
            max_rows=max(1,(foot-sidebar_y-12)//list_row_h)
            for index,(i,label) in enumerate(visible[:max_rows]):
                py=sidebar_y+8+index*list_row_h
                bx=(grid_x+9,py,grid_end-9,py+list_row_h-2)
                self.tile_rects.append(bx)
                if i==self.selected:
                    rounded(d,bx,6,fill=(213,237,255),outline=(53,145,236))
                elif index%2:
                    rounded(d,bx,5,fill=(249,252,255))
                if self.thumb[i] is not None:
                    photo=ImageOps.fit(self.thumb[i],(38,28),method=Image.Resampling.LANCZOS)
                    im.paste(photo,(grid_x+18,py+3))
                text(d,(grid_x+67,py+6),ellipsize(d,label,usable-133,12),12,(32,59,92),i==self.selected)
                text(d,(grid_end-15,py+8),f'{self.file_counts[i]} elem.',10,(112,140,169),anchor='rt')
                self.hit(bx,'select:'+str(i))
        if not visible:
            text(d,(grid_x+usable//2,sidebar_y+50),'Nessun risultato',13,(93,126,160),anchor='mt')
        self.file_layout={
            'window':(x,y,x+w,y+h),
            'sidebar':(left,sidebar_y,left+side_w,foot),
            'grid':(grid_x,sidebar_y,grid_end,foot),
            'inspector':(rx,sidebar_y,x+w-12,foot),
            'columns':tile_cols,
            'visible_count':len(visible),
            'view':self.file_view,
            'visible_names':[label for _,label in visible],
            'selected_count':self.file_counts[self.selected] if visible else None,
            'action_rects':dict(self.action_rects),
        }
        if show_inspector and visible:
            # Fixed inspector bounds: preview is centered and uses cover crop for
            # photography, preserving aspect ratio (no distorted 198x146 stretch).
            d.rectangle((rx,sidebar_y,x+w-12,foot),fill=(244,249,254))
            inside=(rx+12,sidebar_y+13,x+w-26,sidebar_y+153)
            if self.thumb[self.selected] is not None:
                source=self.detail_venice if self.selected==1 and self.detail_venice is not None else self.thumb[self.selected]
                tile=ImageOps.fit(source,(inside[2]-inside[0],inside[3]-inside[1]),method=Image.Resampling.LANCZOS)
                im.paste(tile,inside[:2])
                rounded(d,inside,4,fill=None,outline=(200,220,236))
            label=names[self.selected]
            text(d,(rx+12,sidebar_y+169),ellipsize(d,label,detail_width-26,15),15,(20,40,66),True)
            text(d,(rx+12,sidebar_y+199),f'Cartella · {self.file_counts[self.selected]} elementi',11,(95,122,153))
            d.line((rx+12,sidebar_y+225,x+w-26,sidebar_y+225),fill=(218,229,238),width=1)
            # Avoid fabricating per-folder filesystem metadata in the UX lab.
            for i,(k,v) in enumerate([('Modifica','—'),('Dimensione','—'),('Tipo','Cartella demo'),('Posizione','/'.join(self.nav_path[:self.nav_cursor+1]))]):
                yy=sidebar_y+239+i*31
                text(d,(rx+12,yy),k,10,(109,131,158))
                text(d,(rx+102,yy),ellipsize(d,v,detail_width-121,10),10,(53,76,102))
            text(d,(rx+12,sidebar_y+371),'Etichette',11,(94,123,149))
            for i,c in enumerate([(238,84,114),(148,80,208),(33,161,251),(16,178,155)]):
                d.ellipse((rx+104+i*22,sidebar_y+369,rx+117+i*22,sidebar_y+382),fill=c)
        elif show_inspector:
            d.rectangle((rx,sidebar_y,x+w-12,foot),fill=(244,249,254))
            text(d,(rx+12,sidebar_y+26),'Nessun elemento selezionato',12,(84,118,153))
        d.rectangle((x+10,foot,x+w-11,y+h-10),fill=(234,244,254))
        detail_status='Anteprima abilitata' if show_inspector else 'Pannello dettagli nascosto (spazio ridotto)'
        view_info=f'{tile_cols} colonne adattive' if self.file_view=='griglia' else 'vista elenco'
        text(d,(x+22,foot+8),f'{len(visible)} di {len(names)} cartelle demo  •  {view_info}  •  {detail_status}',11,(94,125,157))
        # Render thumbnail preview from the real Files window into the taskbar
        # (instead of showing an unrelated scenic photograph).
        self.file_preview_source=im.crop((x+7,y+39,x+w-7,y+h-7)).convert('RGB')

    def draw_settings(self,im,d):
        if not self.settings_open:return
        w=min(585,self.width-42);h=min(444,self.taskbar_top-48);x=max(18,self.width-w-44);y=max(32,min(118,(self.taskbar_top-h)//2))
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
        y=self.taskbar_top
        self.glass(im,(0,y,self.width,self.height),radius=0,rgba=(8,40,71,148),border=(181,226,253,170),blur=19)
        d.line((0,y+1,self.width,y+1),fill=(220,244,255,190),width=1)
        rounded(d,(13,y+4,65,self.height-5),12,fill=(29,82,144,110),outline=(71,140,219,100))
        logo=self.logo.resize((40,40),Image.Resampling.LANCZOS)
        im.paste(logo,(19,y+6),logo)
        self.hit((5,y,68,self.height-1),'start')
        for i,(cat,n,act) in enumerate([('apps','file-manager','files'),('apps','microsoft-edge','previews'),('apps','firefox','previews'),('apps','utilities-terminal','previews'),('apps','preferences-system','settings')]):
            xx=77+i*59
            if i==0: rounded(d,(xx,y+4,xx+49,y+47),9,fill=(50,114,171,135),outline=(105,167,223,120))
            self.paste_icon(im,cat,n,(xx+9,y+7),34)
            self.hit((xx,y+4,xx+53,y+47),act)
        if self.width<950:
            xx=386
            rounded(d,(xx,y+15,xx+91,y+42),6,fill=(37,78,121,194))
            text(d,(xx+46,y+20),f'Spazio {self.workspace}  ›',11,(239,248,255),True,anchor='mt')
            self.hit((xx,y+15,xx+91,y+42),'workspace-cycle')
        else:
            for i in range(4):
                xx=466+i*30
                rounded(d,(xx,y+17,xx+25,y+43),6,fill=(44,115,187,210) if i+1==self.workspace else (48,71,105,155))
                text(d,(xx+13,y+21),str(i+1),11,'white',True,anchor='mt')
                self.hit((xx,y+17,xx+26,y+44),'workspace:'+str(i+1))
        # Compact monochrome system tray, 6 reusable 20px Melania icons.
        # Anchor all controls to the *right* viewport edge at any screen width.
        tray_items=[('chevron-up','quick'),('wifi','quick'),('bluetooth','quick'),
                    ('volume','quick'),('bell','quick-tab:Notifiche'),('battery','quick')]
        tray_start=self.width-280
        for i,(icon_name,action) in enumerate(tray_items):
            xx=tray_start+i*28
            if self.quick_open and i==1:
                rounded(d,(xx-3,y+10,xx+25,y+39),7,fill=(44,113,180,124))
            self.paste_shell(im,icon_name,(xx+2,y+16),20,'tray')
            self.hit((xx-3,y+7,xx+27,self.height-3),action)
        self.tray_rect=(tray_start-3,y+6,tray_start+169,self.height-3)
        text(d,(self.width-15,y+10),dt.datetime.now().strftime('%H:%M'),14,'white',True,anchor='rt')
        text(d,(self.width-15,y+30),dt.datetime.now().strftime('%d/%m/%Y'),12,(190,215,239),anchor='rt')
        self.clock_rect=(self.width-86,y+4,self.width-3,self.height-3)
        self.hit(self.clock_rect,'calendar')

    def draw_start(self, im,d):
        if not self.start_open:return
        w=min(376,self.width-20)
        h=min(371,self.taskbar_top-22)
        x=8
        y=self.taskbar_top-h-8
        self.start_rect=(x,y,x+w,y+h)
        self.glass(im,(x,y,x+w,y+h),radius=16,rgba=(8,34,62,173),border=(166,222,255,190),blur=17)
        rounded(d,(x+15,y+14,x+w-15,y+54),12,fill=(42,89,132,184),outline=(153,205,245,134))
        self.paste_shell(im,'nav-search',(x+25,y+23),18,'tray')
        self.search_rect=(x+50,y+20,x+w-23,y+49)
        self.start_search.place(x=x+50,y=y+21,width=w-74,height=28)
        d.line((x+183,y+65,x+183,y+h-43),fill=(73,114,151,170))
        query=self.start_query()
        names=[item for item in self.start_items() if query in item[2].casefold()]
        for i,(cat,ic,lab,act) in enumerate(names):
            iy=y+66+i*26
            if i==self.hovered_menu or (self.hovered_menu is None and i==0):
                rounded(d,(x+12,iy-2,x+181,iy+25),6,fill=(26,90,142,160))
            self.paste_icon(im,cat,ic,(x+17,iy),21)
            text(d,(x+47,iy+3),lab,12,(230,242,255))
            self.hit((x+15,iy,x+180,iy+24),act)
        if not names:
            text(d,(x+23,y+86),'Nessuna applicazione trovata',11,(177,208,233))
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
        self.paste_shell(im,'power',(x+187,y+h-31),17,'tray')
        text(d,(x+208,y+h-29),'Esci da Melania',11,(236,245,255))
        self.hit((x+180,y+h-37,x+w-12,y+h-8),'quit-demo')
        text(d,(x+21,y+h-27),'Melania Desktop',11,(169,202,228))

    def draw_previews(self,im,d):
        # A single preview anchored over the corresponding running taskbar app.
        if not self.preview_windows or self.file_preview_source is None: return
        xx,yy,w,h=77,self.taskbar_top-144,223,132
        self.glass(im,(xx,yy,xx+w,yy+h),12,rgba=(10,32,55,228),blur=9)
        self.paste_icon(im,'apps','file-manager',(xx+12,yy+9),21)
        text(d,(xx+43,yy+10),'Melania Files',12,(242,248,255),True)
        rounded(d,(xx+9,yy+37,xx+w-9,yy+h-9),5,fill=(243,249,255))
        shot=ImageOps.contain(self.file_preview_source,(w-23,h-49),method=Image.Resampling.LANCZOS)
        im.paste(shot,(xx+(w-shot.width)//2,yy+39+(h-49-shot.height)//2))
        self.hit((xx,yy,xx+w,yy+h),'files')

    def draw_quick(self,im,d):
        if not self.quick_open: return
        w,h=306,434;x=self.width-w-12;y=max(9,self.taskbar_top-h-9)
        self.glass(im,(x,y,x+w,y+h),14,rgba=(8,35,63,174),border=(167,224,255,175),blur=18)
        rounded(d,(x+12,y+9,x+148,y+40),9,fill=(24,118,205,235) if self.quick_tab=='Notifiche' else (26,52,78,170),outline=(121,189,244,180))
        rounded(d,(x+152,y+9,x+w-13,y+40),9,fill=(24,118,205,235) if self.quick_tab=='Area rapida' else (26,52,78,170),outline=(71,109,146,120))
        text(d,(x+80,y+17),'Notifiche',12,'white',True,anchor='mt')
        text(d,(x+219,y+17),'Area rapida',12,(198,221,239),anchor='mt')
        self.hit((x+12,y+9,x+147,y+40),'quick-tab:Notifiche')
        self.hit((x+151,y+9,x+w-13,y+40),'quick-tab:Area rapida')
        if self.quick_tab=='Notifiche':
            if not self.notices:
                text(d,(x+w//2,y+98),'Nessuna nuova notifica',13,(188,217,240),anchor='mt')
            for i,(ic,heading,descr,when) in enumerate(self.notices[:3]):
                iy=y+53+i*55
                rounded(d,(x+9,iy-3,x+w-9,iy+48),8,fill=(21,52,79,160),outline=(65,106,145,70))
                self.paste_icon(im,'apps',ic,(x+16,iy+6),29)
                text(d,(x+57,iy+3),heading,12,(238,248,255),True)
                text(d,(x+57,iy+23),descr,10,(166,201,228))
                text(d,(x+w-14,iy+3),when,10,(132,168,199),anchor='rt')
        else:
            for i,(title,description) in enumerate([
                ('Rete', 'Wi-Fi attivo' if self.toggles['Wi-Fi'] else 'Wi-Fi disattivato'),
                ('Bluetooth','Attivo' if self.toggles['Bluetooth'] else 'Non attivo'),
                ('Sistema','Controlli di Melania Desktop'),
            ]):
                iy=y+53+i*55
                rounded(d,(x+9,iy-3,x+w-9,iy+48),8,fill=(21,52,79,160),outline=(65,106,145,70))
                text(d,(x+18,iy+4),title,12,(237,247,255),True)
                text(d,(x+18,iy+25),description,11,(171,208,237))
        text(d,(x+41,y+223),'Non disturbare',11,(225,240,252))
        rounded(d,(x+11,y+221,x+31,y+232),6,fill=(47,146,226) if self.toggles['Non disturbare'] else (61,103,143))
        self.hit((x+10,y+214,x+153,y+241),'toggle:Non disturbare')
        rounded(d,(x+w-106,y+214,x+w-13,y+241),7,fill=(29,65,94),outline=(77,122,161,130))
        text(d,(x+w-60,y+219),'Cancella tutto',10,(214,236,252),anchor='mt')
        self.hit((x+w-106,y+214,x+w-13,y+241),'clear-notices')
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
        w,h=306,336;x=self.width-w-12;y=max(9,self.taskbar_top-h-9)
        self.glass(im,(x,y,x+w,y+h),14,rgba=(8,29,50,226),blur=13)
        now=dt.datetime.now()
        target_month_index=(now.year*12+now.month-1)+self.calendar_month_offset
        year,zero_month=divmod(target_month_index,12)
        month=zero_month+1
        text(d,(x+15,y+15),f'{MONTHS_IT[month-1].capitalize()} {year}',15,(245,251,255),True)
        text(d,(x+185,y+13),'‹      ›',20,(193,227,248))
        self.hit((x+180,y+8,x+206,y+40),'calendar-prev')
        self.hit((x+221,y+8,x+247,y+40),'calendar-next')
        days=['L','M','M','G','V','S','D']
        for c,l in enumerate(days):text(d,(x+19+c*27,y+56),l,10,(181,207,228),True)
        for r,week in enumerate(calendar.monthcalendar(year,month)):
            for c,day in enumerate(week):
                if day<=0:continue
                xx=x+16+c*27;yy=y+82+r*28
                if day==now.day and month==now.month and year==now.year:
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
        text(d,(cx,y+181),DAYS_IT[now.weekday()].capitalize(),12,'white',True,anchor='mt')
        text(d,(cx,y+198),f'{now.day} {MONTHS_IT[now.month-1]} {now.year}',10,(192,222,244),anchor='mt')
        rounded(d,(x+211,y+h-47,x+w-13,y+h-15),8,fill=(40,83,121,170))
        text(d,(x+255,y+h-40),'Eventi oggi  +',11,'white',anchor='mt')

    def render(self):
        self.hits=[]
        im=self.canvas_bg()
        d=ImageDraw.Draw(im)
        self.draw_desktop(im,d)
        self.draw_file(im,d)
        self.draw_settings(im,d)
        self.draw_taskbar(im,d)
        self.draw_previews(im,d)
        self.draw_start(im,d)
        self.draw_quick(im,d)
        self.draw_calendar(im,d)
        # Native Tk entries are the only overlay widgets; position them from
        # the same responsive rectangles as the artwork, avoiding fixed coords.
        if self.start_open and self.search_rect:
            sx1,sy1,sx2,sy2=self.search_rect
            self.start_search.place(x=sx1,y=sy1,width=sx2-sx1,height=sy2-sy1)
        else:
            self.start_search.place_forget()
        if self.file_open and self.file_search_rect:
            sx1,sy1,sx2,sy2=self.file_search_rect
            self.file_search.place(x=sx1,y=sy1,width=sx2-sx1,height=sy2-sy1)
        else:
            self.file_search.place_forget()
        if self.toast_message:
            toast_w=min(390,self.width-24)
            tx=(self.width-toast_w)//2; ty=self.taskbar_top-80
            rounded(d,(tx,ty,tx+toast_w,ty+43),11,fill=(12,40,69,244),outline=(120,180,228))
            text(d,(tx+14,ty+12),ellipsize(d,self.toast_message,toast_w-25,12),12,(241,249,255))
        self.preview=ImageTk.PhotoImage(im.convert('RGB'))
        self.canvas.delete('all')
        self.canvas.create_image(0,0,anchor='nw',image=self.preview)


def main():
    p=argparse.ArgumentParser()
    p.add_argument('--log',default=None)
    p.add_argument('--width',type=int,default=1536)
    p.add_argument('--height',type=int,default=864)
    args=p.parse_args()
    root=tk.Tk(); MelaniaLive(root,args.log,width=args.width,height=args.height)
    root.mainloop()

if __name__=='__main__':main()
