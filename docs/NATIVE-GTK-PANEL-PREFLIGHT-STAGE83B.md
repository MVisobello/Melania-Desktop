# Stage 83B — preflight del vero pannello GTK4

Questa fase aggiunge un controllo **sul binario reale** `melania-panel`
alla CI nativa QEMU. Il runner Ubuntu 24.04 installa le dipendenze di
sviluppo GTK4 e gtk4-layer-shell, compila il binario Rust originale
(`cargo build -p melania-panel --bin melania-panel`) e verifica con
`ldd` che tutte le librerie dinamiche siano risolte.

Il rapporto `gtk-panel-runtime-libs.txt` è incluso negli artefatti
della CI. Il controllo fallisce se il binario manca, se non si compila
o se una libreria dinamica non viene trovata.

**Limiti espliciti:** il binario GTK non viene ancora trasportato né
eseguito nel guest QEMU; la CI esistente continua a collaudare la
superficie Wayland e i pixel KMS tramite il client Rust/libwayland
indipendente. Nessun nuovo screenshot viene presentato come cattura
della taskbar GTK. La compilazione sul runner non garantisce da sola
la compatibilità runtime nel guest.

## Successivo: Stage 83C

Preparare un payload runtime GTK completo e verificato nel guest
isolato; avviare la vera taskbar `melania-panel` sul socket Wayland
del compositore nativo, con una superficie layer-shell configurata e
buffer realmente importati. Solo dopo il collaudo QEMU con screenshot
QMP e test input si potrà affermare che la taskbar GTK gira nativamente.

`NATIVE_SESSION_READY=false`.
