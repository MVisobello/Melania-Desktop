#!/usr/bin/env python3
"""Static checks, not a substitute for cargo check."""
from pathlib import Path
root=Path(__file__).resolve().parents[1]
checks={
 'Start menu a due colonne':('melania-panel/src/main.rs','melania-start-columns'),
 'Ricerca programmi':('melania-panel/src/main.rs','query.connect_changed'),
 'Accessi rapidi Start':('melania-panel/src/main.rs','ACCESSI RAPIDI'),
 'Pannello Layer Shell':('melania-panel/src/main.rs','init_layer_shell'),
 'Area rapida':('melania-panel/src/main.rs','Area rapida'),
 'File manager DnD':('melania-files/src/main.rs','gdk::DragAction::COPY'),
 'Inspector con immagine':('melania-files/src/main.rs','preview_image.set_filename'),
 'CSS stile menu':('melania-panel/assets/panel.css','.melania-start-shortcuts'),
 'CSS stile file manager':('melania-files/assets/style.css','.files-inspector-image'),
 'Specifica approvata':('docs/UI-SPEC-APPROVATA.md','riferimento principale'),
}
for name,(filename,needle) in checks.items():
    assert needle in (root/filename).read_text(), name
    print(f'OK — {name}')
print('Controlli statici completati: nessuna compilazione Rust eseguita.')
