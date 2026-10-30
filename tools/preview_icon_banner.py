#!/usr/bin/env python3
"""Compact 16:9 overview of the actual rasterised Melania Aero icons."""
from PIL import Image,ImageDraw,ImageFont
from build_icon_theme import ROOT,OUT,ICONS
P=ROOT/'branding/melania-icons-banner.png'
W,H=1800,1012
im=Image.new('RGB',(W,H),'#edf4fb');d=ImageDraw.Draw(im)
f='/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'
ft=ImageFont.truetype(f,20);fs=ImageFont.truetype(f,17);sub=ImageFont.truetype(f,25);title=ImageFont.truetype(f,60)
for y in range(0,252):
 r=y/252;d.line([(0,y),(W,y)],fill=(int(8+11*r),int(26+18*r),int(50+27*r)))
for x in range(1100,2100,98): d.polygon([(x,0),(x+46,0),(x-126,252),(x-168,252)],fill=(17,48,85))
logo=Image.open(OUT/'128x128/apps/melania.png').convert('RGBA');im.paste(logo,(56,41),logo)
d.text((204,47),'MELANIA AERO ICONS',font=title,fill='white')
d.text((209,132),'Icone vettoriali originali per il desktop Linux che stiamo sviluppando',font=sub,fill='#b9d8f7')
d.rounded_rectangle([1362,69,1742,171],radius=20,fill='#214a76',outline='#406e9b',width=2)
d.text((1395,83),'SVG + PNG',font=sub,fill='white')
d.text((1395,125),'16 → 256 px · HiDPI',font=ft,fill='#d4e9ff')
set1=['folder','folder-documents','folder-pictures','folder-download','folder-music','folder-videos','user-home','user-trash']
set2=['org.melaniadesktop.Files','org.melaniadesktop.Settings','preferences-desktop-theme','utilities-terminal','applications-internet','system-software-install','x-office-calendar','internet-mail']
set3=['x-office-document','x-office-spreadsheet','application-pdf','image-x-generic','audio-x-generic','video-x-generic','text-x-script','package-x-generic']
set4=['network-wireless','bluetooth-active','audio-volume-high','battery-full','display-brightness','notifications','system-shutdown','system-lock-screen']
sets=[('CARTELLE','places',set1),('APPLICAZIONI','apps',set2),('DOCUMENTI','mimetypes',set3),('SISTEMA','status',set4)]
sets[-1]=('SISTEMA',None,set4)
# 8 per row, 4 sections, 88 px icon artwork
cell_w=210;cell_h=140;X=60
for row,(head,cat,items) in enumerate(sets):
 y0=268+row*177
 d.text((64,y0+1),head,font=ft,fill='#1a476e')
 d.line((240,y0+16,1740,y0+16),fill='#c8d7e7',width=2)
 for j,n in enumerate(items):
  x=X+j*211;y=y0+30
  d.rounded_rectangle((x,y,x+196,y+127),radius=17,fill='#fff',outline='#dce8f3',width=2)
  category=cat or ('status' if n in ICONS['status'] else 'actions')
  icon=Image.open(OUT/'128x128'/category/f'{n}.png').convert('RGBA');icon.thumbnail((87,87))
  im.paste(icon,(x+(196-icon.width)//2,y+2),icon)
  label=ICONS[category][n][2]
  tw=d.textbbox((0,0),label,font=fs)[2];d.text((x+(196-tw)//2,y+95),label,font=fs,fill='#2d4869')
# Footer
d.rounded_rectangle((50,H-44,W-50,H-9),radius=12,fill='#0b2039')
d.text((76,H-38),'96 icone principali  •  13 simboliche  •  Temi e colori coerenti  •  Logo Melania ufficiale',font=fs,fill='#c5def7')
im.save(P,optimize=True)
print('Saved',P,im.size)
