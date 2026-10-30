#!/usr/bin/env python3
"""Melania Fluent: original Windows-11-inspired icon language (not Microsoft assets).

All icons are original SVG geometry. Exact external branding is not imported.
The Flat Remix manifests supply *identifiers only*, never artwork.
"""
from __future__ import annotations
import hashlib
import html
import re

# Semantic IDs determine the visual metaphor, not any upstream graphic.
ALIASES = {
    'folder': 'folder inode-directory folder-symbolic folder-yellow',
    'folder-open': 'folder-open folder-open-symbolic',
    'documents': 'folder-documents user-documents folder-templates folder-text folder-documents-symbolic',
    'downloads': 'folder-download folder-downloads user-download user-downloads folder-download-symbolic',
    'pictures': 'folder-pictures user-pictures folder-images folder-image folder-photo folder-pictures-symbolic',
    'music': 'folder-music user-music folder-audio folder-music-symbolic',
    'videos': 'folder-videos user-videos folder-video folder-videos-symbolic',
    'desktop': 'user-desktop folder-desktop desktop',
    'home': 'user-home folder-home go-home',
    'public': 'folder-public folder-publicshare folder-shared',
    'recent': 'document-open-recent folder-recent user-recent recent',
    'favorites': 'user-bookmarks folder-bookmarks folder-favorites emblem-favorite starred',
    'trash': 'user-trash user-trash-symbolic',
    'trash-full': 'user-trash-full user-trash-full-symbolic',
    'computer': 'computer computer-desktop video-display display',
    'laptop': 'computer-laptop computer-notebook',
    'system-disk': 'drive-harddisk drive-harddisk-system drive-harddisk-root',
    'disk': 'drive-harddisk-data drive-harddisk-usb drive-multidisk',
    'usb': 'drive-removable-media drive-removable-media-usb media-removable media-flash',
    'network': 'network-workgroup network-local network-wired network-wired-symbolic',
    'server': 'network-server network-server-symbolic',
    'printer': 'printer printer-network printer-printing',
    'keyboard': 'input-keyboard',
    'mouse': 'input-mouse input-touchpad',
    'headphones': 'audio-headphones',
    'camera': 'camera-photo camera-video',
    'browser': 'web-browser internet-web-browser applications-internet org.melaniadesktop.Browser',
    'files': 'system-file-manager org.melaniadesktop.Files file-manager folder-manager melania-files',
    'terminal': 'utilities-terminal org.gnome.Terminal org.melaniadesktop.Terminal terminal xterm',
    'mail': 'internet-mail email mail-client evolution org.melaniadesktop.Mail',
    'settings': 'preferences-system org.melaniadesktop.Settings systemsettings gnome-control-center',
    'photos': 'applications-graphics image-viewer org.gnome.Loupe shotwell org.melaniadesktop.Photos',
    'office': 'applications-office libreoffice-startcenter office',
    'graphics': 'applications-graphics-drawing inkscape',
    'calculator': 'accessories-calculator calculator gnome-calculator',
    'calendar': 'x-office-calendar org.gnome.Calendar calendar',
    'editor': 'accessories-text-editor text-editor org.gnome.TextEditor',
    'help': 'help-browser help-about help-contents dialog-question',
    'archive': 'file-roller org.gnome.FileRoller utilities-file-archiver',
    'multimedia': 'applications-multimedia',
    'games': 'applications-games',
    'development': 'applications-development',
    'utilities': 'applications-utilities applications-accessories',
    'science': 'applications-science applications-education',
    'system': 'applications-system preferences-system-windows',
    'pdf': 'application-pdf application-x-pdf application-postscript',
    'text': 'text-x-generic text-plain',
    'word': 'x-office-document application-msword application-vnd.oasis.opendocument.text application-vnd.openxmlformats-officedocument.wordprocessingml.document',
    'sheet': 'x-office-spreadsheet application-vnd.ms-excel application-vnd.oasis.opendocument.spreadsheet application-vnd.openxmlformats-officedocument.spreadsheetml.sheet',
    'slides': 'x-office-presentation application-vnd.ms-powerpoint application-vnd.oasis.opendocument.presentation application-vnd.openxmlformats-officedocument.presentationml.presentation',
    'image-file': 'image-x-generic image-png image-jpeg image-svg+xml',
    'audio-file': 'audio-x-generic audio-mpeg audio-flac',
    'video-file': 'video-x-generic video-mp4 video-x-matroska',
    'zip': 'application-zip application-x-archive application-x-tar application-x-compressed-tar application-x-7z-compressed package-x-generic',
    'code': 'text-x-python text-x-script text-x-csrc application-x-executable',
    'back': 'go-previous go-previous-symbolic gtk-go-back-ltr',
    'forward': 'go-next go-next-symbolic gtk-go-forward-ltr',
    'up': 'go-up go-up-symbolic',
    'down': 'go-down go-down-symbolic',
    'refresh': 'view-refresh view-refresh-symbolic reload gtk-refresh',
    'search': 'edit-find edit-find-symbolic system-search search',
    'copy': 'edit-copy edit-copy-symbolic',
    'cut': 'edit-cut edit-cut-symbolic',
    'paste': 'edit-paste edit-paste-symbolic',
    'delete': 'edit-delete edit-delete-symbolic user-trash-symbolic-actions',
    'new': 'document-new document-new-symbolic',
    'new-folder': 'folder-new folder-new-symbolic',
    'save': 'document-save document-save-symbolic',
    'open': 'document-open document-open-symbolic',
    'print': 'document-print document-print-symbolic',
    'list': 'view-list view-list-symbolic',
    'grid': 'view-grid view-grid-symbolic',
    'zoom-in': 'zoom-in zoom-in-symbolic',
    'zoom-out': 'zoom-out zoom-out-symbolic',
    'undo': 'edit-undo edit-undo-symbolic',
    'redo': 'edit-redo edit-redo-symbolic',
    'close': 'window-close window-close-symbolic',
    'plus': 'list-add list-add-symbolic',
    'minus': 'list-remove list-remove-symbolic',
    'play': 'media-playback-start media-playback-start-symbolic',
    'pause': 'media-playback-pause media-playback-pause-symbolic',
    'stop': 'media-playback-stop media-playback-stop-symbolic',
    'power': 'system-shutdown system-shutdown-symbolic',
    'lock': 'system-lock-screen system-lock-screen-symbolic',
    'logout': 'system-log-out system-log-out-symbolic',
}
REVERSE = {}
for key, names in ALIASES.items():
    for n in names.split():
        REVERSE.setdefault(n, key)
