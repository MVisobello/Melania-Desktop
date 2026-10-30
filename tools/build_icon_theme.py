#!/usr/bin/env python3
"""Generate Melania Aero icons: editable SVG sources + freedesktop theme PNG sizes.
Only the original Melania wordmark/icon supplied by the user is imported.
Requires CairoSVG and Pillow for bitmap exports/previews; SVG-only build supports --svg-only.
"""
import argparse
import configparser
import io
import math
import shutil
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'branding' / 'Melania'
SIZES = (16, 24, 32, 48, 64, 128, 256)

# All icons use the same 96-unit canvas, silhouette treatment, lighting direction
# and radius family. Colors are controlled centrally to keep the set cohesive.
PALETTES = {
    'blue': ('#72c9ff','#3294f9','#0953bb'),
    'azure': ('#7bdbff','#269bd7','#065baf'),
    'violet': ('#c1a7ff','#7d68e6','#3a4caa'),
    'coral': ('#ffafaf','#f66d74','#bc3d74'),
    'amber': ('#ffe4a0','#eeae43','#c47a2b'),
    'mint': ('#8deed8','#31bba5','#147b87'),
    'indigo': ('#94b7ff','#4d77da','#293e91'),
    'slate': ('#a7c7e0','#5f84a9','#294568'),
    'green': ('#9de6a5','#4bbf78','#167b55'),
}

