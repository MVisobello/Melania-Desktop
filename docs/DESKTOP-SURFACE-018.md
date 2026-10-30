# Melania Desktop 0.18 — superficie desktop autonoma

## Obiettivo completato a livello di codice
È stato aggiunto un **client GTK 4 / Layer Shell** (`melania-desktop-surface`) distinto dalla vecchia demo Tkinter/X11 e da `melania-preview`. Fa parte del workspace Cargo.

- `Layer::Background` ancorato ai quattro lati: la superficie resta dietro le finestre XDG e sotto la barra Layer::Top.
- Wallpaper originale (PNG 1600×900) e selezione di wallpaper esterni tramite `MELANIA_WALLPAPER=/percorso/file.png`.
- Lettura della cartella XDG Desktop con supporto alla localizzazione tramite `xdg-user-dir DESKTOP`.
- Icone dei file/cartelle, apertura con `xdg-open`.
- Sorgenti e destinazione GDK `FileList`: **COPY soltanto**. Copia delegata a `melania-transfer`, senza richiesta di cancellazione dell'originale.
- Osservazione della cartella tramite `gio::FileMonitor` per aggiornare le icone.
- Integrazione nello script di avvio annidato, `integration/org.melaniadesktop.Desktop.desktop` e pipeline Linux.

## Come provare su Linux
Installare Rust e pacchetti di sviluppo GTK 4 + gtk4-layer-shell come indicato nel README, poi:

```bash
cargo check -p melania-desktop-surface
./scripts/run-melania-v018-nested.sh
```

Prova DnD sicura: creare una cartella temporanea nel proprio home, trascinare un piccolo file da Melania Files verso la Scrivania e verificare che l'originale non venga rimosso. **Non utilizzare dati importanti finché non avremo verificato l'esecuzione end-to-end.**

## Limiti non risolti
- **Non compilato qui:** mancano Cargo, GTK 4 di sviluppo e accesso ai repository. Il codice deve essere verificato sul sistema Linux di destinazione; non è ancora una funzione funzionante verificata.
- Il drag & drop con MOVE non è abilitato. Mancano selezione multipla, menu contestuale, ordinamento manuale per coordinate e scelta della posizione di rilascio.
- La gestione multimonitor e i wallpaper per output sono ancora da implementare.
- Aero blur autentico va implementato nel compositore, non in CSS.
- Il compositore Smithay precedente è anch'esso non compilato e resta sperimentale.

## Prossime milestone
1. `cargo check --workspace` e fix delle API (blocco principale).
2. Test annidato: avvio di compositor, desktop layer, taskbar e almeno una finestra GTK.
3. DnD effettivo fra Melania Files e desktop sotto Wayland con operazioni COPY/MOVE distinguibili.
4. Gestione popup, finestra taskbar, XWayland e portali desktop.