# Deliberately resolve ambiguities in the proper freedesktop context below.
FOLDER_TYPES = {'folder','folder-open','documents','downloads','pictures','music','videos','desktop','public'}
FILE_TYPES = {'pdf','text','word','sheet','slides','image-file','audio-file','video-file','zip','code'}
UI_TYPES = {'back','forward','up','down','refresh','search','copy','cut','paste','delete','new','new-folder','save','open','print','list','grid','zoom-in','zoom-out','undo','redo','close','plus','minus','play','pause','stop','power','lock','logout'}

# 96x96 canvas, no external font dependency, transparent background, no Microsoft logos.
def _svg(body: str, defs: str = '') -> str:
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" '
            'width="96" height="96"><defs>' + defs + '</defs>' + body + '</svg>')

def _grad(id: str, a: str, b: str, c: str | None = None) -> str:
    if c is None:
        c = b
    return (f'<linearGradient id="{id}" x1="0%" y1="0%" x2="40%" y2="100%">'
            f'<stop stop-color="{a}"/><stop offset=".55" stop-color="{b}"/>'
            f'<stop offset="1" stop-color="{c}"/></linearGradient>')

def _shadow(cx=48, cy=84, rx=31):
    return f'<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="3.5" fill="#0F2646" opacity=".10"/>'

def _paths(body, stroke='#FFFFFF', width=5, fill='none'):
    return f'<g stroke="{stroke}" stroke-width="{width}" fill="{fill}" stroke-linejoin="round" stroke-linecap="round">{body}</g>'

FOLDER_COLOR = {
    'folder': ('#FFDE82','#FFC83D','#E89E21'),
    'folder-open': ('#FFDE82','#FFC83D','#E89E21'),
    'documents': ('#C6D9E6','#889FB8','#607D99'),
    'downloads': ('#75E7C6','#10AE91','#057E78'),
    'pictures': ('#78DDFC','#20A0E4','#0873C1'),
    'music': ('#FFB16A','#EF7E6D','#C65C9C'),
    'videos': ('#CFA4FE','#9874E9','#6E56C0'),
    'desktop': ('#81DAF5','#22B6D3','#087EA8'),
    'public': ('#FFDA7D','#F3B93A','#D99B22'),
}

def _folder(kind, compact):
    a,b,c=FOLDER_COLOR.get(kind,FOLDER_COLOR['folder'])
    defs=_grad('f',a,b,c)+_grad('tab',a,c)
    # New Windows-style flat front face, short tab, subtly offset back plate.
    base=(_shadow(48,84,36)+
       '<path d="M8 28Q8 23 14 23h24l8 8h35q7 0 7 7v7H8Z" fill="url(#tab)"/>'+
       '<path d="M10 37h74q7 0 7 8v30q0 8-9 8H13q-8 0-8-8V44q0-7 5-7Z" '
       'fill="url(#f)"/>'+
       '<path d="M11 39h72" stroke="#FFFFFF" stroke-width="2" opacity=".32"/>'+
       '<path d="M8 74h81" stroke="#172B48" stroke-width="2" opacity=".09"/>')
    if kind=='folder-open':
        base=(_shadow(48,84,36)+
              '<path d="M8 27h29l9 9h38v37H8Z" fill="url(#tab)"/>'+
              '<path d="M16 38h75L79 84H6Z" fill="url(#f)"/>'+
              '<path d="M18 40h69" stroke="#fff" opacity=".35" stroke-width="2"/>')
    glyphs={
      'documents':'<path d="M31 51h34M31 61h34M31 71h23"/>',
      'downloads':'<path d="M48 46v23m-11-11 11 11 11-11M34 75h28"/>',
      'pictures':'<path d="M25 71 40 53l11 12 9-10 13 16Z" fill="white" stroke="none"/><circle cx="65" cy="51" r="4" fill="white" stroke="none"/>',
      'music':'<path d="M44 67V46l21-4v20M44 51l21-4"/><ellipse cx="36" cy="68" rx="8" ry="5" fill="white" stroke="none"/><ellipse cx="57" cy="64" rx="8" ry="5" fill="white" stroke="none"/>',
      'videos':'<rect x="26" y="48" width="43" height="25" rx="3"/><path d="m43 53 14 8-14 8Z" fill="white" stroke="none"/>',
      'desktop':'<path d="M28 49h41v22H28Z"/><path d="M48 71v5M38 76h20"/>',
      'public':'<circle cx="41" cy="55" r="6"/><circle cx="57" cy="55" r="6"/><path d="M27 72q0-12 14-12t14 12M45 72q0-12 12-12t12 12"/>',
    }
    if kind in glyphs:
        base+=_paths(glyphs[kind],stroke='#FFFFFF',width=(6 if compact else 4.5))
    return _svg(base,defs)