# Category, motif, palette, Italian preview title
ICONS = {
 'places': {
  'folder': ('folder','blue','Cartella'),
  'folder-open': ('folder-open','blue','Cartella aperta'),
  'folder-documents': ('folder-documents','blue','Documenti'),
  'folder-download': ('folder-download','blue','Download'),
  'folder-pictures': ('folder-pictures','blue','Immagini'),
  'folder-music': ('folder-music','blue','Musica'),
  'folder-videos': ('folder-videos','blue','Video'),
  'folder-projects': ('folder-projects','blue','Progetti'),
  'folder-publicshare': ('folder-publicshare','blue','Condivisi'),
  'folder-templates': ('folder-templates','blue','Modelli'),
  'folder-favorites': ('folder-favorites','blue','Preferiti'),
  'folder-recent': ('folder-recent','blue','Recenti'),
  'user-home': ('home','blue','Home'),
  'user-desktop': ('desktop','indigo','Scrivania'),
  'user-trash': ('trash','slate','Cestino'),
  'user-trash-full': ('trash-full','slate','Cestino pieno'),
  'network-workgroup': ('network','azure','Rete'),
  'network-server': ('server','slate','Server'),
 },
 'apps': {
  'org.melaniadesktop.Files': ('folder-app','blue','Melania Files'),
  'org.melaniadesktop.Settings': ('gear','slate','Impostazioni'),
  'org.melaniadesktop.Panel': ('panel','blue','Melania Panel'),
  'org.melaniadesktop.Desktop': ('desktop','indigo','Melania Desktop'),
  'system-file-manager': ('folder-app','blue','Gestore file'),
  'preferences-system': ('gear','slate','Impostazioni'),
  'preferences-desktop-theme': ('palette','violet','Aspetto'),
  'preferences-system-network': ('wifi','blue','Rete'),
  'preferences-system-bluetooth': ('bluetooth','blue','Bluetooth'),
  'preferences-system-power': ('power','green','Alimentazione'),
  'preferences-desktop-notification': ('bell','coral','Notifiche'),
  'preferences-system-privacy': ('shield','indigo','Privacy'),
  'preferences-system-display': ('monitor','blue','Schermo'),
  'applications-system': ('gear','indigo','Sistema'),
  'applications-internet': ('globe','azure','Internet'),
  'applications-multimedia': ('play','coral','Multimedia'),
  'applications-graphics': ('palette','violet','Grafica'),
  'applications-office': ('docs','blue','Ufficio'),
  'applications-games': ('game','violet','Giochi'),
  'applications-accessories': ('tools','slate','Accessori'),
  'utilities-terminal': ('terminal','slate','Terminale'),
  'system-software-install': ('bag','azure','Software'),
  'system-software-update': ('update','green','Aggiornamenti'),
  'accessories-calculator': ('calculator','blue','Calcolatrice'),
  'x-office-calendar': ('calendar','coral','Calendario'),
  'internet-mail': ('mail','azure','Posta'),
 },
 'mimetypes': {
  'text-x-generic': ('text','blue','Testo'),
  'text-x-script': ('script','slate','Codice'),
  'x-office-document': ('document','blue','Documento'),
  'x-office-spreadsheet': ('spreadsheet','green','Foglio di calcolo'),
  'x-office-presentation': ('presentation','coral','Presentazione'),
  'application-pdf': ('pdf','coral','PDF'),
  'image-x-generic': ('image','violet','Immagine'),
  'audio-x-generic': ('audio','coral','Audio'),
  'video-x-generic': ('video','indigo','Video'),
  'package-x-generic': ('archive','amber','Archivio'),
  'application-x-executable': ('executable','mint','Applicazione'),
  'application-x-sharedlib': ('script','slate','Libreria'),
  'inode-directory': ('folder','blue','Directory'),
 },
 'devices': {
  'computer': ('computer','blue','Computer'),
  'drive-harddisk': ('drive','slate','Disco'),
  'drive-removable-media': ('usb','blue','USB'),
  'drive-optical': ('disc','slate','Disco ottico'),
  'printer': ('printer','azure','Stampante'),
  'audio-headphones': ('headphones','violet','Cuffie'),
  'input-keyboard': ('keyboard','slate','Tastiera'),
  'input-mouse': ('mouse','slate','Mouse'),
  'video-display': ('monitor','blue','Monitor'),
  'camera-photo': ('camera','violet','Fotocamera'),
 },
 'status': {
  'network-wireless': ('wifi','blue','Wi-Fi'),
  'network-wireless-offline': ('wifi-off','slate','Wi-Fi spento'),
  'bluetooth-active': ('bluetooth','blue','Bluetooth attivo'),
  'audio-volume-high': ('volume','blue','Volume'),
  'audio-volume-muted': ('mute','slate','Muto'),
  'battery-full': ('battery','green','Batteria'),
  'battery-low': ('battery-low','coral','Batteria scarica'),
  'display-brightness': ('sun','amber','Luminosità'),
  'notifications': ('bell','blue','Notifiche'),
  'weather-clear-night': ('moon','indigo','Notte'),
  'emblem-favorite': ('star','amber','Preferito'),
  'emblem-important': ('important','coral','Importante'),
  'emblem-synchronizing': ('update','blue','Sincronizzazione'),
 },
 'actions': {
  'system-search': ('search','blue','Cerca'),
  'system-shutdown': ('power','coral','Arresta'),
  'system-reboot': ('update','blue','Riavvia'),
  'system-log-out': ('logout','indigo','Esci'),
  'system-lock-screen': ('lock','slate','Blocca'),
  'document-open': ('folder-open','blue','Apri'),
  'document-save': ('save','blue','Salva'),
  'edit-copy': ('copy','blue','Copia'),
  'edit-cut': ('cut','slate','Taglia'),
  'edit-paste': ('paste','mint','Incolla'),
  'list-add': ('plus','green','Aggiungi'),
  'user-trash-empty': ('trash','slate','Svuota cestino'),
 }
}
CONTEXTS = {'places':'Places','apps':'Applications','mimetypes':'MimeTypes','devices':'Devices','status':'Status','actions':'Actions'}

# Shared 2D vector symbols. They are authored, not borrowed from external icon themes.
def line(d, col='#fff', width=5, attrs=''):
    return f'<path d="{d}" fill="none" stroke="{col}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round" {attrs}/>'

