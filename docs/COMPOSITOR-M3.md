# Melania v0.5 — prima implementazione del compositore annidato

## Che cosa è stato effettivamente aggiunto

`melania-compositor` è un **nuovo binario Rust** che usa Smithay 0.7.0 e il backend `winit`. Non si limita a scaricare o avviare l'esempio esterno `smallvil`. Il codice:

1. crea un socket Wayland separato (`melania-N`) e accetta client;
2. espone `wl_compositor`, `wl_shm`, `xdg_shell`, `wl_seat`, `wl_output` e `wl_data_device`;
3. crea e disegna finestre `xdg-toplevel` in una finestra annidata sul desktop host;
4. registra un `Space<Window>` per la geometria delle finestre e il test dell'area sotto il puntatore;
5. inoltra input base da mouse e tastiera e aggiorna il focus client;
6. rende disponibile la negoziazione clipboard / DnD di Smithay `DataDeviceState`.

È una **prima implementazione non ancora compilata e non testata**. La struttura è basata su API documentate di Smithay 0.7.0, ma potrebbero essere necessarie correzioni in fase di `cargo check` e test d'interazione. Non usarla come sessione principale.

## Come avviarlo

Su un Linux con Rust, Cargo, GTK4 SDK, OpenGL e Wayland:

```bash
./scripts/check-melania-v05.sh
./scripts/run-melania-nested.sh
```

Il secondo script compila e prova ad aprire Melania Files nella finestra del nuovo compositore.

Per avviare una **seconda finestra** dentro la medesima sessione, leggere il socket stampato nel terminale (`melania-N`) e usare, in un altro terminale:

```bash
WAYLAND_DISPLAY=melania-N GDK_BACKEND=wayland ./target/debug/melania-files
```

Non sostituire `melania-N` con un valore casuale: deve corrispondere a quello stampato nel tuo sistema. Trascinare un file da una finestra all'altra permette di valutare `wl_data_device` + `GtkDragSource`/`GtkDropTarget` dopo aver compilato.

## Drag & drop: punto fondamentale

**Protocollo:** `wl_data_device` registrato in Smithay; aggiornamento del focus di tastiera e puntatore; superficie bersaglio determinata tramite hit-testing reale delle finestre e delle rispettive regioni di input. Non consegniamo copie di byte del drag al compositore: i dati passano attraverso la normale negoziazione tra sorgente e destinazione Wayland.

**Applicazione:** Melania Files utilizza `GtkDragSource` + `GtkDropTarget` e la libreria `melania-transfer` per copiare file nella cartella scelta; eventuale `MOVE` richiede conferma dal protocollo prima della cancellazione presso la sorgente.

**Difese:** `melania-dnd::local_uris` aggiunge una lettura stretta dei dati `text/uri-list`: rifiuta payload troppo grandi, URL remoti, percent encoding invalido e NUL; questo parser è predisposto per le future integrazioni e non è ancora collegato direttamente al backend GTK.

### Cosa non è ancora disponibile

- **Nessuna garanzia E2E sul DnD.** Occorre eseguire un test fra due applicazioni GTK e poi tra toolkit differenti; si tratta per ora di implementazione del percorso principale, non di compatibilità certificata.
- Niente trascinamento/ridimensionamento **delle finestre** via `xdg_toplevel.move/resize` (servono grab serializzati e validati).
- Niente gestione avanzata di popup, cursore server, clipboards estese, input-method, DRM/GBM, XWayland, layer-shell, global shortcuts, multimonitor, sessione di accesso e Aero blur.
- `melania-dnd` modella la policy ma non intercetta ancora gli eventi provenienti dal `DataDeviceState` di Smithay.
- La finestra `melania-preview` è ancora una preview GTK, non è stata trasformata in un pannello Wayland `layer-shell`.

## Quale sarà la prossima milestone

M4: stabilizzare la compilazione/test GUI, spostamento interattivo delle finestre, gestione popup e avvio della shell Melania nella sessione Wayland; poi supporto layer-shell e XWayland.

## Fonti tecniche

- https://smithay.github.io/smithay/smithay/index.html
- https://github.com/Smithay/smithay/tree/v0.7.0/smallvil
- https://docs.rs/wayland-server/0.31.9/wayland_server/struct.ListeningSocket.html