def _document(kind,compact):
    shades={
      'pdf':('#F57070','#DD4446','#A92138'), 'text':('#B5D8F0','#629ECF','#346EA4'),
      'word':('#61B7F5','#267DD8','#174CA5'), 'sheet':('#57D7A8','#21976D','#10684E'),
      'slides':('#FFC37E','#F08B45','#C65231'), 'image-file':('#87D6F9','#328BD4','#2750A9'),
      'audio-file':('#BBA7F4','#7968D8','#5843A4'), 'video-file':('#B994F1','#9261CC','#6447A9'),
      'zip':('#FFE08C','#C89E4A','#A16A30'), 'code':('#A1C9EE','#667DB6','#405990')}
    a,b,c=shades[kind]; defs=_grad('tag',a,b,c)+_grad('paper','#FFFFFF','#EEF5FB','#DCE9F3')
    body=(_shadow(48,85,22)+
      '<path d="M22 11h37l16 16v56q0 5-5 5H25q-5 0-5-6V17q0-6 2-6Z" '
      'fill="url(#paper)" stroke="#A2B5C7" stroke-width="1.3"/>'+
      '<path d="M59 11v17h16Z" fill="#D1DFEC"/>'+
      '<path d="M23 16h31" stroke="white" stroke-opacity=".9" stroke-width="3"/>'+
      '<rect x="26" y="37" width="43" height="34" rx="5" fill="url(#tag)"/>')
    marks={
      'pdf':'<path d="M38 61q9-20 9-14 1 10 9 11m-18 3q13-9 21-5"/>',
      'text':'<path d="M33 47h29M33 55h29M33 63h22"/>',
      'word':'<path d="m33 49 4 15 6-11 6 11 5-15"/>',
      'sheet':'<rect x="34" y="44" width="27" height="20" rx="1"/><path d="M34 51h27M34 57h27M45 44v20"/>',
      'slides':'<rect x="34" y="43" width="26" height="19" rx="1"/><path d="m38 56 8-9 5 6 5-5M47 62v5"/>',
      'image-file':'<path d="m32 61 12-13 8 8 5-5 7 10Z" fill="#FFFFFF" stroke="none"/><circle cx="58" cy="46" r="3" fill="#FFFFFF" stroke="none"/>',
      'audio-file':'<path d="M43 59V44l17-3v15"/><ellipse cx="38" cy="60" rx="5" ry="3"/><ellipse cx="55" cy="57" rx="5" ry="3"/>',
      'video-file':'<path d="m40 46 18 9-18 9Z" fill="#FFFFFF" stroke="none"/>',
      'zip':'<path d="M46 43v25M42 45h8M42 52h8M42 59h8"/>',
      'code':'<path d="m41 47-9 8 9 8M55 47l9 8-9 8M51 44 46 66"/>',
    }
    body+=_paths(marks[kind],width=(5 if compact else 3.7))
    return _svg(body,defs)

def _home():
    defs=_grad('roof','#6BC5FA','#277FCB','#1F65B3')+_grad('wall','#F6FBFE','#DBEAF4')
    body=(_shadow()+
        '<path d="m13 46 35-31 35 31-9 10-26-22-26 22Z" fill="url(#roof)"/>'+
        '<path d="M25 45 48 26l23 19v34H25Z" fill="url(#wall)" stroke="#4E96D1" stroke-width="2"/>'+
        '<path d="M40 79V55h16v24" fill="#3D91D3"/>'+
        '<path d="m13 46 35-31 35 31" fill="none" stroke="#77D1F7" stroke-width="2" opacity=".75"/>'+
        '<rect x="29" y="52" width="8" height="8" rx="1" fill="#73CBF1"/>')
    return _svg(body,defs)

