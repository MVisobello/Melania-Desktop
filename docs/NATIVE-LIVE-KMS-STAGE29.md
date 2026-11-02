# Stage 29 — flusso di aggiornamento Wayland → Rust KMS in QEMU

## Scopo

La fase precedente (Stage 28) acquisiva **due** buffer \`wl_shm\` reali,
terminava il server Wayland e solo dopo avviava la prova KMS Rust.

Stage 29 cambia il ciclo di vita:

1. \`melania-native --experimental-wayland-seat\` resta in esecuzione e
   pubblica \`/run/melania-native-qemu/frame-stream.sock\` (solo nella VM).
2. \`qemu-rust-kms-stream\` si connette e apre la scheda DRM virtuale.
3. Un vero client \`xdg_toplevel\` effettua **tre commit consecutivi**
   \`wl_shm\` con pixel propri, diversi a ogni commit.
4. Smithay copia i buffer e trasferisce i frame integrali su un
   socket Unix con lunghezza esplicita e verificata.
5. Il processo KMS Rust alloca due framebuffer e visualizza il primo
   usando \`set_crtc\`, quindi aggiorna gli altri tramite \`page_flip\`.
6. Il client riceve \`wl_buffer.release\` soltanto dopo
   l'ACK del processo KMS per ogni commit. L'ultimo ACK segue
   l'effettivo ripristino dello stato originale del CRTC.
7. QMP acquisisce **tre screenshot distinti** durante il funzionamento
   contemporaneo del server Wayland e del processo Rust KMS e ne
   controlla i colori dei pixel del client, dello sfondo e del bordo.
8. Il guest verifica il ripristino e il normale ritorno della console.

Non è consentito eseguire questa prova fuori dalla VM QEMU: servono
insieme identificazione DMI, \`/dev/tty1\`, consenso esplicito,
GPU virtuale e ambiente grafico ospitante assente.

## Limitazioni

Questo è un **test finito di tre frame** con presentazione KMS
Rust e server Smithay attivi contemporaneamente. Non è un desktop
continuo autonomo: il processo KMS è ancora separato da Smithay,
l'ACK serializza intenzionalmente ogni commit e non sono pronti
frame callback generali, scene graph multi-finestra, rendering GBM/EGL
client completo, layering del pannello, gestione robusta del pause/resume
libseat, hotplug, più monitor e rollback dopo crash o SIGKILL.

Restano \`MELANIA_NATIVE_CONTINUOUS_COMPOSITOR_READY=false\`,
\`MELANIA_NATIVE_SESSION_READY=false\` e i blocchi di produzione
del renderer nativo. Il vecchio test Stage 28 rimane attivo come
regressione indipendente, così come la sessione annidata esistente.

## Prossimo passaggio

Consolidare un unico ciclo eventi Smithay/DRM, gestire frame callback
e scene graph con buffers/scale/strati, trasferire il display al renderer
nativo tramite libseat e validare pause/resume e recovery sul guest.
Solo allora provare Panel, Files e Settings in una sessione nativa QEMU.
