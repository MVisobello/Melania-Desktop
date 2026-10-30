#!/usr/bin/env python3
"""Melania Classic Aero: 30 original, screenshot-guided desktop icon families.

A bright, familiar Windows-7-era desktop language: unframed desktop objects,
classic warm folders, cyan/photo thumbnails, gently polished metal; simple
blue navigation controls and distinct, colourful launcher pictograms.

The user-provided Melania screenshots are the art direction. Flat Remix icon
artwork is never imported. This source produces editable standalone SVGs.
"""
from __future__ import annotations
from pathlib import Path
import argparse
import html
import xml.etree.ElementTree as ET

# Values are drawing identities, NOT categories/colors substituted into a tile.
ALIASES = {
    'home': ('user-home','go-home','folder-home','home','user-home-symbolic'),
    'documents': ('folder-documents','user-documents','folder-templates','folder-documents-symbolic'),
    'pictures': ('folder-pictures','user-pictures','folder-image','folder-images','pictures','folder-pictures-symbolic'),
    'music': ('folder-music','user-music','folder-audio','folder-music-symbolic'),
    'videos': ('folder-videos','user-videos','folder-video','folder-videos-symbolic'),
    'downloads': ('folder-download','folder-downloads','user-download','folder-download-symbolic'),
    'recent': ('document-open-recent','folder-recent','user-recent','recent','document-open-recent-symbolic'),
    'favorites': ('user-bookmarks','folder-bookmarks','folder-favorites','starred','emblem-favorite'),
    'desktop': ('user-desktop','folder-desktop','desktop'),
    'trash': ('user-trash','user-trash-full','user-trash-symbolic','user-trash-full-symbolic'),
    'folder': ('folder','inode-directory','folder-symbolic'),
    'folder-open': ('folder-open','folder-open-symbolic'),
    'computer': ('computer','computer-desktop','video-display','display','applications-system'),
    'system': ('drive-harddisk-system','drive-harddisk-root','drive-harddisk','harddisk','drive-harddisk-symbolic'),
    'disk': ('drive-harddisk-data','drive-removable-media','drive-removable-media-usb'),
    'usb': ('media-flash','media-removable','drive-removable-media-usb','drive-removable-media','drive-harddisk-usb'),
    'network': ('network-workgroup','network-local','network-wired','network-wired-symbolic'),
    'server': ('network-server','network-server-symbolic','server'),
    'devices': ('preferences-system-devices','applications-devices','input-keyboard','devices'),
    'browser': ('web-browser','internet-web-browser','applications-internet','org.melaniadesktop.Browser'),
    'files': ('system-file-manager','org.melaniadesktop.Files','file-manager','folder-manager','melania-files'),
    'terminal': ('utilities-terminal','org.gnome.Terminal','org.melaniadesktop.Terminal','terminal','xterm'),
    'mail': ('internet-mail','email','mail-client','evolution','org.melaniadesktop.Mail'),
    'settings': ('preferences-system','org.melaniadesktop.Settings','systemsettings','gnome-control-center','applications-utilities'),
    'photos': ('applications-graphics','image-viewer','org.gnome.Loupe','shotwell','org.melaniadesktop.Photos'),
    'office': ('applications-office','libreoffice-startcenter','office'),
    'back': ('go-previous','go-previous-symbolic','gtk-go-back-ltr'),
    'forward': ('go-next','go-next-symbolic','gtk-go-forward-ltr'),
    'refresh': ('view-refresh','view-refresh-symbolic','reload','gtk-refresh'),
    'search': ('edit-find','edit-find-symbolic','system-search','search'),
}
# Some aliases occur in multiple families: prefer launcher icon for files/apps;
# explicit root name for documents; USB for USB identifiers.
REVERSE = {}
for identity, names in ALIASES.items():
    for name in names:
        REVERSE.setdefault(name, identity)
REVERSE.update({'folder': 'folder', 'drive-removable-media-usb':'usb',
                'drive-removable-media':'usb','applications-system':'computer',
                'applications-internet':'browser', 'applications-graphics':'photos',
                'preferences-system':'settings', 'go-home':'home'})