def _simple_shape(kind,compact):
    bg={
      'recent':('#9FCDF6','#3C8BCB'), 'favorites':('#88BFF8','#2C72D0'),
      'trash':('#EDF7FB','#9ABACE'), 'trash-full':('#EDF7FB','#9ABACE'),
      'computer':('#78D4FF','#107ABF'), 'laptop':('#78D4FF','#107ABF'),
      'system-disk':('#E5EDF3','#8AABBF'), 'disk':('#E5EDF3','#8AABBF'),
      'usb':('#E9F3F8','#8AAABC'), 'network':('#84CFEF','#2588BE'),
      'server':('#E0EFF7','#779DAF'), 'printer':('#E9F3F8','#8FA7BC'),
      'keyboard':('#DDEFF8','#859AB0'), 'mouse':('#DDEFF8','#859AB0'),
      'headphones':('#DDEFF8','#859AB0'), 'camera':('#DDEFF8','#859AB0')}
    a,b=bg[kind];defs=_grad('a',a,b)+_grad('screen','#89DEFC','#1686CA','#185798')
    body=_shadow()
    if kind=='recent':
        body+='<circle cx="48" cy="47" r="32" fill="url(#a)"/><circle cx="48" cy="47" r="25" fill="#F6FBFF"/><path d="M48 27v21l14 10" fill="none" stroke="#368AC6" stroke-width="6" stroke-linecap="round"/>'
    elif kind=='favorites':
        body+='<path d="m48 12 12 25 27 4-20 19 5 27-24-12-24 12 5-27-20-19 27-4Z" fill="url(#a)"/>'
    elif kind.startswith('trash'):
        body+=('<path d="M28 34h40l-4 45q-16 6-32 0Z" fill="url(#a)" stroke="#8AA6BB" stroke-width="2"/>'
              '<path d="M23 30h50v8H23Z" fill="#B9CEDC"/><path d="M38 23h20" stroke="#839BAC" stroke-width="6" stroke-linecap="round"/>'
              '<path d="M40 47v23M49 47v23M58 47v23" stroke="#92AABB" stroke-width="3"/>')
        if kind=='trash-full': body+='<path d="m35 44 8-8 11 9 7-5v16H35Z" fill="#F3F8FE" stroke="#B0C3D0" stroke-width="2"/>'
    elif kind in ('computer','laptop'):
        body+=('<rect x="13" y="18" width="70" height="51" rx="5" fill="#B3CBDC"/>'
               '<rect x="17" y="22" width="62" height="42" rx="2" fill="url(#screen)"/>'
               '<path d="M48 69v10M33 79h30" stroke="#94AFBF" stroke-width="7" stroke-linecap="round"/>')
        if kind=='laptop': body+='<path d="M14 68h68l10 12H4Z" fill="#D7E5EF" stroke="#89A6BB" stroke-width="2"/>'
    elif kind in ('disk','system-disk'):
        body+=('<rect x="16" y="23" width="64" height="57" rx="8" fill="url(#a)" stroke="#87A1B5" stroke-width="2"/>'
               '<path d="M18 44h60" stroke="#9EB9CB" stroke-width="2"/>'
               '<circle cx="65" cy="66" r="4" fill="#21BE96"/>')
        if kind=='system-disk':body+='<path d="M32 30h32" stroke="#FFFFFF" stroke-width="4" opacity=".75"/>'
    elif kind=='usb':
        body+=('<path d="M27 17h42v62H27Z" fill="url(#a)" stroke="#8BA7B9" stroke-width="2"/>'
               '<path d="M34 9h28v19H34Z" fill="#7D9DB3"/><path d="M41 15v7M55 15v7" stroke="#D7E6F1" stroke-width="5"/>')
    elif kind=='network':
        body+=('<rect x="9" y="22" width="44" height="35" rx="3" fill="url(#a)"/>'
               '<rect x="14" y="26" width="34" height="24" fill="url(#screen)"/>'
               '<rect x="43" y="43" width="44" height="34" rx="3" fill="url(#a)"/>'
               '<rect x="48" y="47" width="34" height="24" fill="url(#screen)"/>')
    elif kind=='server':
        body+=('<rect x="28" y="11" width="42" height="73" rx="7" fill="url(#a)" stroke="#829AAD" stroke-width="2"/>'
               '<rect x="34" y="19" width="30" height="21" rx="3" fill="url(#screen)"/>'
               '<path d="M35 49h28M35 66h28" stroke="#7893A5" stroke-width="7"/>'
               '<circle cx="59" cy="49" r="2.5" fill="#44DBA3"/><circle cx="59" cy="66" r="2.5" fill="#44DBA3"/>')
    elif kind=='printer':
        body+=('<rect x="27" y="15" width="42" height="30" fill="#F6FAFF" stroke="#93AFC2" stroke-width="2"/>'
               '<rect x="16" y="36" width="64" height="37" rx="6" fill="url(#a)"/>'
               '<rect x="27" y="56" width="42" height="26" fill="#FFFFFF"/>'
               '<path d="M34 65h28M34 72h22" stroke="#92B8D3" stroke-width="2"/>')
    elif kind=='keyboard':
        body+='<rect x="8" y="32" width="80" height="44" rx="6" fill="url(#a)" stroke="#7897AF" stroke-width="2"/>'
        body+=''.join(f'<rect x="{15+col*12}" y="{40+row*12}" width="8" height="7" rx="1.5" fill="#F9FDFF"/>' for row in range(2) for col in range(6))
        body+='<rect x="28" y="65" width="40" height="5" rx="2" fill="#F9FDFF"/>'
    elif kind=='mouse':
        body+='<path d="M48 15q24 0 24 24v19q0 25-24 25T24 58V39q0-24 24-24Z" fill="url(#a)" stroke="#8EA8B8" stroke-width="2"/><path d="M48 15v28M24 43h48" stroke="#9AB8CA" stroke-width="3"/>'
    elif kind=='headphones':
        body+='<path d="M18 59V43q0-30 30-30t30 30v16" fill="none" stroke="#839AB7" stroke-width="11"/><rect x="12" y="53" width="17" height="27" rx="6" fill="url(#a)"/><rect x="67" y="53" width="17" height="27" rx="6" fill="url(#a)"/>'
    else:
        body+='<rect x="19" y="28" width="58" height="46" rx="8" fill="url(#a)"/><circle cx="48" cy="51" r="15" fill="#5B96C4"/><circle cx="48" cy="51" r="8" fill="#A1DBF3"/>'
    return _svg(body,defs)

# App and category metaphors sit in a compact radiused product-icon silhouette,
# but are not one repeated generic badge. Important apps have distinct layers.
APP_PALETTE={
 'browser':('#6ED8E6','#0BAAB5','#146DD0'),
 'files':('#FFE38B','#F1B840','#DF8D29'),
 'terminal':('#5D748D','#283C58','#122237'),
 'mail':('#8ADFFC','#249DEA','#1869C9'),
 'settings':('#9BD2F0','#3A91C7','#2B5E9E'),
 'photos':('#FFB4CE','#EC76B8','#756CDF'),
 'office':('#FFC98E','#F3965C','#DB644B'),
 'graphics':('#EAB0FF','#B66FEA','#775BCD'),
 'calculator':('#AAD8F7','#5A9ACC','#2F68AC'),
 'calendar':('#FFE3A6','#F49F69','#E86C63'),
 'editor':('#9BD7F0','#4CA0DD','#3865C3'),
 'help':('#90E8FC','#269EDC','#1F79C9'),
 'archive':('#FFDFAC','#E8A752','#C4803C'),
 'multimedia':('#B7A7F9','#8274DD','#5C51B3'),
 'games':('#91DAF3','#4B9DE4','#4972C5'),
 'development':('#A3CAFC','#5385DE','#3856B4'),
 'utilities':('#B9DDF5','#6799D1','#4970AB'),
 'science':('#9FE6D8','#4DBBB2','#3E829D'),
 'system':('#C1D9EF','#7A9CBC','#506E9F'),
}