def rect(x,y,w,h,r=4,fill='none',stroke='#fff',sw=4,extra=''):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" {extra}/>'

def circle(cx,cy,r,fill='none',stroke='#fff',sw=4):
    return f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>'

def motif_svg(m):
    # Motifs fit approximately the 24..72 glyph bounding box.
    C='#fff'
    if m in ('folder','folder-app'): return f'<path d="M25 40v-6q0-4 4-4h13l6 6h17q5 0 5 5v21q0 5-5 5H29q-4 0-4-5Z" fill="none" stroke="white" stroke-width="4" stroke-linejoin="round"/>'+line('M26 44h42',width=3)
    if m=='folder-open':return line('M23 43V34q0-4 4-4h16l5 6h18v10 M23 46h47l-6 20H29Z')
    if m in ('gear',):return circle(48,48,12,sw=5)+circle(48,48,4,fill='white',stroke='none')+''.join(line(f'M{48+20*math.cos(a):.1f} {48+20*math.sin(a):.1f}l{7*math.cos(a):.1f} {7*math.sin(a):.1f}',width=6) for a in [i*math.pi/4 for i in range(8)])
    if m in ('wifi','wifi-off'):
        x=line('M24 42Q48 20 72 42 M32 51Q48 36 64 51 M41 61Q48 54 55 61',width=5)+circle(48,70,3,fill='white',stroke='none')
        if m=='wifi-off':x+=line('M22 23 74 74',col='#ffcfdd',width=7)
        return x
    if m=='bluetooth': return line('M46 20V76L64 62 30 34 M46 20L64 34 30 62 M46 76L64 62 M46 20L64 34',width=5)
    if m in ('power',):return line('M48 20V49 M35 28C22 34 20 50 28 63C36 76 60 76 68 63C76 50 74 34 61 28',width=6)
    if m=='bell':return line('M31 59L36 53V41Q36 27 48 27Q60 27 60 41V53L65 59Z M43 65Q48 73 53 65',width=5)
    if m=='shield':return line('M48 20L68 27V46Q68 60 48 73Q28 60 28 46V27Z',width=5)+line('M39 46L46 53 58 40',width=4)
    if m in ('monitor','desktop'):
        return rect(22,27,52,35,5,fill='none',sw=4)+line('M48 63V72 M36 73H60',width=5)+line('M28 56H68',width=2)
    if m=='panel':return rect(23,27,50,41,6,sw=4)+line('M24 57H72 M34 64H44 M56 64H64',width=4)
    if m=='palette':return line('M48 22C28 22 20 35 22 49C24 66 39 75 48 72C55 70 51 62 54 59C57 55 72 61 75 51C80 33 65 22 48 22Z',width=4)+''.join(circle(x,y,3.7,fill='white',stroke='none') for x,y in [(36,35),(49,30),(61,38),(34,50)])
    if m=='globe':return circle(48,48,25,sw=4)+line('M23 48H73 M48 23Q29 45 48 73 M48 23Q67 45 48 73',width=3)
    if m=='play':return '<path d="M36 26L67 48 36 70Z" fill="none" stroke="white" stroke-width="5" stroke-linejoin="round"/>'
    if m=='docs':return rect(30,23,38,51,3,sw=4)+line('M37 37H59 M37 46H58 M37 55H55',width=3)+line('M23 30V67',width=4)
    if m=='game':return line('M35 42Q20 40 21 59Q23 77 38 64L58 64Q73 78 75 59Q76 40 61 42Z',width=4)+line('M33 49V59 M28 54H38',width=3)+circle(61,50,2.8,fill='white',stroke='none')+circle(68,56,2.8,fill='white',stroke='none')
    if m=='tools':return line('M28 66L60 34 M49 30Q57 19 68 26L60 34 64 42Q76 37 73 27 M26 60L36 70',width=5)
    if m=='terminal':return rect(23,28,50,42,6,fill='none',sw=4)+line('M31 41 41 50 31 58 M46 59H62',width=4)
    if m=='bag':return rect(26,34,44,37,6,fill='none',sw=4)+line('M38 38V31Q38 21 48 21Q58 21 58 31V38',width=4)+line('M38 54H58 M48 44V64',width=4)
    if m=='update':return line('M68 43Q65 24 46 25Q31 26 25 41 M25 29V43H39 M28 53Q31 72 50 71Q65 70 71 55 M71 68V54H57',width=4.5)
    if m=='calculator':return rect(29,21,38,54,5,sw=4)+rect(35,29,26,11,2,fill='white',stroke='none')+''.join(rect(x,y,6,6,1,fill='white',stroke='none') for x in (35,45,55) for y in (47,58))
    if m=='calendar':return rect(24,29,48,42,4,sw=4)+line('M24 42H72 M36 24V35 M60 24V35',width=4)+''.join(circle(x,y,2,fill='white',stroke='none') for x in (36,48,60) for y in (50,60))
    if m=='mail':return rect(22,32,52,37,4,sw=4)+line('M23 36 48 54 73 36 M24 65 40 51 M72 65 56 51',width=3)
    if m=='home':return line('M22 46L48 24 74 46 M29 43V72H67V43 M44 72V54H56V72',width=5)
    if m=='trash' or m=='trash-full':return line('M32 34H64L60 72H36Z M28 34H68 M40 30V25H56V30 M44 44V62 M54 44V62',width=4)+(circle(59,28,4,fill='#ffdc87',stroke='none') if m=='trash-full' else '')
    if m=='network':return circle(48,28,9,sw=3)+circle(27,64,9,sw=3)+circle(69,64,9,sw=3)+line('M43 36 32 55 M53 36 64 55 M36 64H60',width=3)
    if m=='server':return rect(24,27,48,19,4,sw=4)+rect(24,50,48,19,4,sw=4)+circle(33,37,2.4,fill='white',stroke='none')+circle(33,60,2.4,fill='white',stroke='none')+line('M44 37H65 M44 60H65',width=3)
    if m=='computer':return rect(20,27,56,38,4,sw=4)+line('M48 65V74 M37 75H59',width=4)
    if m=='drive':return line('M24 36L31 26H65L72 36V68H24Z M24 45H72 M34 56H50',width=4)+circle(61,57,3,fill='white',stroke='none')
    if m=='usb':return line('M36 24H60V66Q60 73 48 73Q36 73 36 66Z M41 24V31 M55 24V31 M42 46H54',width=4)
    if m=='disc':return circle(48,48,26,sw=4)+circle(48,48,8,sw=4)+line('M52 27Q65 32 69 43',width=3)
    if m=='printer':return rect(28,21,40,20,2,sw=4)+rect(22,38,52,26,5,sw=4)+rect(32,53,32,22,2,sw=4)+line('M38 62H58',width=3)
    if m=='headphones':return line('M24 53V46Q24 22 48 22Q72 22 72 46V60 M24 51H34V70H24Z M62 51H72V70H62Z',width=4)
    if m=='keyboard':return rect(18,36,60,32,5,sw=4)+''.join(rect(x,y,5,5,0.5,fill='white',stroke='none') for x in range(26,70,10) for y in (43,53))+line('M34 62H61',width=3)
    if m=='mouse':return rect(35,21,26,53,13,sw=4)+line('M48 22V45 M35 45H61',width=3)
    if m=='camera':return rect(22,34,52,35,6,sw=4)+line('M31 34 37 26H55L61 34',width=4)+circle(48,51,11,sw=4)
    if m=='volume' or m=='mute':return line('M22 43H35L49 31V65L35 53H22Z',width=4)+(line('M58 39Q68 48 58 58 M65 30Q83 48 65 67',width=4) if m=='volume' else line('M58 40 74 56 M74 40 58 56',width=5))
    if m in ('battery','battery-low'):
        return rect(22,32,49,32,5,sw=4)+rect(72,41,5,14,2,fill='white',stroke='none')+rect(29,39,34 if m=='battery' else 9,18,3,fill='white' if m=='battery' else '#ffd2e1',stroke='none')
    if m=='sun':return circle(48,48,11,sw=4)+''.join(line(f'M{48+20*math.cos(a):.1f} {48+20*math.sin(a):.1f}L{48+27*math.cos(a):.1f} {48+27*math.sin(a):.1f}',width=4) for a in [i*math.pi/4 for i in range(8)])
    if m=='moon':return '<path d="M59 24Q33 27 34 52Q36 73 61 70Q45 82 31 67Q17 51 29 35Q41 20 59 24Z" fill="white"/>'
    if m=='star':return '<path d="M48 22L56 40 76 42 61 56 65 76 48 66 31 76 35 56 20 42 40 40Z" fill="none" stroke="white" stroke-width="4" stroke-linejoin="round"/>'
    if m=='important':return circle(48,48,25,sw=4)+line('M48 32V52',width=6)+circle(48,63,3,fill='white',stroke='none')
    if m=='search':return circle(44,43,19,sw=5)+line('M58 59 73 74',width=6)
    if m=='logout':return line('M48 29H30V68H48 M44 48H75 M64 37L75 48 64 59',width=5)
    if m=='lock':return rect(27,43,42,30,5,sw=4)+line('M35 44V34Q35 21 48 21Q61 21 61 34V44',width=5)+circle(48,57,3,fill='white',stroke='none')
    if m=='save':return line('M25 22H65L72 29V74H25Z',width=4)+rect(36,24,24,16,2,sw=3)+rect(34,51,28,20,2,sw=3)
    if m=='copy':return rect(25,23,36,40,4,sw=4)+rect(37,34,36,40,4,sw=4)
    if m=='cut':return circle(32,65,8,sw=4)+circle(64,65,8,sw=4)+line('M33 29L63 58 M63 29L33 58',width=4)
    if m=='paste':return rect(29,30,39,43,5,sw=4)+rect(39,24,18,11,3,sw=3)+line('M37 49H59 M37 59H55',width=3)
    if m=='plus':return circle(48,48,25,sw=4)+line('M48 34V62 M34 48H62',width=5)
    if m=='image':return rect(22,26,52,45,5,sw=4)+circle(60,39,5,fill='white',stroke='none')+line('M26 65 40 50 50 57 59 47 71 60',width=4)
    if m=='audio':return line('M43 33L64 28V58 M43 40L64 35 M43 40V63',width=5)+circle(34,65,9,fill='white',stroke='none')+circle(55,61,9,fill='white',stroke='none')
    if m=='video':return rect(21,30,54,38,5,sw=4)+'<path d="M43 37L62 49 43 61Z" fill="white"/>'
    if m=='archive':return rect(25,30,47,42,3,sw=4)+line('M25 40H72 M43 31V53 M48 38V59 M43 58H55',width=4)
    if m=='executable':return rect(25,25,46,46,8,sw=4)+line('M33 49 44 59 63 38',width=5)
    if m=='script':return line('M36 29L24 48 36 67 M60 29L72 48 60 67 M54 25L42 72',width=5)
    if m=='pdf':return rect(28,24,41,50,3,sw=4)+line('M35 58Q47 35 50 44Q55 53 65 56Q49 59 35 58',width=3)
    if m=='text':return rect(28,23,40,51,3,sw=4)+line('M36 36H60 M36 46H60 M36 56H57 M36 65H51',width=3)
    if m=='document':return rect(27,23,42,51,3,sw=4)+line('M36 35H60 M36 44H60 M36 53H56 M36 62H61',width=3)
    if m=='spreadsheet':return rect(25,24,46,50,3,sw=4)+line('M25 40H71 M25 54H71 M43 40V74 M59 40V74',width=3)
    if m=='presentation':return rect(23,27,50,40,3,sw=4)+line('M48 67V76 M34 77H62',width=4)+'<path d="M48 36A13 13 0 1 0 61 49H48Z" fill="white" opacity=".95"/>'
    if m=='folder-documents':return motif_svg('docs')
    if m=='folder-download':return line('M48 27V60 M35 49L48 62 61 49 M34 68H62',width=5)
    if m=='folder-pictures':return motif_svg('image')
    if m=='folder-music':return motif_svg('audio')
    if m=='folder-videos':return motif_svg('play')
    if m=='folder-projects':return motif_svg('script')
    if m=='folder-publicshare':return motif_svg('network')
    if m=='folder-templates':return line('M29 28H67V67H29Z M29 47H67 M48 28V67',width=4)
    if m=='folder-favorites':return motif_svg('star')
    if m=='folder-recent':return circle(48,48,22,sw=4)+line('M48 33V49L60 55',width=4)
    raise ValueError(f'Unknown motif: {m}')

