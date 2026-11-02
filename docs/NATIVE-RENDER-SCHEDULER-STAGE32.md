# Melania Native — Stage 32: ticket di presentazione e coalescing dei frame

## Implementazione

`melania-native/src/render_scheduler.rs` introduce uno scheduler di stato
per **un output**, limitato a una sola presentazione in corso e a una richiesta
di ridisegno successiva: gli eventi di danno ripetuti non generano una coda
illimitata. Un ticket `(epoch, sequence)` identifica un tentativo di disegno.

- Il completamento del frame è valido soltanto dopo la conferma del renderer
  e solo se il ticket coincide con quello ancora attivo.
- Il fallimento conserva una richiesta di ridisegno, **senza emettere un
  `wl_surface.frame`** e senza trattare il buffer come presentato.
- Pausa/ripresa della seat e invalidazione annullano i ticket precedenti;
  la conferma tardiva di una generazione superata viene rifiutata.
- I contatori sono saturanti e la struttura non conserva i dati dei pixel;
  non c'è alcuna coda di immagini non limitata.

L'attuale percorso VM (`wayland_bridge.rs`) usa ora lo scheduler per le
tre vere superfici di frame successive di un client xdg-shell già presenti
nello Stage 30. La prova obbligatoria QEMU richiede un ACK per ogni ticket
dopo la verifica DRM/KMS, **prima** della callback Wayland e del rilascio
del buffer del client. Test Rust coprono coalescing, doppio ACK, errore,
hotplug e stato della sessione.

## Limiti

Il passaggio corrente è un **meccanismo di sicurezza e scheduling**, non un
render loop completamente asincrono. Il socket con Rust KMS rimane bloccante
con timeout, e QEMU usa un solo client per il percorso pixel. Il sistema
di composizione multilivello dello Stage 31 è una base CPU testata, non
ancora una scena dinamica di più finestre Wayland reali.

Passi successivi: trasporto asincrono delle immagini con dimensioni limitate,
eventi KMS associati a ticket, buffering del lavoro senza bloccare Smithay,
scene graph dei ruoli xdg/layer-shell, output enter/leave, scale/damage,
libseat e recupero da crash. Nessun login nativo, nessun uso di DRM host.