def _app(kind, compact, name=None):
    if kind not in APP_PALETTE: kind='utilities'
    a,b,c=APP_PALETTE[kind];defs=_grad('a',a,b,c)+_grad('b','#F5FCFF','#BFD9EE','#7DB2DB')
    if kind=='browser':
        body=(_shadow()+
          '<circle cx="48" cy="46" r="35" fill="url(#a)"/>'+
          '<path d="M79 42q-4-29-37-30-22 0-32 27 18-15 36-9 9 4 12 12-17-5-29 5-8 8-9 21 19 21 46 7Q85 63 79 42Z" fill="#32C2C6"/>'+
          '<path d="M73 34Q88 66 51 82 24 89 14 64q20 6 35-9 15-18 24-21Z" fill="#146EC3"/>'+
          '<path d="M59 41q-2 14-19 16-11 0-15-6Q30 31 49 32q10 0 10 9Z" fill="#D2F5F8"/>')
        return _svg(body,defs)
    if kind=='files':
        body=(_shadow()+
              '<rect x="16" y="17" width="64" height="62" rx="16" fill="#3598E2"/>'+
              '<path d="M24 35h18l6 7h25v26q0 6-6 6H29q-6 0-6-6Z" fill="url(#a)"/>'+
              '<path d="M26 42h45" stroke="#FFE6A0" stroke-width="2"/>')
        return _svg(body,defs)
    if kind=='terminal':
        return _svg(_shadow()+
           '<rect x="15" y="15" width="66" height="65" rx="15" fill="url(#a)"/>'+
           '<path d="m29 39 12 11-12 11M47 63h18" fill="none" stroke="#EAF5FF" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>'
           ,defs)
    if kind=='mail':
        return _svg(_shadow()+
           '<rect x="12" y="24" width="72" height="53" rx="12" fill="url(#a)"/>'+
           '<path d="m16 35 32 24 32-24" fill="none" stroke="#E8F7FE" stroke-width="6" stroke-linejoin="round"/>'+
           '<path d="m17 72 24-21m38 21L55 51" stroke="#CDEEFF" stroke-width="4"/>',defs)
    if kind=='settings':
        # 8-tooth gear with centered hub, no app-tile square.
        import math
        points=[]
        for j in range(32):
            th=2*math.pi*j/32-math.pi/2
            rad=38 if j%4 in (0,1) else 31
            points.append(f'{48+rad*math.cos(th):.2f},{48+rad*math.sin(th):.2f}')
        return _svg(_shadow()+f'<polygon points="{" ".join(points)}" fill="url(#a)"/>'+
                    '<circle cx="48" cy="48" r="17" fill="#E7F5FF"/>'+
                    '<circle cx="48" cy="48" r="9" fill="#5D9BC7"/>',defs)
    glyphs={
      'photos':'<path d="m21 66 18-25 17 18 9-11 14 18Z" fill="#FFFFFF" stroke="none"/><circle cx="65" cy="36" r="6" fill="#FFF3BA" stroke="none"/>',
      'office':'<path d="M27 27h31v40H27Z"/><path d="M43 37h26v37H43Z"/><path d="M31 35h15M31 46h16M49 52h13M49 62h13"/>',
      'graphics':'<path d="M23 65q15-45 33-38t17 23q-4 24-29 23Z"/><circle cx="46" cy="38" r="3" fill="#FFF" stroke="none"/><circle cx="60" cy="50" r="3" fill="#FFF" stroke="none"/>',
      'calculator':'<rect x="26" y="17" width="44" height="62" rx="6"/><rect x="32" y="24" width="32" height="15" rx="3"/><path d="M36 50h7M57 50h7M36 63h7M57 63h7"/>',
      'calendar':'<rect x="20" y="23" width="56" height="55" rx="7"/><path d="M21 40h54M34 18v13M62 18v13M34 50h8M52 50h9M34 62h8"/>',
      'editor':'<rect x="26" y="18" width="44" height="62" rx="4"/><path d="M35 35h26M35 45h26M35 55h22M35 65h18"/>',
      'help':'<circle cx="48" cy="48" r="29"/><path d="M39 37q1-10 12-10t11 10q-1 8-13 13v7M49 66v2"/>',
      'archive':'<rect x="23" y="21" width="50" height="55" rx="5"/><path d="M44 25v44M40 29h8M40 38h8M40 47h8M40 56h8"/>',
      'multimedia':'<path d="M29 25 70 48 29 71Z" fill="#FFFFFF" stroke="none"/>',
      'games':'<path d="M27 42h43q10 0 12 14l2 12q0 10-10 6L59 64H36L21 74q-10 4-10-6l2-12q2-14 14-14Z"/><path d="M28 52h16M36 44v16"/><circle cx="61" cy="51" r="3"/><circle cx="71" cy="59" r="3"/>',
      'development':'<path d="m38 30-17 18 17 18M58 30l17 18-17 18M55 24 41 74"/>',
      'utilities':'<path d="M28 27 68 67M68 27 28 67"/><circle cx="28" cy="27" r="8"/><circle cx="68" cy="67" r="8"/>',
      'science':'<path d="M37 19h22M43 19v26L27 73q-5 7 5 7h32q10 0 5-7L53 45V19M33 64h30"/>',
      'system':'<circle cx="48" cy="48" r="28" stroke-dasharray="12 6"/><circle cx="48" cy="48" r="12"/>',
    }
    body=_shadow()+ '<rect x="12" y="12" width="72" height="72" rx="19" fill="url(#a)"/>'
    body += _paths(glyphs[kind],width=(6 if compact else 4.5))
    return _svg(body,defs)