def svg_defs(palette):
    a,b,c=PALETTES[palette]
    return f'''<defs>
    <linearGradient id="face" x1="0" x2="0.78" y1="0" y2="1"><stop stop-color="{a}"/><stop offset="0.51" stop-color="{b}"/><stop offset="1" stop-color="{c}"/></linearGradient>
    <linearGradient id="shine" x1="0" x2="0" y1="0" y2="1"><stop stop-color="#fff" stop-opacity=".42"/><stop offset="1" stop-color="#fff" stop-opacity="0"/></linearGradient>
    <linearGradient id="glass" x1="0" x2="0" y1="0" y2="1"><stop stop-color="#fff" stop-opacity=".82"/><stop offset="1" stop-color="#e9f5ff" stop-opacity=".96"/></linearGradient>
    </defs>'''

def card_svg(motif,palette):
    return f'''<ellipse cx="48" cy="84" rx="31" ry="4" fill="#09264c" opacity=".13"/>
    <rect x="12" y="10" width="72" height="72" rx="20" fill="url(#face)" stroke="#174879" stroke-opacity=".22" stroke-width="1.5"/>
    <path d="M19 34V30Q19 17 32 17H65Q77 17 77 31V39Q48 48 19 34Z" fill="url(#shine)" opacity=".55"/>
    <path d="M19 58V65Q19 75 29 75H67" fill="none" stroke="#d7f5ff" stroke-opacity=".35" stroke-width="2" stroke-linecap="round"/>
    <g>{motif_svg(motif)}</g>'''