DEFS = '''
<linearGradient id="folderBack" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FFF5C9"/><stop offset=".38" stop-color="#FFE59B"/><stop offset="1" stop-color="#ED9A2C"/></linearGradient>
<linearGradient id="folderFace" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FFF7D8"/><stop offset=".24" stop-color="#FFEBA9"/><stop offset=".67" stop-color="#FFD066"/><stop offset="1" stop-color="#F3A23D"/></linearGradient>
<linearGradient id="aeroBlue" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#BAEDFF"/><stop offset=".36" stop-color="#4ABAF2"/><stop offset="1" stop-color="#1270B8"/></linearGradient>
<linearGradient id="deepBlue" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#73D0FC"/><stop offset=".5" stop-color="#2A8BD1"/><stop offset="1" stop-color="#134F92"/></linearGradient>
<linearGradient id="metal" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FBFEFF"/><stop offset=".36" stop-color="#E5F1F8"/><stop offset=".66" stop-color="#B6CCDC"/><stop offset="1" stop-color="#758DA4"/></linearGradient>
<linearGradient id="paper" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FFFFFF"/><stop offset="1" stop-color="#DCEBF7"/></linearGradient>
<linearGradient id="darkTile" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#8EA4B8"/><stop offset=".4" stop-color="#425A6C"/><stop offset="1" stop-color="#273846"/></linearGradient>
<linearGradient id="mailTile" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#90E9FF"/><stop offset=".45" stop-color="#1EACF3"/><stop offset="1" stop-color="#0874C8"/></linearGradient>
<linearGradient id="violetTile" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#ECBCFF"/><stop offset=".5" stop-color="#B251DE"/><stop offset="1" stop-color="#7734B1"/></linearGradient>
<linearGradient id="orangeTile" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FFE7A0"/><stop offset=".45" stop-color="#FFA32D"/><stop offset="1" stop-color="#EE6619"/></linearGradient>
<linearGradient id="pinkTile" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#FFD0EA"/><stop offset=".5" stop-color="#ED6CC7"/><stop offset="1" stop-color="#AC329D"/></linearGradient>
<linearGradient id="greenTile" x1="0" y1="0" x2="0" y2="1"><stop stop-color="#D5FFC4"/><stop offset=".45" stop-color="#5BCF9E"/><stop offset="1" stop-color="#209B7B"/></linearGradient>
<radialGradient id="glass" cx=".32" cy=".16" r=".92"><stop stop-color="#C5F9FF"/><stop offset=".55" stop-color="#40B9EA"/><stop offset="1" stop-color="#1169BD"/></radialGradient>
'''


def S(body: str) -> str:
    return '<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96"><defs>'+DEFS+'</defs>'+body+'</svg>'


def _shadow(cx=48, rx=30, y=84):
    return f'<ellipse cx="{cx}" cy="{y}" rx="{rx}" ry="3.5" fill="#112F4C" opacity=".17"/>'


def _folder_base(opened=False, small=False):
    back = ('<path d="M12 31 Q12 25 19 25 H38 L45 31 H77 Q84 31 84 39 V73 Q84 79 76 79 H19 Q12 79 12 72Z" '
            'fill="url(#folderBack)" stroke="#CD8B30" stroke-width="1.4"/>')
    fold = ('<path d="M15 45 Q18 40 24 40 H83 L77 77 Q76 82 69 82 H17 Q11 82 12 76Z" '
            'fill="url(#folderFace)" stroke="#D38D25" stroke-width="1.5"/>')
    if opened:
        fold = '<path d="M20 43 L86 35 L74 81 H10Z" fill="url(#folderFace)" stroke="#C78B2D" stroke-width="1.5"/>'
    gloss = '<path d="M19 45 H79" fill="none" stroke="#FFFFFF" stroke-opacity=".55" stroke-width="2"/>'
    if small: gloss=''
    return _shadow(48,31,86)+back+fold+gloss


def folder_style(identity, small=False):
    if identity in ('documents','folder','folder-open','downloads'):
        base=_folder_base(identity=='folder-open',small)
        if identity=='downloads':
            base += ('<path d="M47 49v18m-8-8 8 9 8-9M38 72h18" fill="none" stroke="#347FC1" '
                     'stroke-width="4.2" stroke-linecap="round" stroke-linejoin="round"/>')
        return S(base)
    return None


