# Melania — Aero Glass / correzione Impostazioni

## Cosa viene visualizzato

- Titoli di **Impostazioni** e **Melania Files** con client-side decorations GTK4:
  gradiente azzurro, riflesso superiore, bordi luminosi e pulsanti finestra nativi.
- Finestra Impostazioni con sidebar e schede lievemente trasparenti.
- Switch GTK4 verticalmente centrati e con dimensioni minime definite sia nel
  layout Rust sia nei nodi CSS `switch > slider`. Evitano l'effetto di
  "levetta allungata" visto negli screenshot.
- La preferenza Aero Glass modifica la resa della finestra Impostazioni senza
  riavvio. Il tema scuro viene applicato subito alle Impostazioni.
- L'intensità memorizzata in `blur_strength` oggi varia la trasparenza del
  tema in quattro fasce, preparando la stessa preferenza per il renderer futuro.
- Default prima installazione: Aero attivo, variante chiara. Le impostazioni
  già presenti in `~/.config/melania/settings.conf` prevalgono sul default.

## Limite tecnico importante

Un gradiente semitrasparente GTK **non è il vero blur dello sfondo**.
I file GLSL nel progetto compositor sono preparatori: il compositor
Smithay dovrà acquisire il backdrop, eseguire i pass Gaussiani su GPU e
comporre le superfici traslucide con la giusta alpha. Questa patch non
finge di aver implementato tale render pass.

Il colore effettivamente visibile oltre le superfici trasparenti dipende
anche dal compositor della sessione host e dal supporto all'alpha.

## Verifica

- `cargo check -p melania-settings -p melania-files` su Linux con GTK4.
- `cargo test -p melania-config` per il default iniziale.
- Esecuzione grafica: verificare tutte le tre levette, cambio tema,
  pulsanti titlebar e ridimensionamento finestra a circa 870×600.
- È necessario un controllo visivo effettivo su un display Wayland/X11
  prima di dichiarare la fedeltà Aero definitiva.