def folder_svg(motif,palette):
    # Large folder silhouette. Accent inset is specific to folder type.
    body='''<ellipse cx="49" cy="79" rx="36" ry="5" fill="#08274a" opacity=".12"/>
    <path d="M10 30Q10 23 17 23H40L47 30H76Q85 30 85 38V66Q85 74 77 74H18Q10 74 10 66Z" fill="#2765b3"/>
    <path d="M12 34Q12 28 19 28H41L47 34H77Q83 34 83 40V63H12Z" fill="url(#face)"/>
    <path d="M12 46Q12 42 18 42H80Q86 42 84 48L80 70Q79 76 72 76H19Q13 76 12 70Z" fill="url(#face)" stroke="#1b67b3" stroke-opacity=".42" stroke-width="1.5"/>
    <path d="M16 51Q16 46 23 46H77" fill="none" stroke="white" stroke-width="2" opacity=".44" stroke-linecap="round"/>
    <path d="M17 61Q34 51 58 54" fill="none" stroke="#d6f6ff" stroke-opacity=".19" stroke-width="3" stroke-linecap="round"/>'''
    if motif in ('folder','folder-open'):return body if motif=='folder' else body+'<path d="M70 38L85 46 78 73" stroke="#b0dfff" stroke-width="2" fill="none"/>'
    # Emblem in a glass plaque, scaled glyph (coordinates in 96-canvas are scaled down into inset)
    icon = motif.removeprefix('folder-')
    mapping={'documents':'docs','download':'folder-download','pictures':'image','music':'audio','videos':'play','projects':'script','publicshare':'network','templates':'folder-templates','favorites':'star','recent':'folder-recent'}
    glyph=motif_svg(mapping.get(icon,icon))
    return body+f'<circle cx="62" cy="57" r="16" fill="#082c57" opacity=".30"/><circle cx="62" cy="55" r="16" fill="#fff" opacity=".24"/><g transform="translate(41,34) scale(.44)">{glyph}</g>'