def house(small):
    return S(_shadow(47,27,87)+
      '<path d="M18 45 45 19 Q47 17 50 20 L78 45 72 51 48 30 24 51Z" fill="url(#metal)" stroke="#2B76B3" stroke-width="2.6" stroke-linejoin="round"/>'+
      '<path d="M28 44 48 28 69 44 V77 Q69 81 65 81 H31 Q28 81 28 77Z" fill="url(#paper)" stroke="#4089C5" stroke-width="2"/>'+
      '<path d="M41 79 V57 H55 V79" fill="url(#aeroBlue)" stroke="#226FBB" stroke-width="1.5"/>'+
      ('' if small else '<path d="M32 48v22" stroke="white" stroke-width="3" opacity=".75"/>')+
      '<path d="M19 44 47 19 77 44" fill="none" stroke="#C4EEFF" stroke-width="3" stroke-linecap="round"/>')


def photo_icon(small, variation=False):
    photo = ('<rect x="12" y="14" width="72" height="69" rx="7" fill="url(#metal)" stroke="#176AA7" stroke-width="2.5"/>'
             '<rect x="17" y="20" width="62" height="52" rx="2" fill="url(#aeroBlue)"/>'
             '<path d="M17 60Q32 43 44 56T79 48V72H17Z" fill="#65B95C"/>'
             '<path d="M17 69 40 42 56 57 68 46 79 57V72H17Z" fill="#2B9454"/>'
             '<path d="M40 42 46 48 51 44 56 57Z" fill="#EAF5F9"/>'
             '<circle cx="63" cy="35" r="7" fill="#FFE8A3"/>'
             '<path d="M23 25h33" stroke="#E9FDFF" stroke-width="3" opacity=".5"/>')
    if small: photo=photo.replace('<path d="M23 25h33" stroke="#E9FDFF" stroke-width="3" opacity=".5"/>','')
    return S(_shadow(47,31,85)+photo)


def note(small):
    return S('<path d="M40 65V31L68 25v35" fill="none" stroke="#185F9E" stroke-width="7" stroke-linecap="round"/>'
             '<path d="M40 35 68 29" stroke="#5DC7F5" stroke-width="4"/>'
             '<ellipse cx="33" cy="68" rx="10" ry="7" fill="url(#deepBlue)" stroke="#165C95" stroke-width="1.5"/>'
             '<ellipse cx="61" cy="64" rx="10" ry="7" fill="url(#deepBlue)" stroke="#165C95" stroke-width="1.5"/>')


def video(small):
    return S(_shadow()+
         '<rect x="13" y="23" width="70" height="53" rx="5" fill="url(#metal)" stroke="#346F9C" stroke-width="2"/>'
         '<rect x="21" y="30" width="54" height="38" rx="2" fill="url(#deepBlue)"/>'
         '<path d="M41 37 60 49 41 61Z" fill="#F9FCFF" stroke="#C3E7FF" stroke-width="1.5"/>'
         '<path d="M18 28v43M78 28v43" stroke="#4C819E" stroke-width="2.5" stroke-dasharray="5 5"/>')


def download(small):
    return S('<path d="M48 18v44m-17-17 17 18 17-18" stroke="#247ABD" stroke-width="8" fill="none" stroke-linejoin="round" stroke-linecap="round"/>'
             '<path d="M48 17v44m-16-16 16 17 16-17" stroke="#73D9FF" stroke-width="3" fill="none" stroke-linecap="round"/>'
             '<path d="M25 74h46" stroke="#2269A3" stroke-width="7" stroke-linecap="round"/>')


def recent(small):
    return S('<circle cx="48" cy="47" r="31" fill="url(#metal)" stroke="#327AB9" stroke-width="3"/>'
             '<circle cx="48" cy="47" r="23" fill="#F9FCFE" stroke="#C3D9EB" stroke-width="1.5"/>'
             '<path d="M48 30v18l13 8" fill="none" stroke="#277AC0" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>')


def favorites(small):
    return S('<path d="m48 13 11 24 26 4-19 18 5 27-23-12-23 12 5-27-19-18 26-4Z" fill="url(#aeroBlue)" stroke="#1D6EAE" stroke-width="2.5" stroke-linejoin="round"/>'
             '<path d="m48 20 8 18 19 4" stroke="#E4FBFF" stroke-width="3" opacity=".8" fill="none"/>')


def monitor(small):
    return S(_shadow(48,36,86)+'<rect x="10" y="19" width="76" height="53" rx="6" fill="url(#metal)" stroke="#5B7D97" stroke-width="2.5"/>'
             '<rect x="16" y="25" width="64" height="39" rx="2" fill="url(#deepBlue)"/>'
             '<path d="M18 28h55" stroke="#D9F9FF" stroke-width="3" opacity=".6"/>'
             '<path d="M48 72v8m-19 2h38" stroke="#7895AA" stroke-width="5" stroke-linecap="round"/>')


