# Stage 83A — geometria della taskbar GTK nel percorso Wayland nativo QEMU

## Obiettivo

Allineare il collaudo del client `zwlr_layer_shell_v1` alla geometria
**effettiva** della barra di `melania-panel/src/main.rs`:

- posizione: Bottom, Left, Right;
- altezza richiesta: **52 pixel logici**;
- larghezza richiesta: **0** (stiramento su entrambi gli anchor), non una
  larghezza fissa di 1280 inviata dal client;
- zona esclusiva: 52 pixel, corrispondente alla taskbar GTK.

Il server di collaudo QEMU configura la superficie come `1280×52`.
Il client reale libwayland conferma il configure e invia buffer `wl_shm`
`1280×52` posseduti dal client, attraverso la scena Smithay e il renderer
Rust DRM/KMS. La cattura **esterna** QMP deve trovare il bordo superiore
esattamente a `y=748` su un display da `1280×800`, sfondo sopra il bordo,
pixel fino all'ultima riga e finestra xdg conservata.

Il workflow fallisce se non arrivano i callback del client, il rilascio dei
buffer, la conferma CRTC framebuffer, l'immagine QMP o le asserzioni dei pixel.

## Limiti e passaggio successivo

**Questa non è ancora la taskbar GTK eseguita nel guest QEMU.** Il client
di collaudo è un piccolo programma Rust/libwayland, con la stessa richiesta
geometrica dell'app GTK. Non prova icone, menu Start, tray, click GTK,
scalatura HiDPI o login. La compatibilità della superficie è necessaria ma
non sufficiente per avviare l'intera shell.

Stage 83B: trasportare ed eseguire **il vero binario `melania-panel`**
nel guest QEMU con runtime GTK4/gtk4-layer-shell, confermare una surface
52px e verificare pixel e input senza usare screenshot sintetici.

Nessun cambiamento allo styling GTK. `NATIVE_SESSION_READY=false`.