def file_svg(motif,palette):
    a,b,c=PALETTES[palette]
    # sheet body in lightly tinted glass, corner fold in accent.
    bg=f'''<ellipse cx="49" cy="84" rx="25" ry="4" fill="#284362" opacity=".1"/>
    <path d="M23 13H58L74 29V77Q74 82 69 82H28Q23 82 23 77Z" fill="url(#glass)" stroke="#8da7bb" stroke-width="1.6"/>
    <path d="M58 13V27Q58 30 62 30H74Z" fill="{a}" stroke="{c}" stroke-width="1.3"/>
    <path d="M28 18V76" stroke="#fff" stroke-width="2.5" opacity=".65"/>
    <path d="M32 74H65" stroke="{b}" stroke-width="2" stroke-opacity=".45"/>
    <rect x="30" y="43" width="38" height="30" rx="7" fill="{b}" opacity=".96"/>'''
    g=motif_svg(motif)
    return bg+f'<g transform="translate(13,22) scale(.72)">{g}</g>'

def device_svg(motif,palette):
    # Distinct device geometry on soft coloured halo rather than full square.
    return f'''<ellipse cx="48" cy="82" rx="30" ry="4" fill="#0c355c" opacity=".13"/>
    <circle cx="48" cy="46" r="36" fill="url(#face)" opacity=".18"/>
    <circle cx="48" cy="46" r="32" fill="url(#face)" stroke="#a9e9ff" stroke-opacity=".55" stroke-width="1.2"/>
    {motif_svg(motif)}'''