# Last-resort app monograms: recognisable and distinct instead of several
# thousand identical generic "Utilities" tiles. No external brand assets.
def _fallback_app(name,compact):
    raw=name.split('.')[-1].replace('_','-').replace(' ','-')
    tokens=[x for x in re.split(r'[-.]',raw) if x and x not in ('org','com','net','io','desktop')]
    label=''.join(x[0] for x in tokens[-2:]).upper() if len(tokens)>1 else raw[:2].upper()
    label=''.join(x for x in label if x.isascii() and x.isalnum())[:2] or 'A'
    digest=hashlib.sha256(name.encode()).digest()
    palette=[('#8CCEF5','#2969C8'),('#98E7DD','#16968D'),('#F6C17D','#D47A46'),
             ('#DDB0FC','#9159D4'),('#FFC0D9','#CE6A99'),('#B7D1FF','#577ECB'),
             ('#B9E6A1','#70A757'),('#D3DBEC','#758BA6'),('#F4C2A4','#C76F5B'),
             ('#A4DAED','#327DAD'),('#D9C4F9','#8661B8'),('#F3DF99','#BA9949')]
    a,b=palette[digest[0]%len(palette)]
    # A layer and a clean monogram, visually matching Windows 11 generic UWP apps.
    defs=_grad('g',a,b)
    body=(_shadow()+
          '<rect x="11" y="11" width="74" height="74" rx="19" fill="url(#g)"/>'+
          '<path d="M17 26Q19 16 34 16h29" stroke="#FFFFFF" opacity=".24" stroke-width="3" fill="none"/>')
    # Font is never embedded. On Linux the system can use its installed UI font.
    body+=f'<text x="48" y="61" text-anchor="middle" font-family="DejaVu Sans,sans-serif" font-size="{34 if len(label)==1 else 29}" font-weight="bold" fill="#FFFFFF">{html.escape(label)}</text>'
    return _svg(body,defs)

# True line-art system glyphs (no blue round background).
ACTION_SHAPES={
 'back':'<path d="m57 22-26 26 26 26M31 48h46"/>',
 'forward':'<path d="m39 22 26 26-26 26M65 48H19"/>',
 'up':'<path d="M22 58 48 32l26 26M48 33v43"/>',
 'down':'<path d="m22 38 26 26 26-26M48 64V19"/>',
 'refresh':'<path d="M72 42A25 25 0 0 0 28 31m0-12v17h17M24 53A25 25 0 0 0 69 65m0 12V59H52"/>',
 'search':'<circle cx="42" cy="41" r="19"/><path d="m56 56 21 21"/>',
 'copy':'<rect x="22" y="17" width="42" height="44" rx="5"/><rect x="34" y="31" width="42" height="44" rx="5"/>',
 'cut':'<circle cx="26" cy="67" r="8"/><circle cx="70" cy="67" r="8"/><path d="m30 26 37 35M67 26 30 61"/>',
 'paste':'<rect x="26" y="27" width="46" height="51" rx="5"/><path d="M38 28v-7h23v7M36 45h25M36 55h20"/>',
 'delete':'<path d="M29 31h38l-5 45H34ZM24 31h48M38 23h20M43 43v22M54 43v22"/>',
 'new':'<path d="M25 14h34l14 14v52H25ZM59 14v14h14M48 45v27M35 59h26"/>',
 'new-folder':'<path d="M16 31h28l8 9h27l-7 34H20Z"/><path d="M62 18v23M50 29h24"/>',
 'save':'<path d="M25 17h42l11 11v51H25ZM35 17v23h30V17M34 78V54h33v24"/>',
 'open':'<path d="M14 34h30l7 8h29l-8 31H20Z"/>',
 'print':'<path d="M28 20h40v20H28ZM18 38h60v31H18ZM28 56h40v23H28Z"/>',
 'list':'<path d="M34 29h43M34 48h43M34 67h43"/><circle cx="21" cy="29" r="3"/><circle cx="21" cy="48" r="3"/><circle cx="21" cy="67" r="3"/>',
 'grid':'<rect x="20" y="20" width="23" height="23" rx="3"/><rect x="53" y="20" width="23" height="23" rx="3"/><rect x="20" y="53" width="23" height="23" rx="3"/><rect x="53" y="53" width="23" height="23" rx="3"/>',
 'zoom-in':'<circle cx="42" cy="42" r="20"/><path d="M42 31v22M31 42h22m4 15 19 19"/>',
 'zoom-out':'<circle cx="42" cy="42" r="20"/><path d="M31 42h22m4 15 19 19"/>',
 'undo':'<path d="m38 27-20 21 20 20M20 48h37q18 0 18 17"/>',
 'redo':'<path d="m58 27 20 21-20 20M76 48H39q-18 0-18 17"/>',
 'close':'<path d="m28 28 40 40M68 28 28 68"/>',
 'plus':'<path d="M48 23v50M23 48h50"/>',
 'minus':'<path d="M23 48h50"/>',
 'play':'<path d="m31 19 45 29-45 29Z"/>',
 'pause':'<rect x="25" y="21" width="15" height="54" rx="2"/><rect x="56" y="21" width="15" height="54" rx="2"/>',
 'stop':'<rect x="26" y="26" width="44" height="44" rx="4"/>',
 'power':'<path d="M48 17v34M30 29a28 28 0 1 0 36 0"/>',
 'lock':'<rect x="26" y="42" width="44" height="35" rx="5"/><path d="M36 42v-12a12 12 0 0 1 24 0v12"/>',
 'logout':'<path d="M42 22H24v52h18M46 48h30M62 34l14 14-14 14"/>',
}

def _action(kind,dark,compact):
    ink='#F4FAFF' if dark else '#335579'
    # 16/20/24 icon weights mimic Fluent Regular instead of scaling 96px hard.
    stroke=(7 if compact else 5.4)
    shape=ACTION_SHAPES.get(kind,ACTION_SHAPES['new'])
    return _svg(_paths(shape,ink,stroke))

