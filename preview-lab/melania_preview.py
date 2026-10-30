#!/usr/bin/env python3
"""Interactive Melania UX test bench. Tk/X11 ONLY, not the Rust/Wayland desktop.

Contains real safe file copying between simulated folders, real interactive
windows, Start menu, taskbar, quick controls, snap and keyboard shortcuts.
This demo never modifies files outside its own temporary demo directory.
"""
from __future__ import annotations
import argparse
import datetime as dt
import os
import pathlib
import json
import shutil
import tempfile
import tkinter as tk
from PIL import Image, ImageTk, ImageDraw, ImageFont

BASE = pathlib.Path(__file__).resolve().parent
ICON = BASE.parent / 'branding/icons/melania-256.png'
NAVY = '#08182c'; BLUE = '#1878ec'; INK = '#18253a'; TEXT = '#edf4ff'
PANEL = '#0c2039'; FRAME = '#f6f9ff'; MUTED = '#7890aa'


def safe_copy_file(src: pathlib.Path, dest_dir: pathlib.Path) -> pathlib.Path:
    """Copy a regular file without clobbering an existing destination.

    Use O_EXCL before writing, and unlink partial output on failure.
    This is deliberately scoped to regular files and refuses symlinks.
    """
    src = pathlib.Path(src)
    dest_dir = pathlib.Path(dest_dir)
    if src.is_symlink() or not src.is_file() or not dest_dir.is_dir() or dest_dir.is_symlink():
        raise ValueError('Sorgente/destinazione non valida')
    dest = dest_dir / src.name
    if src.resolve() == dest.resolve():
        raise FileExistsError('Il file si trova già nella destinazione')
    fd = os.open(dest, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    try:
        with os.fdopen(fd, 'wb') as dst, src.open('rb') as origin:
            shutil.copyfileobj(origin, dst, 1024 * 1024)
    except BaseException:
        dest.unlink(missing_ok=True)
        raise
    return dest


def bg_image(w, h):
    img = Image.new('RGB', (w, h), '#142d53'); px = img.load()
    for y in range(h):
        r = y / h
        if r < .65:
            rgb = (int(13 + 30*r), int(41 + 49*r), int(87 + 61*r))
        else:
            rgb = (int(20 + 27*r), int(39 + 30*r), int(72 + 30*r))
        for x in range(w):
            px[x,y] = rgb
    d = ImageDraw.Draw(img, 'RGBA')
    # Soft horizon and an original abstract Mediterranean landscape.
    d.ellipse((790, 65, 1170, 445), fill=(253,195,120,48))
    d.ellipse((880,135,1100,355), fill=(252,201,135,160))
    d.polygon([(0,510),(145,315),(265,475),(370,365),(590,570),(770,345),(1000,525),(1280,350),(1280,800),(0,800)], fill=(14,34,66,230))
    d.polygon([(0,620),(260,445),(465,625),(660,535),(870,710),(1120,520),(1280,620),(1280,800),(0,800)], fill=(10,26,55,240))
    d.rectangle((0,650,1280,800), fill=(11,36,78,64))
    for y in range(670, 760, 13):
        d.line((0,y,1280,y), fill=(100,168,237,18), width=2)
    return img


class MelaniaApp:
    def __init__(self, root, demo_root):
        self.root = root
        self.demo = pathlib.Path(demo_root)
        self.event_log = self.demo / 'events.jsonl'
        self.desktop_dir = self.demo / 'Scrivania'
        self.current_dir = self.demo / 'Home'
        self.pop = None
        self.windows = {}
        self.z = []
        self.dragging = None
        self.dnd_source = None
        self.switches = {'Wi-Fi':True,'Bluetooth':True,'Non disturbare':False,'Luce notturna':False}
        self.logo = ImageTk.PhotoImage(Image.open(ICON).convert('RGBA').resize((38,38)))
        self.logo_large = ImageTk.PhotoImage(Image.open(ICON).convert('RGBA').resize((64,64)))
        self.background = ImageTk.PhotoImage(bg_image(1280,800))
        root.geometry('1280x800+0+0'); root.resizable(False,False)
        root.title('Melania Desktop — anteprima verificabile (Tk/X11)')
        root.configure(bg='#08182c')
        root.bind('<Escape>',lambda e:self.close_pop())
        root.bind('<Control-Alt-t>',lambda e:self.create_window('Terminale'))
        root.bind('<Control-e>',lambda e:self.create_window('Melania Files'))
        root.bind('<Super_L>',lambda e:self.show_start())
        self.canvas = tk.Canvas(root,width=1280,height=800,highlightthickness=0)
        self.canvas.place(x=0,y=0)
        self.canvas.create_image(0,0,image=self.background,anchor='nw')
        self.canvas.bind('<Button-1>',lambda e:self.close_pop())
        self.desktop_icons()
        self.taskbar()
        self.create_window('Melania Files', 208, 106, 725, 495)
        self.create_window('Impostazioni', 956, 102, 298, 355)
        self.show_start()
        root.after(250, self.clock_tick)

    def log_event(self, event, **details):
        with self.event_log.open('a',encoding='utf-8') as logfile:
            logfile.write(json.dumps({'event':event,**details},ensure_ascii=False)+'\n')

    def desktop_icons(self):
        for i,(sym,name) in enumerate([('⌂','Home'),('▣','Computer'),('▤','Cestino')]):
            x,y=45, 100+i*90
            self.canvas.create_rectangle(x-21,y-22,x+25,y+26,fill='#15447c',outline='#3678bb',width=1)
            self.canvas.create_text(x+2,y+1,text=sym,font=('DejaVu Sans',25),fill='#c4e7ff')
            self.canvas.create_text(x+2,y+44,text=name,fill='white',font=('DejaVu Sans',10))
            self.canvas.create_rectangle(x-30,y-24,x+35,y+58,fill='',outline='',tags=(f'icon-{name}',))
            self.canvas.tag_bind(f'icon-{name}','<Double-Button-1>',lambda e,n=name:self.open_desktop(n))

    def open_desktop(self,name):
        if name == 'Home': self.current_dir=self.demo/'Home'
        elif name=='Computer':self.current_dir=self.demo
        else:self.current_dir=self.desktop_dir
        self.create_window('Melania Files')

    def btn(self,parent,text,command,bg=PANEL,fg=TEXT,px=8,py=6,font=('DejaVu Sans',10),**kw):
        return tk.Button(parent,text=text,command=command,bg=bg,fg=fg,activebackground=BLUE,
                         activeforeground='white',bd=0,highlightthickness=0,cursor='hand2',
                         padx=px,pady=py,font=font,**kw)

    def taskbar(self):
        bar=tk.Frame(self.root,bg='#08172d',height=56,highlightthickness=1,highlightbackground='#3c6597')
        bar.place(x=0,y=744,width=1280,height=56)
        self.start_btn=self.btn(bar,'',self.show_start,bg='#08172d',px=6)
        self.start_btn.config(image=self.logo,compound='left')
        self.start_btn.place(x=15,y=4,width=48,height=45)
        self.btn(bar,'▣',lambda:self.create_window('Melania Files'),font=('DejaVu Sans',21),bg='#0c2c53').place(x=77,y=6,width=48,height=43)
        self.btn(bar,'⚙',lambda:self.create_window('Impostazioni'),font=('DejaVu Sans',22),bg='#0c2c53').place(x=137,y=6,width=48,height=43)
        self.btn(bar,'▸',lambda:self.create_window('Terminale'),font=('DejaVu Sans',22),bg='#0c2c53').place(x=197,y=6,width=48,height=43)
        self.windows_area=tk.Frame(bar,bg='#08172d');self.windows_area.place(x=263,y=6,width=675,height=44)
        self.btn(bar,'⌃   ◉   ♫   ⚡',self.show_controls,bg='#102d4e').place(x=956,y=9,width=185,height=35)
        self.time_button=self.btn(bar,'',self.show_calendar,bg='#08172d',font=('DejaVu Sans',10))
        self.time_button.place(x=1151,y=5,width=116,height=45)

    def clock_tick(self):
        now=dt.datetime.now()
        self.time_button.config(text=now.strftime('%H:%M\n%d/%m/%Y'))
        self.root.after(15000,self.clock_tick)

    def refresh_windows(self):
        for w in self.windows_area.winfo_children():w.destroy()
        for title in self.z[-4:]:
            if title in self.windows:
                self.btn(self.windows_area,title[:18],lambda t=title:self.focus(t),bg='#154577',px=9).pack(side='left',padx=3,fill='y')

    def close_pop(self):
        if self.pop and self.pop.winfo_exists():self.pop.destroy()
        self.pop=None

    def popup(self,x,y,w,h):
        self.close_pop()
        p=tk.Frame(self.root,bg='#0b1c35',highlightthickness=1,highlightbackground='#5087c4')
        p.place(x=x,y=y,width=w,height=h)
        p.lift()
        self.pop=p
        return p

    def show_start(self):
        self.log_event('start-menu')
        p=self.popup(18,325,380,410)
        tk.Label(p,text='MELANIA',bg='#0b1c35',fg='#dfeeff',font=('DejaVu Sans',17,'bold')).pack(anchor='w',padx=17,pady=(15,8))
        entry=tk.Entry(p,bg='#1c3456',fg='white',insertbackground='white',bd=0,font=('DejaVu Sans',11))
        entry.pack(fill='x',padx=15,ipady=9)
        entry.insert(0,'Cerca applicazioni…')
        items=tk.Frame(p,bg='#0b1c35');items.pack(fill='both',expand=True,padx=13,pady=12)
        options=[('▣','Melania Files'),('⚙','Impostazioni'),('▸','Terminale')]
        def render(*_):
            for x in items.winfo_children():x.destroy()
            query=entry.get().strip().lower()
            for symbol,name in options:
                if query in ('cerca applicazioni…','') or query in name.lower():
                    self.btn(items,symbol+'  '+name,lambda n=name:(self.close_pop(),self.create_window(n)),bg='#122b4d',px=12,py=10).pack(fill='x',pady=3)
        entry.bind('<FocusIn>',lambda e:(entry.delete(0,'end'),render()) if entry.get()=='Cerca applicazioni…' else None)
        entry.bind('<KeyRelease>',render)
        render()
        tk.Label(p,text='●  Melania Desktop  |  Prototipo interattivo',bg='#102747',fg='#9bbce3',font=('DejaVu Sans',9)).pack(side='bottom',fill='x',pady=0,ipady=9)

    def show_controls(self):
        self.log_event('quick-panel')
        p=self.popup(900,394,366,342)
        tk.Label(p,text='Controlli rapidi',bg='#0b1c35',fg='white',font=('DejaVu Sans',15,'bold')).pack(anchor='w',padx=18,pady=(16,12))
        rows=tk.Frame(p,bg='#0b1c35');rows.pack(padx=13,fill='x')
        for i,k in enumerate(list(self.switches)):
            def toggle(key=k):
                self.switches[key]=not self.switches[key];self.log_event('quick-toggle', control=key, enabled=self.switches[key]);self.show_controls()
            self.btn(rows,k+'  '+('●' if self.switches[k] else '○'),toggle,
                     bg='#1559aa' if self.switches[k] else '#223751',px=12,py=10).grid(row=i//2,column=i%2,padx=5,pady=5,sticky='ew')
        for key,val in [('Luminosità',65),('Volume',75)]:
            tk.Label(p,text=key,bg='#0b1c35',fg='#d5e8fd').pack(anchor='w',padx=18,pady=(10,0))
            tk.Scale(p,from_=0,to=100,orient='horizontal',bg='#0b1c35',fg='#a8d7ff',troughcolor='#1b4a82',
                     highlightthickness=0,bd=0,length=310).pack();p.winfo_children()[-1].set(val)
        tk.Label(p,text='Toggle dimostrativi: nessuna modifica al sistema ospite',bg='#0b1c35',fg='#88a3c0',font=('DejaVu Sans',8)).pack(pady=5)

    def show_calendar(self):
        p=self.popup(1037,514,228,223)
        tk.Label(p,text=dt.datetime.now().strftime('%A %d %B %Y'),bg='#0b1c35',fg='white',font=('DejaVu Sans',11,'bold')).pack(pady=18)
        tk.Label(p,text=dt.datetime.now().strftime('%H:%M'),bg='#0b1c35',fg='#9acbff',font=('DejaVu Sans',30)).pack(pady=12)
        tk.Label(p,text='Nessuna nuova notifica',bg='#0b1c35',fg='#b4c6dc').pack(pady=16)

    def create_window(self,title,x=None,y=None,w=None,h=None):
        self.close_pop()
        if title in self.windows:
            self.focus(title);return
        defaults={'Melania Files':(208,106,725,495),'Impostazioni':(930,105,320,380),'Terminale':(295,170,620,360)}
        x,y,w,h=(x or defaults[title][0],y or defaults[title][1],w or defaults[title][2],h or defaults[title][3])
        box=tk.Frame(self.root,bg=FRAME,highlightthickness=1,highlightbackground='#68a7dc')
        box.place(x=x,y=y,width=w,height=h)
        head=tk.Frame(box,bg='#174a7c',height=38);head.pack(side='top',fill='x')
        tk.Label(head,text='  ◉   '+title,bg='#174a7c',fg='white',font=('DejaVu Sans',11,'bold')).pack(side='left')
        self.btn(head,'×',lambda:self.close_window(title),bg='#9d3b47',px=10,font=('DejaVu Sans',13)).pack(side='right')
        self.btn(head,'─',lambda:self.minimize(title),bg='#174a7c',font=('DejaVu Sans',12)).pack(side='right')
        head.bind('<ButtonPress-1>',lambda e,t=title:self.begin_move(e,t))
        head.bind('<B1-Motion>',lambda e,t=title:self.move(e,t))
        head.bind('<ButtonRelease-1>',lambda e,t=title:self.end_move(e,t))
        content=tk.Frame(box,bg=FRAME);content.pack(fill='both',expand=True)
        self.log_event('open-window',title=title)
        self.windows[title]={'box':box,'size':(w,h),'pos':(x,y),'content':content,'minimized':False}
        self.z.append(title)
        if title=='Melania Files':self.populate_files(content)
        elif title=='Impostazioni':self.populate_settings(content)
        elif title=='Terminale':self.populate_terminal(content)
        self.focus(title)

    def focus(self,title):
        if title not in self.windows:return
        o=self.windows[title]
        o['box'].place(x=o['pos'][0],y=o['pos'][1],width=o['size'][0],height=o['size'][1])
        o['minimized']=False;o['box'].lift()
        if title in self.z:self.z.remove(title)
        self.z.append(title);self.refresh_windows()
        if self.pop:self.pop.lift()

    def minimize(self,title):
        if title in self.windows:
            self.windows[title]['box'].place_forget();self.windows[title]['minimized']=True

    def close_window(self,title):
        if title in self.windows:
            self.windows[title]['box'].destroy();del self.windows[title]
            if title in self.z:self.z.remove(title)
            self.refresh_windows()

    def begin_move(self,event,title):
        self.dragging=(title,event.x_root,event.y_root,*self.windows[title]['pos']);self.focus(title)

    def move(self,event,title):
        if not self.dragging or self.dragging[0]!=title:return
        _,start_x,start_y,old_x,old_y=self.dragging
        x=old_x+event.x_root-start_x;y=old_y+event.y_root-start_y
        o=self.windows[title];o['pos']=(x,y)
        o['box'].place(x=x,y=y)

    def end_move(self,event,title):
        if not self.dragging:return
        self.dragging=None
        o=self.windows[title];w,h=o['size'];x,y=o['pos']
        if x < 24: o['pos']=(0,0);o['size']=(640,744)
        elif x+w > 1256:o['pos']=(640,0);o['size']=(640,744)
        elif y < 24:o['pos']=(0,0);o['size']=(1280,744)
        o['box'].place(x=o['pos'][0],y=o['pos'][1],width=o['size'][0],height=o['size'][1])
        self.log_event('window-moved',title=title,x=o['pos'][0],y=o['pos'][1],width=o['size'][0],height=o['size'][1])

    def populate_settings(self,frame):
        tk.Label(frame,text='Personalizzazione',font=('DejaVu Sans',15,'bold'),bg=FRAME,fg=INK).pack(anchor='w',padx=20,pady=(18,12))
        for title,description in [('Aspetto','Tema Aero Blu'),('Schermo','Risoluzione e monitor'),('Finestre','Aero Snap abilitato'),('Accessibilità','Scala e contrasto'),('Account','Sessione dimostrativa')]:
            row=tk.Frame(frame,bg='#e5eef9');row.pack(fill='x',padx=13,pady=5,ipady=7)
            tk.Label(row,text=title,bg='#e5eef9',fg='#20436d',font=('DejaVu Sans',10,'bold')).pack(anchor='w',padx=12)
            tk.Label(row,text=description,bg='#e5eef9',fg='#597694',font=('DejaVu Sans',9)).pack(anchor='w',padx=12)

    def populate_terminal(self,frame):
        txt=tk.Text(frame,bg='#08182a',fg='#a7e8bb',insertbackground='white',font=('DejaVu Sans Mono',11),bd=0)
        txt.pack(fill='both',expand=True)
        txt.insert('end','Melania Desktop — UX Lab\nPrototipo Tk/X11; NON è un terminale di sistema.\n\nmelania@desktop:~$ ')

    def populate_files(self,frame):
        toolbar=tk.Frame(frame,bg='#e6eff9',height=38);toolbar.pack(side='top',fill='x')
        for label,fun in [('⌂',lambda:self.change_dir(self.demo/'Home')),('↑',lambda:self.change_dir(self.current_dir.parent)),('⟳',self.refresh_files)]:
            self.btn(toolbar,label,fun,bg='#dae8f8',fg='#17375b').pack(side='left',padx=3,pady=4)
        self.path_label=tk.Label(toolbar,text='',bg='#e6eff9',fg='#365b85',font=('DejaVu Sans',9))
        self.path_label.pack(side='left',padx=10)
        self.file_body=tk.Frame(frame,bg=FRAME);self.file_body.pack(fill='both',expand=True)
        self.file_info=tk.Label(frame,text='',bg='#e9f0f9',fg='#46698f',anchor='w',font=('DejaVu Sans',9))
        self.file_info.pack(side='bottom',fill='x',ipady=6)
        self.refresh_files()

    def change_dir(self,path):
        path=path.resolve()
        if path.is_dir() and (path == self.demo or self.demo in path.parents):
            self.current_dir=path;self.refresh_files()

    def refresh_files(self):
        if 'Melania Files' not in self.windows:return
        for child in self.file_body.winfo_children():child.destroy()
        self.path_label.config(text='Home › '+(' › '.join(self.current_dir.relative_to(self.demo).parts)))
        nodes=sorted(self.current_dir.iterdir(),key=lambda p:(not p.is_dir(),p.name.lower()))
        for i,item in enumerate(nodes):
            col=i%4;row=i//4
            tile=tk.Frame(self.file_body,bg='#eff5fc',highlightbackground='#d4e3f5',highlightthickness=1,width=145,height=107)
            tile.grid(row=row,column=col,padx=9,pady=10,sticky='nw');tile.grid_propagate(False)
            symbol='📁' if item.is_dir() else ('▧' if item.suffix=='.png' else '▤')
            label=tk.Label(tile,text=symbol,bg='#eff5fc',fg='#1767c4',font=('DejaVu Sans',24))
            label.pack(pady=(8,2))
            name=tk.Label(tile,text=item.name[:20],bg='#eff5fc',fg='#1e3e65',font=('DejaVu Sans',9))
            name.pack()
            for el in (tile,label,name):
                el.target_directory=item if item.is_dir() else None
                el.bind('<Double-Button-1>',lambda e,p=item:self.change_dir(p) if p.is_dir() else self.show_file(p))
                el.bind('<ButtonPress-1>',lambda e,p=item:self.file_press(e,p))
                el.bind('<B1-Motion>',self.file_motion)
                el.bind('<ButtonRelease-1>',self.file_release)
        self.file_info.config(text=f' {len(nodes)} elementi  •  Trascina un file su una cartella per COPIARLO  •  Solo dati temporanei')

    def show_file(self,path):
        self.file_info.config(text=' File selezionato: '+path.name)

    def file_press(self,e,path):
        self.dnd_source=(path,e.x_root,e.y_root)

    def file_motion(self,e):
        if self.dnd_source and abs(e.x_root-self.dnd_source[1])+abs(e.y_root-self.dnd_source[2])>10:
            self.file_info.config(text=' Trascinamento: '+self.dnd_source[0].name+'  →  rilascia sopra una cartella')

    def file_release(self,e):
        if not self.dnd_source:return
        source=self.dnd_source[0];self.dnd_source=None
        target=self.root.winfo_containing(e.x_root,e.y_root)
        destination=None
        while target:
            destination=getattr(target,'target_directory',None)
            if destination:break
            target=target.master
        if destination and source.is_file():
            try:
                new_file=safe_copy_file(source,destination)
                self.log_event('file-copy',source=source.name,destination=destination.name)
                self.file_info.config(text=' Copiato: '+new_file.name+' → '+str(destination.name))
            except (OSError,ValueError) as exc:
                self.file_info.config(text=' Copia annullata: '+str(exc))
            self.refresh_files()


def make_demo_dir(folder):
    d=pathlib.Path(folder)
    for name in ('Home','Scrivania','Home/Documenti','Home/Immagini','Home/Progetti','Home/Download'):(d/name).mkdir(parents=True,exist_ok=True)
    for name,contents in [('Home/Documenti/Benvenuto.txt','Benvenuto in Melania Desktop!\n'),('Home/Documenti/Appunti.txt','Trascina questo file nella cartella Progetti.\n'),('Home/Progetti/README.md','# Melania Desktop\n'),('Home/Download/Note.txt','File di esempio.\n'),('Home/Leggimi.txt','Trascinami nella cartella Progetti.\n')]:
        (d/name).write_text(contents,encoding='utf-8')
    for name,color in [('Mare.png','#377baf'),('Tramonto.png','#f29c58'),('Montagne.png','#456c95')]:
        im=Image.new('RGB',(220,130),color)
        dd=ImageDraw.Draw(im);dd.ellipse((110,22,195,108),fill='#fbd18b')
        dd.polygon([(0,95),(70,60),(120,105),(155,73),(220,100),(220,130),(0,130)],fill='#183d73')
        im.save(d/'Home/Immagini'/name)


def main():
    p=argparse.ArgumentParser()
    p.add_argument('--demo-dir',type=pathlib.Path,help='Use an existing safe demo directory')
    args=p.parse_args()
    if args.demo_dir:
        args.demo_dir.mkdir(parents=True,exist_ok=True)
        make_demo_dir(args.demo_dir)
        root=tk.Tk();MelaniaApp(root,args.demo_dir);root.mainloop()
    else:
        with tempfile.TemporaryDirectory(prefix='melania-ux-') as tmp:
            make_demo_dir(tmp)
            root=tk.Tk();MelaniaApp(root,tmp);root.mainloop()

if __name__=='__main__':main()
