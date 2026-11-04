# Stage 84C — tema Fluent reale nel pannello GTK4 su QEMU

La precedente cattura della taskbar nativa era autentica, ma il log di
`melania-panel` riportava `MELANIA_PANEL_ICON_THEME_SELECTED=Adwaita`:
il pacchetto Fluent non faceva parte dell'immagine ISO del guest.

Questo intervento non ridisegna le icone. Usa esattamente il generatore
di SVG Fluent già incluso nel repository e crea soltanto una selezione
di icone effettivamente utili al collaudo della taskbar. Il pacchetto
completo resta affidato a `tools/build_melania_fluent_themes.py`.

## Catena di verifica

1. In GitHub Actions lo script `tools/build_qemu_fluent_subset.py`
   genera un sottoinsieme di tre temi:
   `Melania-Fluent-Light`, `Melania-Fluent-Dark` e
   `Melania-Fluent-Light-on-Dark-Panel`, con `index.theme`.
2. Il codice di build verifica, nelle tre varianti, il logo Melania,
   le icone reali di Melania Files/Settings, le cartelle e i simboli
   Wi-Fi della taskbar. I collegamenti relativi non devono uscire dal
   pacchetto. I test leggono e validano effettivamente gli SVG.
3. Il payload ISO QEMU include la directory `icons/` in sola lettura,
   con collegamenti relativi e risorse originali.
4. Entrambi gli avvii di `melania-panel` nel guest impostano
   `MELANIA_BUNDLE_DATA_DIR=/mnt/melania-native`, senza cambiare
   alcun tema sul sistema host.
5. Il guest rifiuta un pacchetto incompleto e controlla che entrambi
   i log GTK4 contengano
   `MELANIA_PANEL_ICON_THEME_SELECTED=Melania-Fluent-Light`.
   Un fallback a `Adwaita` rende il collaudo QEMU **fallito**.

Questa è una verifica dell'integrazione del tema e della corretta
risoluzione delle icone, non una dimostrazione che tutte le migliaia di
icone del pacchetto completo siano già rifinite.

Anche con il tema corretto, il sistema rimane un collaudo a singolo
frame in VM: `MELANIA_NATIVE_SESSION_READY=false`.
