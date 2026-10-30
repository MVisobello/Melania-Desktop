#!/usr/bin/env python3
"""Build Melania Fluent Light/Dark/Mixed themes with full freedesktop coverage.

By default read the Flat Remix *name-only inventories* already tracked in the
Melania repository. For standalone release reconstruction, --reference-tar
accepts a previously built Melania pack and reads only its member path names.
No third-party SVG or PNG is imported into the final theme.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import tarfile
import xml.etree.ElementTree as ET
from melania_fluent_artwork import artwork,CORE,REVERSE
from melania_fluent_reference import artwork as approved_artwork
from melania_fluent_settings_icons import artwork as settings_artwork
from melania_shell_symbolic_icons import artwork as shell_artwork, SIDEBAR, TRAY

ROOT=Path(__file__).resolve().parents[1]
COVERAGE=ROOT/'branding'/'coverage'
THEMES=('Melania-Fluent-Light','Melania-Fluent-Dark','Melania-Fluent-Light-on-Dark-Panel')
CONTEXTS=('actions','apps','animations','categories','devices','emblems','mimetypes','panel','places','status')
LABELS={'actions':'Actions','apps':'Applications','animations':'Animations','categories':'Categories',
        'devices':'Devices','emblems':'Emblems','mimetypes':'MimeTypes','panel':'Status','places':'Places','status':'Status'}

def paths_from_inventory()->list[str]:
    d=COVERAGE/'reference-dark-paths.txt';l=COVERAGE/'reference-light-paths.txt'
    if not d.is_file() or not l.is_file():
        raise FileNotFoundError(f'Missing name-only manifests in {COVERAGE}')
    return sorted(set(x.strip() for p in (d,l) for x in p.read_text(encoding='utf8').splitlines()
                      if x.strip() and not x.startswith('#')))

def paths_from_archive(file:Path)->list[str]:
    result=set()
    with tarfile.open(file,'r:*') as archive:
        for member in archive:
            p=member.name.removeprefix('./')
            prefix='Melania-Dark/'
            if p.startswith(prefix) and p.endswith('.svg') and '/_melania-assets/' not in p:
                result.add(p[len(prefix):])
    return sorted(result)

def validate_name(path:str):
    p=Path(path)
    if p.is_absolute() or '..' in p.parts or len(p.parts)<2 or p.parts[0] not in CONTEXTS or p.suffix.lower() != '.svg':
        raise ValueError(f'Invalid freedesktop icon path: {path}')

def size_from_path(path:str)->int:
    for part in Path(path).parts[:-1]:
        m=re.fullmatch(r'(\d+)(?:x\d+)?',part)
        if m:return int(m.group(1))
    return 96

def _symlink(path:Path,target:Path):
    path.parent.mkdir(parents=True,exist_ok=True)
    path.symlink_to(os.path.relpath(target,path.parent))

def _theme_index(title:str, paths:set[str])->str:
    dirs=sorted({str(Path(p).parent) for p in paths})
    result=['[Icon Theme]',f'Name={title}',f'Name[it]={title}',
            'Comment=Original Melania Fluent icons, compatible with the freedesktop icon-name inventory',
            'Inherits=Adwaita,hicolor','Example=folder','DesktopDefault=48','ToolbarDefault=24',
            'PanelDefault=24',f'Directories={",".join(dirs)}','']
    for directory in dirs:
        size=size_from_path(directory+'/index.svg')
        scalable=(size==96 or 'symbolic' in directory or '/' not in directory)
        result.extend([f'[{directory}]',f'Context={LABELS[directory.split("/")[0]]}',
                       f'Size={16 if scalable else size}',f'Type={"Scalable" if scalable else "Fixed"}'])
        if scalable:result.extend(['MinSize=16','MaxSize=512'])
        result.append('')
    return '\n'.join(result)

def build(dest:Path, paths:list[str], extra_sizes:bool=True)->dict:
    for path in paths:validate_name(path)
    unique=set(paths)
    # Additional real-sized paths of essential Melania metaphors, not generic
    # stand-ins. These are freedesktop compliant and remain independent SVGs.
    if extra_sizes:
        for _label, name, context in CORE:
            for size in (16,24,32,48,64,96,128,256):
                unique.add(f'{context}/{size}x{size}/{name}.svg')
    # Exact GTK application IDs must exist as real theme entries. Without
    # these names, Start and pinned Files/Settings show missing-image tiles.
    for app_name in ("melania", "org.melaniadesktop.Files", "org.melaniadesktop.Settings",
                     "preferences-system-notifications-symbolic"):
        unique.add(f"apps/scalable/{app_name}.svg")
    # Guarantee every exact symbolic name used by the sidebar and taskbar.
    for name in SIDEBAR:
        for size in ("scalable", "16x16", "24x24"):
            unique.add(f"places/{size}/{name}.svg")
    for name in TRAY:
        for size in ("scalable", "16x16", "24x24"):
            unique.add(f"status/{size}/{name}.svg")
    paths=sorted(unique)
    if dest.exists():
        raise FileExistsError(f'Output already exists; remove knowingly: {dest}')
    light,dark,mixed=[dest/x for x in THEMES]
    for base in (light,dark,mixed):base.mkdir(parents=True,exist_ok=True)
    known_assets={}
    stats={'paths_per_theme':len(paths),'reference_paths':len(unique),
           'curated_families':len(CORE),'asset_files':0,'light_shared':0}
    for variant,root in (('dark',dark),('light',light)):
        for path in paths:
            p=Path(path);category=p.parts[0];name=p.stem
            # Color illustrations identical across Light/Dark; only system
            # glyphs and statuses change stroke color, including symbolic forms.
            contrast=(category in ('actions','emblems','panel','status','animations')
                      or name.endswith('-symbolic') or 'symbolic' in p.parts)
            if variant=='light' and not contrast:
                _symlink(root/path,dark/path)
                stats['light_shared']+=1
                continue
            size=size_from_path(path)
            design=('dark' if variant=='dark' else 'light')
            if not contrast:design='color'
            key=(category,name,design,'compact' if size<=24 else 'full')
            if key not in known_assets:
                svg=(shell_artwork(name,category,'dark' if variant=='dark' else 'light',size)
                     or approved_artwork(name,category,'dark' if variant=='dark' else 'light',size)
                     or settings_artwork(name,category,'dark' if variant=='dark' else 'light',size)
                     or artwork(name,category,'dark' if variant=='dark' else 'light',size))
                ET.fromstring(svg)
                digest=hashlib.sha256(svg.encode('utf8')).hexdigest()
                asset=root/'_melania-assets'/f'{digest[:30]}.svg'
                if not asset.exists():
                    asset.parent.mkdir(exist_ok=True)
                    asset.write_text(svg,encoding='utf8')
                    stats['asset_files']+=1
                known_assets[key]=asset
            _symlink(root/path,known_assets[key])
    # Inter-theme relative symlinks remain valid only when installed together.
    for category in CONTEXTS:
        source=dark if category in ('panel','status','animations') else light
        _symlink(mixed/category,source/category)
    for directory,title in ((dark,'Melania Fluent Dark'),(light,'Melania Fluent Light'),
                            (mixed,'Melania Fluent Light on Dark Panel')):
        (directory/'index.theme').write_text(_theme_index(title,set(paths)),encoding='utf8')
    # Every required name resolves in all variants, including symlinked dirs.
    for folder in (dark,light,mixed):
        missing=[path for path in paths if not (folder/path).is_file()]
        if missing:raise AssertionError(f'{folder.name}: {len(missing)} broken names; first: {missing[:8]}')
    stats['verified']=True
    stats['themes']=[x.name for x in (light,dark,mixed)]
    (dest/'BUILD-REPORT.json').write_text(json.dumps(stats,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    return stats

if __name__=='__main__':
    ap=argparse.ArgumentParser()
    ap.add_argument('--reference-tar',type=Path,default=None)
    ap.add_argument('--output',type=Path,default=ROOT/'dist'/'melania-fluent-icons')
    ap.add_argument('--without-extra-sizes',action='store_true')
    args=ap.parse_args()
    result=build(args.output.resolve(),paths_from_archive(args.reference_tar) if args.reference_tar else paths_from_inventory(),
                 extra_sizes=not args.without_extra_sizes)
    print(json.dumps(result,indent=2,ensure_ascii=False))
