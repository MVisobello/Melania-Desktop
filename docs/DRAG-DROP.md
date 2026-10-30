# Drag & Drop — Specifica tecnica Melania 0.4

**Stato: implementazione locale prototipale nel file manager e nella finestra preview; nessun compositore Melania funzionante.**

Il drag & drop è una funzionalità *core* e blocca il rilascio di Melania 1.0 finché non soddisfa tutti i criteri.

## Architettura

```text
Gestione puntatore / tastiera (Wayland seat)
                ↓
Melania Compositor (futuro: Smithay data-device + seat focus)
       ↙                          ↘
  client GTK/Qt/X11          client GTK/Qt/X11
  DragSource                 DropTarget / GDK
  FileList/URI list          FileList/URI list
       \__________________________/
               →  Action COPY/MOVE/LINK
               →  Handshake finale
               →  Pulizia origine SOLO per MOVE confermato
```

- **Protocollo Wayland**: `wl_data_device_manager`, data source, data offer e data-device focus per seat. Non basta trascinare una finestra: serve routare i dati di DnD e l'icona di trascinamento, incluso touch. In una futura implementazione Smithay useremo il supporto data-device, focus corretto e policy su target e azioni.
- **GTK4**, come client: `GtkDragSource` prepara `GdkFileList`; `GtkDropTarget` accetta file locali. Le conversioni di formato usano GDK, non formati privati Melania.
- **XWayland/XDND**: ancora da realizzare (incluso ponte fra clients X11 e Wayland); obiettivo di compatibilità con le app di Cinnamon.
- **Autenticazione**: nessuna UI client può richiedere al compositore l'accesso a trascinamenti/clipboard altrui; DnD consentito soltanto da gesto utente con input serial valido.
- **Sicurezza file**: no sovrascrittura; rifiuto della copia di cartella all'interno di se stessa; symlink copiate senza seguirne il target; MOVE del protocollo finalizzato dalla sorgente soltanto dopo il successo della copia; origine rifiutata se sostituita/modificata durante il drag.

## Implementato nel codice v0.4 (NON eseguito in questa sessione)

| Scenario | Stato codice | Note |
|---|---|---|
| Elemento locale Melania Files → cartella visibile | ✅ Implementazione GTK+Rust | DragSource, DropTarget, COPY/MOVE, da collaudare |
| File da altra app GTK/Qt compatibile → Files | ✅ Implementazione GTK | Richiede formati di drag compatibili; test E2E pendente |
| Elemento → vuoto di Melania Files | ✅ Implementazione GTK | Destinazione = cartella corrente |
| Elemento → cartella nella sidebar | ✅ Implementazione GTK | Folders locali disponibili |
| File → sfondo demo Melania Preview | ✅ Implementazione GTK | Copia su cartella XDG desktop e aggiorna le icone visibili |
| File icona desktop → altra app | ✅ Implementazione GTK | DragSource standard; prova E2E pendente |
| Evidenziazione drop target | ✅ CSS / GTK | `drop-ready` |
| Copia ricorsiva dir / symlink | ✅ `melania-transfer` | No follow symlink |
| Annullamento collisione | ✅ `melania-transfer` | Rifiuta overwrite |
| State machine seat/MIME/actions | ✅ `melania-dnd` | Non collegato a Smithay |
| Drag file → app nella taskbar reale | ⏳ Non implementato | Serve shell/compositor + activation |
| Drag icon da un monitor all'altro | ⏳ Non implementato | Compositor multimonitor |
| Drop verso file manager GVfs / SMB / cloud | ⏳ Non implementato | Richiede `gio::File` + autenticazione |
| Drop su cestino (undo) | ⏳ Non implementato | `Gio::File::trash` / Trash spec |
| Drag & drop da X11 | ⏳ Non implementato | XWayland e XDND bridge |
| Large files + progress + cancellazione | ⏳ Non implementato | Usare operazioni async e GtkDropTargetAsync |
| Drag di finestre / snap / virtual desktop | ⏳ Non implementato | Compositor proprietario |

## Scelta sul comportamento COPIA/SPOSTA

La destinazione **copia** i dati ricevuti. Quando la sessione DnD negozia MOVE e il target ha confermato la copia, è l'applicazione sorgente a cancellare l'originale; questo rispetta la semantica di `GtkDragSource::drag-end(delete_data)` e previene cancellazioni premature. Melania Files registra l'identità del file all'inizio del drag e rifiuta la cancellazione se risulta modificato o sostituito prima del completamento.

Al momento le copie sono **sincrone** nell'interfaccia GTK (possono bloccarla con file molto grandi): non sono idonee all'utilizzo quotidiano. Prima della milestone 1.0 serviranno copia async, progress reporting, abort, ripresa/undo e gestione errori. Per i file remoti non facciamo conversioni implicite in percorsi locali.

## Test sicuro su PC Linux

```bash
./scripts/preflight.sh
./scripts/test-dragdrop.sh
```

`test-dragdrop.sh` genera file e cartelle **temporanei**, fa partire Melania Files con quella cartella, permette di provare il drag e li elimina alla chiusura del programma. Non tocca le cartelle personali. Richiede Rust/Cargo, dipendenze GTK4 e una sessione grafica.

### Criteri di accettazione prerelease

1. Trascinare due file da Melania Files a una seconda app e viceversa, con copia e spostamento verificati sul disco.
2. Drag fra monitor, fra workspace e da una finestra XWayland verso una GTK Wayland.
3. Drop su desktop, cartella, file manager, app nella taskbar e cestino.
4. Nessuna perdita di dati con collisioni di nomi, directory annidate, file di oltre 10 GB, filesystem pieni, permessi rifiutati, sorgenti modificate, dischi rimossi durante la copia.
5. Notifica dello stato dell'operazione con progresso, pulsante Annulla e ripristino per le operazioni supportate.
6. Test automatizzati in `wlroots`/Smithay test environment più smoke GUI reale, tastiera e touch.

## Riferimenti ufficiali
- GTK drag & drop: https://docs.gtk.org/gtk4/drag-and-drop.html
- gtk-rs DragSource: https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/struct.DragSource.html
- gtk-rs DropTarget: https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/struct.DropTarget.html
- Smithay data_device: https://docs.rs/smithay/latest/smithay/wayland/selection/data_device/
