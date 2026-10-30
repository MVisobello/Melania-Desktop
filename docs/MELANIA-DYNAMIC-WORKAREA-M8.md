# M8 — Work area reale di Melania (Smithay)

## Motivazione
L'ultima schermata reale rivelava che l'app Files doveva indovinare un
margine per la taskbar. L'app non dovrebbe conoscere altezza del pannello:
**il compositore è la fonte autorevole** per l'area libera.

## Nuovo comportamento
- Smithay `layer_map_for_output(...).non_exclusive_zone()` indica i rettangoli
  liberi dopo aver applicato l'esclusione delle superfici wlr-layer-shell.
- Alla prima geometria non nulla del client, la posizione della nuova
  finestra è contenuta in quell'area; non viene alterata dopo il primo
  posizionamento (per non spostare finestre mentre l'utente le usa).
- Per un client inizialmente più grande dell'area viene inviata una
  configurazione xdg_toplevel più piccola. I client non cooperativi
  potrebbero ignorarla; non viene manipolato il contenuto dei buffer.
- A ogni modifica effettiva dell'area (pannello mostrato/nascosto, cambio
  exclusive zone, output ridimensionato), le finestre **massimizzate**
  vengono riposizionate e ricevono una nuova richiesta di dimensione.
  Le finestre normali già spostate dall'utente non vengono forzate.
- Le stesse logiche di focus, tastiera, drag/drop file, workspace e
  stack layering restano invariate.

## Verifica
- Test unitari puri `melania-shell-model::workarea` su pannelli inferiori,
  superiori, output con offset, client sovradimensionati e overflow int.
- `cargo check -p melania-compositor` e `cargo test -p melania-shell-model`.
- Smoke test in compositor annidato senza cambiare il desktop host.
- Test manuale aggiuntivo necessario: cambiare l'esclusione di un layer
  mentre una finestra è massimizzata e verificare i buffer client.

## Limiti non risolti in questa milestone
- L'attuale fallback dimensionale di Files verrà eliminato solo dopo
  aver collaudato l'intera gestione output su più display.
- Le miniature taskbar richiedono cattura autorizzata sul renderer:
  NON si fotografano altre finestre via API GTK.
- Il vero blur Aero Glass richiede pass GPU offscreen prima della
  composizione delle superfici: i framment shader esistenti non sono
  ancora collegati al frame renderer e non vanno dichiarati funzionanti.
