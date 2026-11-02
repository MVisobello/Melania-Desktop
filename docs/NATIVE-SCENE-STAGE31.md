# Melania Native — Stage 31: compositor di scena CPU a livelli

## Modifica effettiva

Il framebuffer **della prova QEMU** viene ora prodotto dal modulo Rust
`melania-native/src/scene.rs`, chiamato da `native_client_frame.rs`.
La superficie `wl_shm` reale del client resta di proprietà del client e viene
copiata in memoria privata mentre la mappatura Smithay è valida. Il nuovo
compositore applica sfondo, bordo e finestra in livelli ordinati e invia i
pixel risultanti alla **stessa catena Rust DRM/KMS** già verificata allo Stage 30.

Il modulo supporta più superfici posizionate, ordine stabile entro ciascun
livello, livelli Background/Bottom/Normal/Top/Overlay, clipping (anche
coordinate negative), stride non compatto e alpha BGRA non premoltiplicato.
Convalida i buffer anche per superfici completamente fuori schermo e limita
a 64 MiB il framebuffer in uscita. I test Rust verificano i pixel effettivi
e i percorsi con dati malformati.

## Verifica

Il workflow `.github/workflows/qemu-melania-native.yml` esegue
`cargo test -p melania-native --all-targets`, il nuovo test strutturale
Stage 31 e la prova completa QEMU. I precedenti test di campionamento
dei pixel e il ripristino verificato del CRTC restano obbligatori.
Una PR non è da considerarsi verificata finché i workflow non sono verdi.

## Limiti

L'integrazione VM utilizza ancora **una sola superficie Wayland reale** e
il trasferimento socket/ACK sincrono fra server Smithay e KMS. Questa
modifica introduce la base del scene graph, **non** realizza già composizione
simultanea di più finestre reali o pannelli layer-shell. Restano necessari:
gestione delle superfici con ruolo, posizionamento dinamico, danni,
render scheduling non bloccante, GBM/EGL, ripristino da VT/hotplug e test
con molte finestre native.

Non si abilita la sessione di login, non si usa la GPU fisica dell'host e
non si modifica la sessione annidata già funzionante.