def trash(small, full=False):
    inner= '<path d="M39 33 49 19 59 36" fill="#FFF" stroke="#829EB5" stroke-width="2"/>' if full else ''
    return S(_shadow(48,26,86)+inner+
             '<path d="M26 33h44l-5 46q-16 6-33 0Z" fill="url(#metal)" stroke="#688CA6" stroke-width="2.6"/>'
             '<rect x="22" y="29" width="52" height="8" rx="3" fill="#B3D4E6" stroke="#638CAC" stroke-width="1.5"/>'
             '<path d="M37 24h22" stroke="#688CA6" stroke-width="5" stroke-linecap="round"/>'
             '<path d="M39 46v25m10-25v25m10-25v25" stroke="#80A9C0" stroke-width="3.2" stroke-linecap="round"/>')


def drive(small, system=False):
    emblem = '<path d="M48 49 37 59l11 10 11-10Z" fill="#238BCC"/>' if system else ''
    return S(_shadow(48,30,85)+
       '<path d="M17 32Q17 24 25 24h46q8 0 8 8v36q0 8-8 8H25q-8 0-8-8Z" fill="url(#metal)" stroke="#5C7F96" stroke-width="2.6"/>'
       '<path d="M21 57h54" stroke="#85A3B8" stroke-width="2"/>'
       '<ellipse cx="47" cy="41" rx="17" ry="9" fill="#E7F3F8" stroke="#98B3C5" stroke-width="2"/>'
       '<ellipse cx="47" cy="41" rx="5" ry="3" fill="#86A9C1"/>'
       '<circle cx="69" cy="67" r="3.8" fill="#39C68C"/>'+emblem)


def usb(small):
    return S(_shadow(48,22,87)+
       '<path d="M36 16h24q6 0 6 6v52q0 7-7 7H37q-7 0-7-7V22q0-6 6-6Z" fill="url(#metal)" stroke="#63839E" stroke-width="2.5"/>'
       '<rect x="36" y="21" width="24" height="23" rx="2" fill="#6F9EBE"/>'
       '<rect x="41" y="27" width="6" height="12" fill="#DDECF6"/>'
       '<rect x="52" y="27" width="6" height="12" fill="#DDECF6"/>'
       '<circle cx="48" cy="68" r="5" fill="#3087C0"/>')


def network(small):
    return S('<rect x="8" y="20" width="42" height="34" rx="4" fill="url(#metal)" stroke="#617E9B" stroke-width="2"/>'
             '<rect x="13" y="25" width="32" height="24" fill="url(#aeroBlue)"/>'
             '<rect x="48" y="37" width="40" height="34" rx="4" fill="url(#metal)" stroke="#617E9B" stroke-width="2"/>'
             '<rect x="53" y="42" width="30" height="24" fill="url(#aeroBlue)"/>'
             '<path d="M29 54v10h24M68 71v8M55 79h25" fill="none" stroke="#6789A5" stroke-width="3"/>')


def server(small):
    return S(_shadow(48,23,86)+
       '<rect x="30" y="12" width="38" height="70" rx="5" fill="url(#metal)" stroke="#66839C" stroke-width="2.5"/>'
       '<rect x="36" y="20" width="26" height="16" rx="2" fill="url(#deepBlue)"/>'
       '<rect x="36" y="44" width="26" height="10" rx="2" fill="#607F96"/>'
       '<rect x="36" y="60" width="26" height="10" rx="2" fill="#607F96"/>'
       '<circle cx="56" cy="49" r="2" fill="#7FFFBA"/><circle cx="56" cy="65" r="2" fill="#7FFFBA"/>')


def gear_path():
    return ('M43 12H53L57 23Q63 25 67 28L78 23L85 31L78 42Q80 48 78 54L86 63L78 73L67 69Q62 72 57 73L54 85H43L39 74Q34 72 29 69L18 74L11 64L19 54Q17 48 19 42L12 32L20 23L30 28Q34 25 39 23Z')


def device_group(small):
    return S('<rect x="9" y="19" width="68" height="48" rx="6" fill="url(#metal)" stroke="#62849D" stroke-width="2"/>'
             '<rect x="15" y="25" width="56" height="34" fill="url(#deepBlue)"/>'
             '<path d="M43 67v9M30 78h26" stroke="#6486A3" stroke-width="4"/>'
             '<path d="M70 46h13v31H70Z" fill="url(#metal)" stroke="#5F809A" stroke-width="2"/>'
             '<circle cx="77" cy="70" r="2" fill="#39C7A5"/>')


