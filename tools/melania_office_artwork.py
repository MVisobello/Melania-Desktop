#!/usr/bin/env python3
"""Original Melania Fluent Office icon, intentionally not Microsoft branding.

A trio of universal document types (text, table, presentation) represents
an entire office productivity suite. Pure SVG; no third-party logo or raster.
"""
from __future__ import annotations
import xml.etree.ElementTree as ET

def office_icon(size: int = 96) -> str:
    compact = size <= 24
    body = """<defs>
<linearGradient id="paper" x1="0" y1="0" x2="0.85" y2="1">
<stop stop-color="#FFFFFF"/><stop offset="1" stop-color="#DFEBF7"/></linearGradient>
<linearGradient id="blue" x1="0" y1="0" x2="0.7" y2="1">
<stop stop-color="#65C4F5"/><stop offset="1" stop-color="#2879CB"/></linearGradient>
<linearGradient id="green" x1="0" y1="0" x2="0.7" y2="1">
<stop stop-color="#74DDAB"/><stop offset="1" stop-color="#238A69"/></linearGradient>
<linearGradient id="violet" x1="0" y1="0" x2="0.7" y2="1">
<stop stop-color="#E4B6FF"/><stop offset="1" stop-color="#9860C9"/></linearGradient>
</defs>
<ellipse cx="48" cy="85" rx="31" ry="5" fill="#102F52" opacity=".12"/>
<rect x="19" y="12" width="48" height="64" rx="8" fill="url(#paper)"
 stroke="#95B1CB" stroke-width="1.5"/>
<path d="M25 24h21M25 31h30M25 38h26" fill="none"
 stroke="#4C91CE" stroke-width="3.5" stroke-linecap="round"/>
<rect x="14" y="41" width="39" height="36" rx="8" fill="url(#green)"
 stroke="#217C62" stroke-width="1.5"/>
<rect x="21" y="49" width="25" height="20" rx="2" fill="none"
 stroke="#F5FFFD" stroke-width="2.7"/>
<path d="M21 56h25M21 63h25M29 49v20M38 49v20" fill="none"
 stroke="#F5FFFD" stroke-width="2"/>
<rect x="50" y="34" width="34" height="43" rx="8" fill="url(#violet)"
 stroke="#7952AF" stroke-width="1.5"/>
<rect x="57" y="43" width="20" height="23" rx="2.5" fill="#F7EEFF" opacity=".95"/>
<path d="M60 59l6-9 4 5 4-6v11H60" fill="#AD7ADB"/>
<path d="M22 15h31" stroke="#FFFFFF" stroke-width="2.5" opacity=".8"/>
"""
    if compact:
        body = """<defs>
<linearGradient id="b" x2="0.8" y2="1"><stop stop-color="#6EC6F4"/><stop offset="1" stop-color="#2B7ED0"/></linearGradient>
</defs>
<rect x="21" y="13" width="52" height="66" rx="8" fill="#F4F9FF" stroke="#6F98BF" stroke-width="3"/>
<path d="M31 25h33M31 34h27" fill="none" stroke="url(#b)" stroke-width="6" stroke-linecap="round"/>
<rect x="11" y="45" width="40" height="34" rx="7" fill="#29A474" stroke="#167D5C" stroke-width="2"/>
<path d="M18 56h26M18 65h26M30 52v20" stroke="#FFFFFF" stroke-width="4"/>
<rect x="52" y="42" width="33" height="37" rx="7" fill="#A879D2" stroke="#8050AE" stroke-width="2"/>
<path d="m60 66 7-13 9 13Z" fill="#FFFFFF"/>
"""
    svg = '<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96">' + body + '</svg>'
    ET.fromstring(svg)
    return svg
