# Melania Desktop 0.7 — M5: Layer Shell, Aero Snap e drag & drop

## Obiettivo

Passare dal prototipo `melania-preview` a un pannello realmente ancorato allo schermo tramite **zwlr_layer_shell_v1**, usando un compositore Smithay nostro. Il vecchio `melania-preview` rimane una demo separata e non viene spacciato per un desktop installabile.

## Modifiche implementate a livello di sorgenti

1. `melania-compositor`: registra `WlrLayerShellState`, mappa le `LayerSurface` sull'output annidato, le include nell'hit-testing di mouse e tastiera, le disegna e invia frame callback. Gestisce inoltre l'area riservata richiesta dalla taskbar.
2. `melania-panel`: processo GTK4 indipendente; pannello `Layer::Top` ancorato sotto; menu Start `Layer::Overlay` in basso a sinistra; centro rapido `Layer::Overlay` in basso a destra. Ricerca le applicazioni registrate in GIO, usa il logo Melania, salva l'ordine delle icone fissate e interroga i servizi Linux per i controlli hardware.
3. `melania-shell-model`: logica Rust indipendente da GTK/Wayland per rilevare edge/corner Snap, calcolare geometrie senza perdere pixel su risoluzioni dispari, modellare focus/minimizzazione/ordine taskbar e distinguere un drop su app dal pinning di un launcher. Include **20 test unitari** da eseguire con Cargo.
4. `Aero Snap`: dopo un trascinamento delle finestre richiesto e autorizzato da un `xdg_toplevel`, il compositore calcola una nuova geometria per metà, quarto schermo o massimizzazione. Il file drag & drop resta invece di competenza di `wl_data_device`.

## Drag & drop: contratto di comportamento

- **File → Files:** continua a utilizzare `gtk::DragSource`, `gtk::DropTarget`, `gdk::FileList` e `melania-transfer`. Non sovrascrive automaticamente un file esistente.
- **Icona fissata → altra posizione nella taskbar:** usa `DragSource` e `DropTarget` GTK con un payload testuale ristretto a due launcher conosciuti. È un riordino, non una copia o una cancellazione; l'ordine viene scritto in `$XDG_CONFIG_HOME/melania/pinned-apps`.
- **File → applicazione:** è previsto nel modello, ma l'UI taskbar non lo accetta ancora come azione completa; non dichiararlo già funzionante.
- **Applicazione GTK → Qt o XWayland:** richiede ancora test di interoperabilità e XWayland completo.
- **Finestra → bordo:** gestita separatamente da `wl_data_device`, mediante eventi `xdg_toplevel.move` con seriale del pointer press verificato.

## Dipendenze Linux

- Cargo / Rust con supporto all'ecosistema GTK4.
- `libgtk-4-dev`, `libgtk4-layer-shell-dev`, `libwayland-dev`, `libxkbcommon-dev`, `libegl1-mesa-dev`, `libgles2-mesa-dev`.
- Per i toggle: `nmcli`, `bluetoothctl`, `wpctl`, `brightnessctl`, e permessi e servizi adeguati.

## Limiti fondamentali

- Il codice v0.7 **non è stato compilato** in questo ambiente: non sono presenti Cargo né l'SDK GTK4. Non sono stati eseguiti i test unitari Rust o i test Wayland grafici.
- Taskbar per ora con **launcher fissati**, non elenco sincronizzato delle finestre aperte: è necessario un protocollo interno sicuro compositor ↔ panel.
- `xdg_popup` non ha ancora gestione di rendering/focus completa; alcuni menu delle applicazioni potrebbero non comparire correttamente.
- Layer Shell di GTK non sostituisce il lavoro necessario per XWayland, screencast, portali Flatpak, clipboard manager, multiscreen e sessione DRM.
- `Melania Compositor` resta **annidato** dentro un altro desktop, non è ancora una sessione DRM/seat autonoma.
- Aero Glass e sfocatura reale non implementati; l'aspetto del pannello è realizzato con CSS GTK e trasparenze.
- Non eseguire test con file personali importanti o sostituire la sessione del login principale.

## Test che il PC Linux deve ancora superare

```bash
./scripts/check-melania-v07.sh
./scripts/run-melania-v07.sh
```

Verificare: il pannello appare in fondo; click menu Start e ricerca; click Centro rapido; due applicazioni `melania-files` aperte e spostate; resize; Snap lungo bordi e angoli; copia/spostamento controllato di file temporanei con DnD; focus corretto dei popup; assenza di crash.