def tile(color, content, outline='#457EA6'):
    return (_shadow(48,29,87)+
      f'<rect x="14" y="12" width="68" height="68" rx="16" fill="url(#{color})" stroke="{outline}" stroke-width="2"/>'
      '<path d="M20 36Q18 18 36 18H60Q76 18 77 37Q49 29 20 36" fill="white" opacity=".25"/>'+content)


def browser(small):
    return S(_shadow(48,28,85)+
      '<circle cx="47" cy="46" r="31" fill="url(#glass)" stroke="#1673BD" stroke-width="2.3"/>'
      '<path d="M73 31Q64 50 46 46Q34 44 36 32Q22 38 24 54Q31 78 55 71Q76 67 73 43" fill="#164FBD" opacity=".9"/>'
      '<circle cx="57" cy="30" r="10" fill="#B6F4FF"/>'
      '<path d="M22 45Q30 22 51 18" fill="none" stroke="#A6EFFF" stroke-width="5" opacity=".7"/>')


def explorer(small):
    return S(tile('aeroBlue','<path d="M22 44q1-4 5-4h13l6 5h23q6 0 6 6v17q0 5-5 5H27q-5 0-5-5Z" fill="url(#folderBack)" stroke="#C99538" stroke-width="1.3"/>'
            '<path d="M23 53h50l-5 20H27q-5 0-4-5Z" fill="url(#folderFace)" stroke="#D89A35" stroke-width="1.4"/>'))


def terminal(small):
    return S(tile('darkTile','<rect x="23" y="29" width="51" height="36" rx="5" fill="#263744" stroke="#A9CADB" stroke-width="2"/>'
             '<path d="m33 40 9 8-9 8M48 57h15" fill="none" stroke="#E8F7FF" stroke-width="3.8" stroke-linecap="round" stroke-linejoin="round"/>','#607E92'))


def mail(small):
    return S(tile('mailTile','<rect x="23" y="33" width="50" height="34" rx="4" fill="url(#paper)" stroke="#ABD9F2" stroke-width="2"/>'
             '<path d="M24 36 48 54 72 36" fill="none" stroke="#47A5E1" stroke-width="3.6" stroke-linejoin="round"/>','#2C85CE'))


def settings(small):
    return S(_shadow()+
      f'<path d="{gear_path()}" fill="url(#metal)" stroke="#6488A2" stroke-width="2.5" stroke-linejoin="round"/>'
      '<circle cx="48" cy="48" r="21" fill="url(#deepBlue)" stroke="#6B95B8" stroke-width="2.5"/>'
      '<circle cx="48" cy="48" r="9" fill="url(#metal)" stroke="#5882A8" stroke-width="2"/>'
      '<path d="M24 33Q32 17 48 18" fill="none" stroke="white" opacity=".55" stroke-width="3"/>')


def photos(small):
    return S(tile('greenTile','<rect x="24" y="23" width="49" height="50" rx="5" fill="#F9FDFF" stroke="#DFEFFF" stroke-width="2"/>'
           '<rect x="29" y="29" width="39" height="37" fill="url(#aeroBlue)"/>'
           '<path d="M30 60 42 46 49 53 56 44 68 60v6H29Z" fill="#5ABF76"/>'
           '<path d="M34 60 47 45 59 60" fill="#288F82"/>'
           '<circle cx="58" cy="39" r="5" fill="#FFF1B0"/>','#1A9EAB'))


def office(small):
    return S(tile('orangeTile','<path d="M29 23h28l10 10v37H29Z" fill="#FDFBFA" stroke="#F7D4AA" stroke-width="2"/>'
           '<path d="M57 23v10h10" fill="#FFDDAD"/>'
           '<path d="M35 43h26M35 51h23M35 59h26" stroke="#E7893D" stroke-width="3"/>'
           '<path d="M23 33v43h35" fill="none" stroke="#D95722" stroke-width="5"/>','#D36C24'))


