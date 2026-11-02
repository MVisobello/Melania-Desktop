# Stage 33 — due vere finestre Wayland, composizione retained e QEMU

Il percorso sperimentale Rust/Smithay dispone ora di due superfici indipendenti
`xdg_toplevel` create da un vero client Wayland. Entrambe inviano propri buffer
`wl_shm` XRGB8888 (360×224) al compositore. Ogni buffer viene validato e
**copiato in memoria privata durante la callback**. Il nuovo
`MultiWindowScene` conserva al massimo due finestre; l'aggiornamento di una
finestra non elimina i pixel dell'altra. I pixel vengono uniti tramite
`scene::compose` Stage31 e mostrati con la catena esistente Rust DRM/KMS
in una VM QEMU isolata, con ticket di frame Stage32.

## Evidenza richiesta dalla CI

La prova Stage33 esegue un **secondo ciclo** di tre presentazioni KMS nella
stessa VM, senza sostituire né ridurre lo storico collaudo Stage32:
- Frame 1: finestra A reale, pixel arancioni/turchesi.
- Frame 2: finestra A invariata, finestra B reale blu/gialla.
- Frame 3: finestra A aggiornata rosa/verde, finestra B invariata.
- Screenshot QMP PPM originali per tutti e tre i frame, pixel verificati nelle
  due regioni e nello sfondo, frame tutti diversi, CRTC originale ripristinato.
- Tre `wl_surface.frame` callback, ciascuna solo dopo ACK Rust KMS;
  rifiuto di ticket obsoleti e nessuna conferma senza presentazione.
- Compilazione/test Rust e test Python su tutte le invarianti di sicurezza.

## Limitazioni ancora aperte

Questa è la **prima vera prova a due finestre visibili su GPU virtuale**, non
una sessione desktop nativa installabile. Le posizioni e dimensioni sono
ancora fisse e il test usa solo XRGB8888; non supporta gestione dinamica
xdg/layer-shell, decorazioni interattive, resize, damage parziale, input
per finestra o riordino. Il trasferimento KMS rimane sincrono e usa ACK
bloccanti; la fase successiva è un canale di trasporto bounded e
**non bloccante** nel loop Wayland, senza anticipare ACK o rilasci di buffer.

`NATIVE_SESSION_READY=false`, gate QEMU-only, `/dev/tty1`, protezioni GPU
fisica e sessione annidata rimangono invariati.