def _status(name,dark):
    ink='#F7FAFF' if dark else '#395572'
    n=name.lower()
    if 'wireless' in n or 'wifi' in n:
        shape='<path d="M12 35q36-31 72 0M24 48q24-20 48 0M37 61q11-9 22 0"/><circle cx="48" cy="73" r="4" fill="currentColor" stroke="none"/>'
    elif 'battery' in n:
        shape='<rect x="18" y="28" width="58" height="40" rx="6"/><path d="M77 40h6v15h-6"/><rect x="25" y="36" width="37" height="24" rx="2" fill="currentColor" stroke="none"/>'
        if 'low' in n or 'empty' in n:shape=shape.replace('width="37"','width="8"')
        if '50' in n or 'half' in n:shape=shape.replace('width="37"','width="21"')
    elif 'audio' in n or 'volume' in n:
        shape='<path d="M15 41h15l17-13v42L30 55H15Z"/><path d="M56 38q12 11 0 20M65 28q23 20 0 40"/>'
    elif 'bluetooth' in n:
        shape='<path d="M45 18v60l21-19-35-29M45 18l21 20-35 28"/>'
    elif 'network' in n or 'ethernet' in n:
        shape='<rect x="28" y="18" width="40" height="25" rx="4"/><path d="M38 43v12M58 43v12M19 56h58v19H19Z"/>'
    elif 'microphone' in n or 'input' in n:
        shape='<rect x="39" y="16" width="18" height="42" rx="9"/><path d="M29 47q0 21 19 21t19-21M48 68v13M37 81h22"/>'
    elif 'notification' in n or 'bell' in n:
        shape='<path d="M25 63h46l-7-12V39q0-17-16-17T32 39v12ZM43 74q5 9 11 0"/>'
    else:
        shape=ACTION_SHAPES['settings'] if 'settings' in ACTION_SHAPES else ACTION_SHAPES['new']
    if any(x in n for x in ('muted','disabled','off','disconnected')):
        shape+='<path d="m65 66 17 17M82 66 65 83" stroke="#E96671" stroke-width="7"/>'
    return _svg('<g color="'+ink+'">'+_paths(shape,ink,6.2)+'</g>')

def _generic(name,category,variant,compact):
    # Family-specific and semantically meaningful fallbacks; unique visuals where
    # possible; no thousands of identical bright-blue square placeholders.
    n=name.lower().replace('_','-')
    if category=='mimetypes':
        if any(k in n for k in ('pdf','postscript')):kind='pdf'
        elif any(k in n for k in ('spreadsheet','excel','csv','calc')):kind='sheet'
        elif any(k in n for k in ('presentation','powerpoint')):kind='slides'
        elif any(k in n for k in ('word','document','writer','odt')):kind='word'
        elif any(k in n for k in ('image','photo','jpeg','png','svg')):kind='image-file'
        elif any(k in n for k in ('audio','music','mp3','flac','ogg')):kind='audio-file'
        elif any(k in n for k in ('video','movie','mp4')):kind='video-file'
        elif any(k in n for k in ('zip','archive','tar','compressed','package')):kind='zip'
        elif any(k in n for k in ('script','code','python','csrc','java')):kind='code'
        else:kind='text'
        return _document(kind,compact)
    if category=='places':
        if 'network' in n or 'server' in n:return _simple_shape('server',compact)
        if 'trash' in n:return _simple_shape('trash',compact)
        if 'home' in n:return _home()
        if 'music' in n:return _folder('music',compact)
        if 'download' in n:return _folder('downloads',compact)
        if 'photo' in n or 'image' in n or 'picture' in n:return _folder('pictures',compact)
        return _folder('folder',compact)
    if category=='devices':
        if 'printer' in n:return _simple_shape('printer',compact)
        if 'keyboard' in n:return _simple_shape('keyboard',compact)
        if 'mouse' in n:return _simple_shape('mouse',compact)
        if 'camera' in n:return _simple_shape('camera',compact)
        if 'drive' in n or 'disk' in n:return _simple_shape('disk',compact)
        return _simple_shape('computer',compact)
    if category in ('actions','emblems'):
        choices=[('copy','copy'),('paste','paste'),('cut','cut'),('search','search'),('find','search'),
                 ('add','plus'),('new','new'),('remove','minus'),('delete','delete'),('close','close'),
                 ('next','forward'),('back','back'),('prev','back'),('down','down'),('up','up'),
                 ('refresh','refresh'),('sync','refresh'),('print','print'),('save','save'),
                 ('play','play'),('pause','pause'),('stop','stop'),('lock','lock'),('open','open')]
        for token,target in choices:
            if token in n:return _action(target,variant=='dark',compact)
        return _action('new',variant=='dark',compact)
    if category in ('status','panel','animations'):
        return _status(name,variant=='dark')
    if category=='categories':
        for token,target in (('internet','browser'),('graphics','graphics'),('game','games'),('office','office'),
                             ('network','browser'),('develop','development'),('science','science'),('educat','science'),
                             ('multimedia','multimedia'),('system','system'),('utilit','utilities'),('accessor','utilities')):
            if token in n:return _app(target,compact)
        return _app('utilities',compact)
    # Apps fallback: semantically categorized; color and metaphor instead of a
    # random monogram on a repetitive square. Specialized app icons are future work.
    for token,target in (('browser','browser'),('firefox','browser'),('chrom','browser'),('network','browser'),
                         ('mail','mail'),('thunderbird','mail'),('file','files'),('nautilus','files'),
                         ('terminal','terminal'),('console','terminal'),('settings','settings'),
                         ('photo','photos'),('image','photos'),('paint','graphics'),('inkscape','graphics'),
                         ('music','multimedia'),('video','multimedia'),('media','multimedia'),
                         ('calendar','calendar'),('calc','calculator'),('office','office'),
                         ('archive','archive'),('editor','editor'),('code','development'),
                         ('game','games'),('help','help')):
        if token in n:return _app(target,compact)
    return _fallback_app(name,compact)

def resolve(name: str, category: str) -> str | None:
    if name.endswith('-symbolic'):
        base=name[:-9]
        if base in REVERSE: return REVERSE[base]
    if category=='actions':
        for k in UI_TYPES:
            if name in ALIASES.get(k,'').split():return k
    if category in ('apps','categories') and name in ('applications-graphics',):return 'photos'
    if category=='mimetypes' and name=='application-pdf':return 'pdf'
    if category=='places' and name in ('folder', 'folder-open'):return name
    return REVERSE.get(name)

