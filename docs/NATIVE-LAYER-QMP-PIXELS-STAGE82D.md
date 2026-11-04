# Stage 82D — pannello layer-shell reale su QEMU

Il client Wayland di prova apre contemporaneamente una finestra
`xdg_toplevel` e una superficie `zwlr_layer_surface_v1` di livello
Top, ancorata agli ultimi 40 pixel in basso, con zona esclusiva 40.

Le due superfici alternano 12 buffer reali `wl_shm`, copiati dal
compositore nativo e inviati alla GPU VirtIO mediante il componente
Rust DRM/KMS. Per ogni buffer e obbligatoria la conferma del framebuffer
CRTC seguita dal callback e dal rilascio del client.

Al frame 12 una cattura separata QMP legge i pixel effettivi della GPU
virtuale mentre il framebuffer e ancora visibile. Si controllano
colore e presenza del pannello fino all'ultimo pixel in basso,
assenza dei suoi pixel appena sopra, immagine mantenuta della finestra
normale e sfondo dove le superfici non esistono.

I test Python con byte sintetici servono solo a verificare che il
validatore rifiuti immagini errate, non rappresentano screenshot reali.

Nessun cambiamento grafico al desktop GTK approvato. Non equivale
all'avvio del vero pannello GTK in una sessione nativa: rimangono da
verificare il toolkit, input e focus del pannello, la gestione seat/VT,
polkit, D-Bus, lockscreen e il login.
`NATIVE_SESSION_READY=false`.