def svg_for(name,category,motif,palette):
    if category=='places' and motif.startswith('folder'):
        body=folder_svg(motif,palette)
    elif category=='mimetypes' and motif!='folder':
        body=file_svg(motif,palette)
    elif category in ('devices','places'):
        body=device_svg(motif,palette)
    else:
        body=card_svg(motif,palette)
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96" width="96" height="96" role="img" aria-label="{name}">{svg_defs(palette)}{body}</svg>'''

def tiny_svg(motif,palette):
    # Reduced silhouette for 16/24 px: avoid hairlines, small effects and text.
    a,b,c=PALETTES[palette]
    if motif.startswith('folder'):
        return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M2 6Q2 4 4 4H10L12 7H20Q22 7 22 9V19Q22 21 20 21H4Q2 21 2 19Z" fill="{c}"/><path d="M2 11H22L20 20H4Z" fill="{b}"/><path d="M4 12H20" stroke="{a}" stroke-width="1.5" stroke-linecap="round"/></svg>'''
    if motif in ('text','document','pdf','spreadsheet','presentation','script','archive','executable','image','audio','video'):
        return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5 2H14L19 7V22H5Z" fill="#eef7ff" stroke="{c}" stroke-width="1.5"/><path d="M14 2V7H19" fill="{a}"/><rect x="8" y="12" width="9" height="7" rx="1.5" fill="{b}"/><path d="M10 14H15M10 17H14" stroke="#fff" stroke-width="1.2"/></svg>'''
    return f'''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="1" y="1" width="22" height="22" rx="6" fill="{b}" stroke="{c}" stroke-width=".8"/><path d="M5 5H19" stroke="{a}" stroke-width="1.6" stroke-linecap="round" opacity=".55"/><g transform="translate(-1,-1) scale(.27)">{motif_svg(motif)}</g></svg>'''