def _symbolic(kind, variant):
    ink='#F4FAFF' if variant=='dark' else '#395674'
    glyphs={
        'folder':'<path d="M12 34h26l9 9h36l-8 31H19Z"/>',
        'folder-open':'<path d="M12 34h26l9 9h36l-8 31H19Z"/>',
        'documents':'<path d="M20 26h27l8 8h23v40H20Z"/><path d="M33 49h30M33 60h26"/>',
        'downloads':'<path d="M19 31h57v42H19Z"/><path d="M48 40v23m-9-9 9 9 9-9"/>',
        'pictures':'<rect x="17" y="24" width="62" height="49" rx="4"/><path d="m20 67 20-23 16 18 9-10 11 15"/><circle cx="64" cy="37" r="4"/>',
        'music':'<path d="M40 66V31l27-5v33M40 37l27-5"/><ellipse cx="32" cy="70" rx="8" ry="4"/><ellipse cx="59" cy="63" rx="8" ry="4"/>',
        'videos':'<rect x="20" y="27" width="57" height="43" rx="4"/><path d="m40 37 20 12-20 12Z"/>',
        'desktop':'<rect x="17" y="21" width="62" height="47" rx="4"/><path d="M48 69v10M33 79h30"/>',
        'home':'<path d="M16 46 48 19l32 27M26 43v35h44V43M40 78V55h16v23"/>',
        'trash':'<path d="M31 34h35l-5 43H35ZM25 34h47M39 24h19M43 46v21M55 46v21"/>',
        'trash-full':'<path d="M31 34h35l-5 43H35ZM25 34h47M39 24h19M43 46v21M55 46v21"/>',
        'computer':'<rect x="17" y="22" width="63" height="43" rx="4"/><path d="M48 66v10M33 76h30"/>',
        'server':'<rect x="30" y="16" width="38" height="63" rx="5"/><path d="M37 38h23M37 57h23"/>',
        'network':'<rect x="13" y="22" width="40" height="33" rx="3"/><rect x="43" y="45" width="40" height="33" rx="3"/>',
    }
    if kind in UI_TYPES:return _action(kind,variant=='dark',True)
    body=glyphs.get(kind)
    if body:return _svg(_paths(body,ink,5.5))
    if kind in FILE_TYPES:return _svg(_paths('<path d="M25 15h33l14 14v50H25ZM58 15v14h14M34 48h29M34 59h25"/>',ink,5.5))
    if kind in APP_PALETTE:return _svg(_paths('<rect x="18" y="17" width="60" height="61" rx="14"/><path d="M18 37h60M33 26h2M47 26h2"/>',ink,5.5))
    return _svg(_paths('<rect x="21" y="24" width="54" height="51" rx="7"/><path d="M34 42h29M34 55h24"/>',ink,5.5))

def artwork(name: str,category: str,variant='light',size=96) -> str:
    kind=resolve(name,category)
    compact=size<=24
    if category in ('status','panel','animations'):return _status(name,variant=='dark')
    if name.endswith('-symbolic') or 'symbolic' in category:
        return _symbolic(kind or 'unknown',variant)
    symbolic=name.endswith('-symbolic') or category in ('actions','panel','status','emblems')
    if kind is None:return _generic(name,category,variant,compact)
    if kind in FOLDER_TYPES:return _folder(kind,compact)
    if kind in FILE_TYPES:return _document(kind,compact)
    if kind=='home':return _home()
    if kind in ('recent','favorites','trash','trash-full','computer','laptop','system-disk','disk','usb','network','server','printer','keyboard','mouse','headphones','camera'):
        return _simple_shape(kind,compact)
    if kind in UI_TYPES:return _action(kind,variant=='dark',compact)
    if kind in APP_PALETTE:return _app(kind,compact)
    return _generic(name,category,variant,compact)

# Independent families which are visually curated (not filesystem aliases).
CORE=[
 ('Cartella','folder','places'),('Aperta','folder-open','places'),
 ('Documenti','folder-documents','places'),('Download','folder-download','places'),
 ('Immagini','folder-pictures','places'),('Musica','folder-music','places'),
 ('Video','folder-videos','places'),('Desktop','folder-desktop','places'),
 ('Home','user-home','places'),('Pubblica','folder-publicshare','places'),
 ('Recenti','document-open-recent','actions'),('Preferiti','user-bookmarks','places'),
 ('Cestino','user-trash','places'),('Cestino pieno','user-trash-full','places'),
 ('Computer','computer','apps'),('Portatile','computer-laptop','devices'),
 ('Disco','drive-harddisk','apps'),('USB','media-flash','devices'),
 ('Rete','network-workgroup','places'),('Server','network-server','places'),
 ('Stampante','printer','devices'),('Browser','web-browser','apps'),
 ('File manager','system-file-manager','apps'),('Terminale','utilities-terminal','apps'),
 ('Posta','internet-mail','apps'),('Impostazioni','preferences-system','apps'),
 ('Foto','image-viewer','apps'),('Ufficio','applications-office','apps'),
 ('Grafica','applications-graphics-drawing','apps'),('Calcolatrice','accessories-calculator','apps'),
 ('Calendario','x-office-calendar','apps'),('Editor','accessories-text-editor','apps'),
 ('Aiuto','help-browser','apps'),('PDF','application-pdf','mimetypes'),
 ('Testo','text-x-generic','mimetypes'),('Documento','x-office-document','mimetypes'),
 ('Foglio','x-office-spreadsheet','mimetypes'),('Presentazione','x-office-presentation','mimetypes'),
 ('Immagine','image-x-generic','mimetypes'),('Archivio','application-zip','mimetypes'),
 ('Indietro','go-previous','actions'),('Avanti','go-next','actions'),
 ('Su','go-up','actions'),('Aggiorna','view-refresh','actions'),
 ('Cerca','edit-find','actions'),('Copia','edit-copy','actions'),
 ('Incolla','edit-paste','actions'),('Elimina','edit-delete','actions'),
]
