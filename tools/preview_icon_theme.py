#!/usr/bin/env python3
"""Create an offline, static contact sheet from generated PNG assets."""
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont, ImageFilter
from build_icon_theme import ICONS, OUT, ROOT

DST=ROOT/'branding'/'anteprima-melania-icons.png'
W=1920
rows=[('CARTELLE E POSIZIONI', 'places', list(ICONS['places'].items())[:18]),
      ('APPLICAZIONI E IMPOSTAZIONI', 'apps', list(ICONS['apps'].items())[:26]),
      ('TIPI DI FILE', 'mimetypes', list(ICONS['mimetypes'].items())[:13]),
      ('DISPOSITIVI', 'devices', list(ICONS['devices'].items())[:10]),
      ('SISTEMA E AZIONI', 'status', list(ICONS['status'].items())[:13]),
      ('COMANDI', 'actions', list(ICONS['actions'].items())[:12])]
BG=(240,246,253); dark=(12,36,67); muted=(87,109,140); navy=(6,22,44)
font='/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'
reg=ImageFont.truetype(font,20);small=ImageFont.truetype(font,16);bold=ImageFont.truetype(font,29);hero=ImageFont.truetype(font,52)
cols=9; cwidth=194; cgap=14; inset=33; tilew=194; tileh=142
height=205
for _,_,items in rows: height+=80+((len(items)+cols-1)//cols)*(tileh+14)+18
height+=135
img=Image.new('RGB',(W,height),BG)
d=ImageDraw.Draw(img)
# Hero navy with diagonal soft bars
for y in range(0,186):
    frac=y/186
    d.line((0,y,W,y),fill=(int(8+15*frac),int(25+23*frac),int(49+34*frac)))
for x in range(1310,2000,100):
    d.polygon([(x,0),(x+50,0),(x-150,186),(x-220,186)],fill=(15,42,78))
logo=Image.open(OUT/'128x128/apps/melania.png').convert('RGBA');logo.thumbnail((115,115))
img.paste(logo,(44,29),logo)
d.text((175,39),'MELANIA',font=hero,fill=(252,254,255))
d.text((178,106),'AERO ICONS  •  SET DI ICONE DI DEFAULT',font=reg,fill=(161,205,245))
d.text((W-465,108),'Rust + GTK 4  •  SVG  •  Linux',font=reg,fill=(184,211,237))
current=203
for title,cat,items in rows:
    d.text((48,current+15),title,font=bold,fill=dark)
    d.line((48,current+58,W-46,current+58),fill=(207,221,235),width=2)
    current+=77
    for i,(name,(motif,palette,label)) in enumerate(items):
        x=44+(i%cols)*(tilew+14);y=current+(i//cols)*(tileh+14)
        d.rounded_rectangle((x+2,y+5,x+tilew+3,y+tileh+5),radius=18,fill=(220,232,244))
        d.rounded_rectangle((x,y,x+tilew,y+tileh),radius=18,fill=(255,255,255),outline=(219,229,239),width=1)
        icon=Image.open(OUT/'128x128'/cat/f'{name}.png').convert('RGBA')
        icon.thumbnail((94,94))
        ix=x+(tilew-icon.width)//2;iy=y+8
        img.paste(icon,(ix,iy),icon)
        text=label if len(label)<=19 else label[:17]+'…'
        box=d.textbbox((0,0),text,font=small)
        tw=box[2]-box[0]
        d.text((x+(tilew-tw)//2,y+110),text,font=small,fill=(35,64,103))
    current+=((len(items)+cols-1)//cols)*(tileh+14)+19
# Footer panel dark
fy=height-117
d.rounded_rectangle((32,fy,W-32,height-24),radius=19,fill=navy)
d.text((58,fy+23),'VETTORIALI SVG  •  16 / 24 / 32 / 48 / 64 / 128 / 256 PX',font=reg,fill=(228,242,255))
d.text((58,fy+54),'Logo ufficiale Melania incluso  •  Tema freedesktop con fallback Adwaita/hicolor',font=small,fill=(161,191,221))
img.save(DST,optimize=True)
print(DST, img.size)