def nav(kind, small, dark):
    # Original small nav icons: translucent Aero circle and blue ink, not
    # glossy full-size illustrations squeezed into the explorer sidebar.
    fg='#BDEAFF' if dark else '#286AAB'
    stroke='#5AA7E3' if dark else '#B5D9F4'
    arrow = ('<path d="M56 32 40 48 56 64M40 48h26"/>' if kind=='back' else
             '<path d="M40 32 56 48 40 64M56 48H30"/>' if kind=='forward' else
             '<path d="M63 42a18 18 0 0 0-32-5m0-9v10h10M33 54a18 18 0 0 0 32 5m0 9V58H55"/>' if kind=='refresh' else
             '<circle cx="42" cy="42" r="15"/><path d="m53 54 14 14"/>')
    if kind in ('back','forward'):
        return S('<circle cx="48" cy="48" r="36" fill="#FFFFFF" fill-opacity=".14" '
                 f'stroke="{stroke}" stroke-width="3"/>'
                 f'<g fill="none" stroke="{fg}" stroke-width="5" stroke-linejoin="round" stroke-linecap="round">{arrow}</g>')
    return S(f'<g fill="none" stroke="{fg}" stroke-width="6" stroke-linejoin="round" stroke-linecap="round">{arrow}</g>')


RENDERERS={
    'home':house,'documents':lambda small: folder_style('documents', small),
    'pictures':photo_icon,'music':note,'videos':video,
    'downloads':download,'recent':recent,'favorites':favorites,
    'desktop':monitor,'trash':trash,'folder':lambda small: folder_style('folder',small),
    'folder-open':lambda small: folder_style('folder-open',small),
    'computer':monitor,'system':lambda small:drive(small,True),'disk':drive,
    'usb':usb,'network':network,'server':server,'devices':device_group,
    'browser':browser,'files':explorer,'terminal':terminal,'mail':mail,
    'settings':settings,'photos':photos,'office':office,
}

DISPLAY = [
    ('home','Home'),('documents','Documenti'),('pictures','Immagini'),
    ('music','Musica'),('videos','Video'),('downloads','Download'),
    ('recent','Recenti'),('favorites','Preferiti'),('desktop','Desktop'),
    ('trash','Cestino'),('folder','Cartella'),('folder-open','Cartella aperta'),
    ('computer','Computer'),('system','Sistema'),('disk','Disco dati'),
    ('usb','Unit\u00e0 USB'),('network','Rete locale'),('server','Server'),
    ('devices','Dispositivi'),('browser','Browser web'),('files','File'),
    ('terminal','Terminale'),('mail','Posta'),('settings','Impostazioni'),
    ('photos','Foto'),('office','Ufficio'),('back','Indietro'),
    ('forward','Avanti'),('refresh','Aggiorna'),('search','Cerca'),
]


def art(identity: str, size: int=96, dark: bool=False, full_trash: bool=False) -> str:
    small=size <=24
    if identity in ('back','forward','refresh','search'):
        result=nav(identity,small,dark)
    elif identity=='trash':
        result=trash(small,full_trash)
    else:
        result=RENDERERS[identity](small)
    ET.fromstring(result)
    return result


def artwork(name: str, category: str, variant: str, symbolic: bool, size: int=96):
    """Return chosen Melania screenshot-style icon, or None for existing pack.

    The variant is 'light'/'dark', while the binary symbolic flag reflects
    the -symbolic name or a symbolic folder in freedesktop inventories.
    """
    identity=REVERSE.get(name)
    if identity is None:
        return None
    if identity in ('back','forward','refresh','search') and category not in ('actions','apps'):
        return None
    # Leave unrelated symbolic names to the pack's own contrast-specific ink.
    if symbolic and identity not in ('back','forward','refresh','search'):
        return None
    if category not in ('actions','places','apps','devices','mimetypes','categories','panel','status'):
        return None
    if category in ('panel','status') and identity not in ('back','forward','refresh','search'):
        return None
    # Actions with homonymous concepts must be toolbar controls, not objects.
    if category=='actions' and identity not in ('back','forward','refresh','search'):
        return None
    return art(identity,size,variant=='dark',name=='user-trash-full')


def export(destination: Path):
    for theme, is_dark in [('Melania-Light',False),('Melania-Dark',True)]:
        for size in (16,24,32,48,64,96,128,256):
            for key,_ in DISPLAY:
                directory=destination/theme/f'{size}x{size}'/'melania-reference'
                directory.mkdir(parents=True,exist_ok=True)
                (directory/f'{key}.svg').write_text(art(key,size,is_dark),encoding='utf8')


if __name__=='__main__':
    p=argparse.ArgumentParser()
    p.add_argument('--export',type=Path)
    args=p.parse_args()
    if args.export: export(args.export)
    else: print('30 original Melania Classic Aero icon families',len(DISPLAY))