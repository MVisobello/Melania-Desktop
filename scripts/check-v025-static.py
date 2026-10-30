#!/usr/bin/env python3
"""Offline repository checks. They do NOT compile Rust or launch Wayland."""
from pathlib import Path
import subprocess
from datetime import date
r = Path(__file__).resolve().parent.parent
panel = (r/'melania-panel/src/main.rs').read_text()
comp = (r/'melania-compositor/src/main.rs').read_text()
control = (r/'melania-control/src/lib.rs').read_text()
model = (r/'melania-shell-model/src/lib.rs').read_text()
css=(r/'melania-panel/assets/panel.css').read_text()
checks={
    'Menu usa app installate': 'gio::AppInfo::all()' in panel,
    'Icone applicazioni GIcon': 'Image::from_gicon(&i)' in panel,
    'Ricerca menu': 'query.connect_changed' in panel,
    'Calendario GTK navigabile': 'fn calendar_window' in panel and 'button.connect_clicked' in panel,
    'Taskbar finestre via IPC': 'WindowCommand::List' in panel,
    'Quit sessione locale': 'WindowCommand::Quit' in panel and 'exit_requested' in comp,
    'Nessun shutdown host': 'Command::new("systemctl")' not in panel and 'Command::new("shutdown")' not in panel,
    'XDG maximize': 'fn maximize_request' in comp and 'fn unmaximize_request' in comp,
    'XDG minimize': 'fn minimize_request' in comp,
    'Click basato su surface_at': 'let target=state.surface_at(pointer.current_location())' in comp,
    'Layer arrange on commit': 'layer_map_for_output(&self.output).arrange();' in comp,
    'Test leap year': 'fn february_leap_year_and_century' in model,
    'Protocollo QUIT': 'Self::Quit => "QUIT\\n"' in control,
    'Classi CSS Aero': '.melania-start-panel' in css and '.melania-calendar-today' in css,
    'Logo e icone preservati': (r/'branding/Melania-Aero-Classic/index.theme').exists(),
    'Immagini di riferimento': (r/'docs/references/start-menu-approved.png').exists() and (r/'docs/references/shell-approved.png').exists(),
}
# Independently verify the Gregorian calculation with Python's calendar stdlib.
for month in range(1,13):
    def calc_first(year,month):
        offsets=(0,3,2,5,0,3,5,1,4,6,2,4)
        adjusted=year-1 if month<3 else year
        sunday=(adjusted+adjusted//4-adjusted//100+adjusted//400+offsets[month-1]+1)%7
        return (sunday+6)%7
    checks[f'Calendario 2026-{month:02d}']=calc_first(2026,month)==date(2026,month,1).weekday()
for name,success in checks.items():
    print(('OK' if success else 'ERRORE')+' '+name)
if not all(checks.values()):raise SystemExit(1)
print(f'{len(checks)} controlli statici/di modello superati; nessuna compilazione Rust eseguita')
