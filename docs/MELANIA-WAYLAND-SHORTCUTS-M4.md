# Melania Wayland — scorciatoie M4 (infrastruttura compositor)

## Obiettivo
Aggiungere produttività da tastiera preservando interamente il desktop Aero
originale e la struttura Rust/GTK4/Smithay già realizzata.

## Scorciatoie in questa milestone

| Combinazione | Funzione | Implementazione |
|---|---|---|
| Alt + Tab | Finestra visibile successiva sul workspace attivo | Gestione Smithay input + `Activate(id)` |
| Alt + Maiusc + Tab | Finestra visibile precedente | Stessa funzione con ordine inverso |
| Ctrl + Alt + Freccia sinistra | Workspace precedente, con ritorno all'ultimo | `Workspaces` via compositor |
| Ctrl + Alt + Freccia destra | Workspace successivo, con ritorno al primo | `Workspaces` via compositor |
| Super + E | Apri Melania Files in sessione Melania | Spawn programma fisso senza shell |

Solo il compositor *nested* riceve le combinazioni quando la relativa
finestra host è attiva. Nessun comando deve cambiare i binding del desktop
ospitante. Il launcher Super+E imposta `WAYLAND_DISPLAY` al socket Melania,
così non apre accidentalmente la finestra nel desktop host.

## Controlli di correttezza
- `cycle_window_id` è puro e testato: non attraversa finestre minimizzate
  o di workspace diversi (il compositor prepara l'elenco filtrato).
- L'attivazione usa il controllo esistente e non inventa un altro focus manager.
- Lo switch workspace usa il codice che già preserva la posizione e gli stati
  delle finestre, non sposta i file DnD.
- Le release dei tasti dedicati vengono intercettate finché il modificatore
  è ancora premuto. Testare i casi di rilascio in ordine inverso nel nested
  e la perdita di focus nella finestra host.
- No anteprima grafica finta: richiede prova reale su compositor/Wayland.

## Da implementare successivamente
- Editor dei keybinding con verifica conflitti e persistenza.
- Shortcut overview e Snap Assist quando la UI e il compositor li supportano.
- Gestione robusta dei key-up persi o inviati dopo un cambio di focus sul
  backend nested; non dichiarare tutti i casi di edge risolti.
- Interazione con input method/IME, lettori schermo e tastiere non italiane.

## Accettazione
- `cargo test -p melania-shell-model` (navigation policy).
- `cargo build -p melania-compositor -p melania-settings -p melania-panel`.
- Smoke nested Wayland (non provare come sessione principale).
- Manuale: Alt+Tab con 2 finestre aperte e una minimizzata; Ctrl+Alt
  su 1, 4 e 8 workspace; Super+E dentro nested; verificare nessun invio
  di shortcut al desktop host e nessuna regressione file DnD.
