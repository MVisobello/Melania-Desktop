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

## Stato del vetro GTK4 — 7 novembre 2026

**Implementato in questa fase:** le superfici GTK4 Aero di taskbar, Start,
pannelli e finestre Files/Impostazioni disegnano pixel con canale alpha reale,
riflessi e gradiente azzurro. Il compositore Wayland sottostante miscela questi
pixel con quelli delle superfici sottostanti. In una sessione annidata, cioè,
l'aspetto translucido deve essere osservabile senza prendere possesso della GPU
o della sessione di login dell'host.

- `Aspetto > Aero`: conserva la palette blu e applica le trasparenze.
- `Aspetto > Intensità vetro (trasparenza)`: l'opacità di Aero diminuisce
  con l'intensità. La barra rispetta anche il limite impostato con
  `panel_opacity`; il processo GTK Panel ricarica il file di configurazione
  già esistente senza dover riavviare la sessione.
- `Light`, `Dark` e `high_contrast`: mantengono superfici opache, senza
  ereditare accidentalmente il vetro Aero.
- Le impostazioni correnti modificano la finestra Impostazioni dal vivo.
  Le altre app GTK possono richiedere la riapertura per applicare il nuovo
  tema, a eccezione del pannello che osserva le preferenze.

**Non implementato:** il *backdrop blur* vero e proprio. Per un Aero Glass
come quello di Windows 7 occorre che il compositore catturi il contenuto
**dietro** la superficie traslucida, lo sfumi (due passaggi di blur orizzontale
e verticale su GPU), poi ricomponga la superficie cliente rispettando
alpha, damage, ordine delle finestre, input e sessione. Gli shader
`melania-compositor/shaders/aero-*.frag` esistono, ma non sono ancora
collegati al renderer. Le superfici qui non affermano di produrre sfocatura.
Nessuna impostazione di trasparenza certifica l'avvio di Melania Native.

### Verifiche

```bash
python3 -m unittest discover -s tests -p 'test_aero_glass_live.py' -v
cargo test -p melania-panel --bin melania-panel
bash scripts/melania-session --nested
```

Nel collaudo visivo, controllare uno sfondo ad alto contrasto dietro Start,
taskbar, titlebar di Files e Impostazioni, alternando Light/Dark/Aero e
alto contrasto. La sola unit test prova la logica; è necessaria una cattura
reale su Wayland per confermare i pixel traslucidi.