def symbolic_svg(motif):
    # Monochrome symbolic icons are recoloured by GTK based on widget state.
    g=motif_svg(motif).replace('stroke="white"','stroke="#000"').replace('fill="white"','fill="#000"').replace('#fff','#000').replace('#ffcfdd','#000')
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 96"><g>{g}</g></svg>'

def theme_index():
    names=[]
    for sz in SIZES:
        for cat in ICONS:
            names.append(f'{sz}x{sz}/{cat}')
    for cat in ICONS: names.append(f'scalable/{cat}')
    names += ['scalable/status','scalable/actions'] if False else []
    content='[Icon Theme]\nName=Melania Aero\nName[it]=Melania Aero\nComment=Original luminous icons for Melania Desktop\nComment[it]=Icone luminose originali di Melania Desktop\nInherits=Adwaita,hicolor\nExample=folder\nDirectories='+','.join(names)+'\n\n'
    for sz in SIZES:
        for cat in ICONS:
            content+=f'[{sz}x{sz}/{cat}]\nSize={sz}\nType=Fixed\nContext={CONTEXTS[cat]}\n\n'
    for cat in ICONS:
        content+=f'[scalable/{cat}]\nSize=48\nType=Scalable\nMinSize=16\nMaxSize=256\nContext={CONTEXTS[cat]}\n\n'
    return content

def render(src, size):
    import cairosvg
    return cairosvg.svg2png(bytestring=src.encode('utf8'),output_width=size,output_height=size)

def build(svg_only=False):
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT/'index.theme').write_text(theme_index(),encoding='utf8')
    count=0
    for cat,items in ICONS.items():
        (OUT/'scalable'/cat).mkdir(parents=True,exist_ok=True)
        for name,(motif,palette,_label) in items.items():
            svg=svg_for(name,cat,motif,palette)
            ET.fromstring(svg)
            (OUT/'scalable'/cat/f'{name}.svg').write_text(svg,encoding='utf8')
            count+=1
            if not svg_only:
                mini=tiny_svg(motif,palette)
                ET.fromstring(mini)
                for sz in SIZES:
                    dest=OUT/f'{sz}x{sz}'/cat
                    dest.mkdir(parents=True,exist_ok=True)
                    (dest/f'{name}.png').write_bytes(render(mini if sz <=24 else svg,sz))
    # Preserve selected Luna crescent-and-dot mark from the user's supplied design.
    logo=(ROOT/'branding/icons/melania-symbol.svg').read_text(encoding='utf8')
    for alias in ('melania','start-here','distributor-logo','org.melaniadesktop.Preview'):
        (OUT/'scalable/apps'/f'{alias}.svg').write_text(logo,encoding='utf8')
        if not svg_only:
            for sz in SIZES:
                dest=OUT/f'{sz}x{sz}'/'apps';dest.mkdir(parents=True,exist_ok=True)
                (dest/f'{alias}.png').write_bytes(render(logo,sz))
        count+=1
    # GTK symbolic names: source-only works across multiple scales.
    for cat,items in {'status':['network-wireless','bluetooth-active','audio-volume-high','audio-volume-muted','battery-full','display-brightness','notifications'],
                      'actions':['system-search','system-shutdown','system-lock-screen','edit-copy','edit-paste','list-add']}.items():
        for name in items:
            motif=ICONS[cat][name][0]
            sym=symbolic_svg(motif)
            ET.fromstring(sym)
            (OUT/'scalable'/cat/f'{name}-symbolic.svg').write_text(sym,encoding='utf8')
            count+=1
    print('Generated',count,'SVG icons and theme',OUT)
    return count

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--svg-only',action='store_true');a=p.parse_args()
    build(a.svg_only)
