# Native Stage 82A — layout del pannello layer-shell

Il bridge Wayland nativo oggi gestisce il protocollo wlr-layer-shell
configure/ACK, ma non presenta ancora i pixel dei veri buffer layer-shell.

Questo contributo aggiunge una politica di geometria in Rust, separata
dall'accesso alla GPU: quattro livelli (background, bottom, top e overlay),
ancoraggio agli angoli e ai lati, dimensioni variabili, margini e zone
esclusive riservate alle finestre normali. La risoluzione del framebuffer
non viene cambiata quando una barra riserva spazio.

I test verificano taskbar in alto e in basso, pannelli laterali, overlay,
wallpaper a pieno schermo, ordine dei livelli di rendering, dimensioni
illecite e prevenzione dei valori negativi nell'area di lavoro.

La fase 82B dovra collegare il ciclo di vita WlrLayerSurface e i
buffer wl_shm realmente acquisiti alla composizione, quindi verificare
pixel QEMU, focus/input e ACK del framebuffer DRM/KMS. Non si dichiara
l'interfaccia nativa completa. NATIVE_SESSION_READY=false.
