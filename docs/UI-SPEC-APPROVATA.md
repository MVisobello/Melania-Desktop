# Melania Desktop — Specifica UI approvata (v0.11)

## Riferimento
L'immagine fornita dall'utente con shell blu scura traslucida, menu a due colonne, file manager chiaro con preview e area rapida verticale sulla destra è il **riferimento principale**. Non usare come riferimento le immagini successive che l'utente ha rifiutato.

## Vincoli implementativi
- Shell blu profondo semitrasparente, bordi chiari sottili, angoli arrotondati, accento #2b8cff.
- Taskbar alta ~52 px; pulsante Melania, pin, finestre vere dal compositor, tray, orologio.
- Start a due colonne: ricerca e app a sinistra, accessi rapidi a destra. Non massimizzato.
- Area rapida in basso a destra: Wi-Fi, Bluetooth, audio, luminosità, risparmio energia, notifiche.
- Files chiaro: sidebar, toolbar, griglia con foto reali, inspector con preview immagini, DnD COPY conservativo.
- Impostazioni chiaro con sidebar e card.
- L'effetto blur dietro le finestre richiede supporto esplicito nel compositore e **non è implementato** da queste modifiche CSS.

## Fatto nel codice v0.11
- `melania-panel`: Start due colonne, scorciatoie, separazione visiva del pannello rapido, CSS unificato.
- `melania-files`: anteprima fotografia nell'inspector su selezione, stile file manager nuovo.
- `melania-settings`: CSS armonizzato.

## Da fare e testare
- Compilazione Rust GTK4 + Smithay, test grafici reali.
- Focus/popup, notifiche, calendario e slider live aggiornati.
- Preview thumbnail della taskbar tramite buffer compositor; blur vero.
- DnD interprocesso COPY/MOVE correttamente negoziati, test GTK/Qt/XWayland.
