# Melania Aero — integrazione produttività M3

## Decisioni che NON cambiano
- Nome e marchio Melania: mezzaluna blu e punto; nessuna sostituzione.
- Desktop in stile Windows 7 Aero, barra in basso, menu Start classico,
  file manager a finestre tradizionali e trasparenze azzurre.
- Il codice Rust/GTK4/Wayland-Smithay è la base reale, il Tk UX-Lab non lo è.
- Ripristino, drag-and-drop, clipboard, trash, scorciatoie già funzionanti,
  impostazioni già salvate, workspace e snap rimangono invariati.
- GPU backdrop blur reale e animazioni del compositore NON vengono spacciati
  come completati da una modifica CSS GTK.

## Modifiche M3 — ispirazione ai quattro desktop

### Cinnamon/XFCE: taskbar non invasiva
- Preferenza persistente `group_windows` (predefinita `false` per non
  cambiare la barra che l'utente ha approvato).
- Modalità raggruppata opzionale per app-id: finestra singola mantiene
  l'attuale pulsante con attiva/minimizza/chiudi e drag-hover COPY.
- Due o più finestre della stessa app: un pulsante unico con contatore e
  popover accessibile da tastiera; selezione e chiusura per ID IPC reale.
- Su 5-8 workspace, il pannello mostra quattro pulsanti e i restanti in
  menu `Altri`, preservando spazio su desktop piccoli.
- La lista delle finestre è ospitata in uno scroller orizzontale GTK invece
  di occupare sempre tutta la larghezza, mantenendo i controlli tray.

### GNOME/COSMIC: ricerca unificata conservando Start classico
- Ricerca Start per le app .desktop già disponibili (GIO).
- Ricerca di sezioni reali di Melania Settings: `appearance`, `desktop`,
  `workspaces`, `network`, `accessibility`, `system`.
- Apertura cartelle XDG principali usando il file manager Melania; nessun
  indexing invasivo, nessun accesso arbitrario alle directory.
- Pressione Invio: avvio primo risultato visibile; risultati aggiuntivi
  nascosti finché la query è vuota.
- Settings espone navigazione tramite argument posizionale (esempio
  `melania-settings accessibility`) e riceve richieste anche mentre una
  finestra Impostazioni è già aperta, attraverso la normale single instance
  GTK `HANDLES_COMMAND_LINE`.

### Accessibilità e comportamento responsabile
- `reduce_motion` (default `false`): sopprime le animazioni GTK nei
  processi Melania (Settings e Panel), NON nelle altre app del sistema.
- `high_contrast` (default `false`): palette più contrastata delle
  superfici Melania Settings/pannelli, lasciando invariato Aero standard.
- Per i pannelli i valori sono letti all'avvio; la finestra Impostazioni
  può aggiornarsi immediatamente.
- `animations` rimane compatibile: un valore `true` non può forzare
  animazioni se `reduce_motion` è abilitato.

## Interfaccia e compatibilità

- `melania-config` conserva formato key=value, scrittura atomica e
  backward compatibility: proprietà mancanti usano i default.
- Nessun comando di modifica host, permessi root, DnD MOVE o copia pericolosa.
- File-manager, compositor ed icone originali non sono sostituiti.
- Nuove dipendenze runtime: nessuna.
- Il raggruppamento e le preferenze di accessibilità del pannello
  richiedono il riavvio del pannello, non del sistema Linux.

## Test d'accettazione

1. `cargo test -p melania-config`: roundtrip di tutte le preferenze.
2. `cargo test -p melania-panel --bin melania-panel`: grouping policy.
3. `cargo check -p melania-panel -p melania-settings -p melania-compositor`.
4. Start senza ricerca: app e accessi rapidi presenti; extra nascosti.
5. Start cerca `accessibilità` e Invio: apre la categoria corretta anche
   se Settings è già avviato.
6. Barra classica resta identica con `group_windows=false`.
7. Con `group_windows=true` due finestre same app_id raggruppate; menu
   attiva/minimizza/chiudi la singola finestra corretta.
8. Con workspace_count=8 la barra mostra quattro pulsanti + menu 5..8.
9. Drag di un file sul pulsante grouped: solo focus su finestra,
   mai trasferimento file eseguito dalla taskbar.
10. `high_contrast=true` aumenta contrasto pannello e Settings;
    `reduce_motion=true` disabilita animazioni GTK Melania.
11. Test a display 1024×768 e 870×600, anche tastiera-only; non
    dichiarare superato il collaudo visivo senza screenshot reale.

## Da completare — deliberatamente non dichiarato pronto
- Anteprime thumbnail delle finestre vere dal compositor.
- Tiling COSMIC, panoramica GNOME, workspace dinamici, plugin isolati.
- Funzioni di rete/audio/schermo via D-Bus complete e collaudate.
- Ricerca globale indicizzata del contenuto dei file.
- Gestione multi-monitor avanzata, recupero sessione, GPU backdrop blur.

Vedere `docs/MELANIA-BEST-OF-FOUR-ROADMAP.md` per le milestone rimanenti.
