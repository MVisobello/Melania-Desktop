#!/usr/bin/env python3
"""Melania Fluent's generic productivity suite icon (not Microsoft Office).

A blue folder/fan of three working documents represents writing,
spreadsheets and presentations. This changes only the Office *category*
visual family: real application brands continue using their own icons.
"""
from __future__ import annotations

OFFICE_CATEGORY_NAMES = frozenset({
    "applications-office",
    "office",
    "org.melaniadesktop.Office",
    "melania-office",
})

def office_artwork(name: str, category: str, variant: str, size: int) -> str | None:
    if category not in ("apps", "categories") or name not in OFFICE_CATEGORY_NAMES:
        return None
    # Distinctive productivity documents, not a Microsoft logo or lettermark.
    detail = size > 24
    lines = ('<path d="M23 42h16M23 48h14M23 54h13" stroke="#e1f1ff" '
             'stroke-width="2.2" stroke-linecap="round"/>'
             '<path d="M48 47h21M48 54h21M55 39v23M63 39v23" stroke="#e7fff5" '
             'stroke-width="1.8"/>'
             '<path d="m68 69 8-11 7 11Z" fill="#fff1e4"/>' if detail else
             '<path d="M24 50h15M48 52h19M67 68h15" stroke="#fff" '
             'stroke-width="4" stroke-linecap="round"/>')
    return f'''<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96" viewBox="0 0 96 96">
<defs>
  <linearGradient id="blue" x2="0" y2="1">
    <stop stop-color="#7bcbff"/><stop offset=".45" stop-color="#2587e8"/>
    <stop offset="1" stop-color="#1757b7"/>
  </linearGradient>
  <linearGradient id="green" x2="0" y2="1">
    <stop stop-color="#89dfbd"/><stop offset="1" stop-color="#178a68"/>
  </linearGradient>
  <linearGradient id="orange" x2="0" y2="1">
    <stop stop-color="#ffd19c"/><stop offset="1" stop-color="#ef823e"/>
  </linearGradient>
</defs>
<ellipse cx="48" cy="83" rx="37" ry="5" fill="#22456c" opacity=".13"/>
<!-- Three overlapping, clipped-corner office sheets -->
<path d="M14 21q0-4 4-4h26l9 9v46q0 4-4 4H18q-4 0-4-4Z"
  fill="url(#blue)" stroke="#2165ad" stroke-width="1.5"/>
<path d="M44 17v10h9" fill="#c8ecff" opacity=".9"/>
<path d="M37 28q0-4 4-4h26l8 9v43q0 4-4 4H41q-4 0-4-4Z"
  fill="url(#green)" stroke="#167556" stroke-width="1.5"/>
<path d="M67 24v9h8" fill="#d3fff0" opacity=".9"/>
<path d="M58 43q0-4 4-4h18q4 0 4 4v32q0 4-4 4H62q-4 0-4-4Z"
  fill="url(#orange)" stroke="#c36a2a" stroke-width="1.5"/>
<path d="M18 24h23" stroke="white" stroke-width="3" opacity=".55"/>
<path d="M41 31h24" stroke="white" stroke-width="2.5" opacity=".4"/>
{lines}
</svg>'''
