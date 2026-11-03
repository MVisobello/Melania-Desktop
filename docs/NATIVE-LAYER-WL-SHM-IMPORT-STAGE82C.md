# Stage 82C — primo collegamento live layer-shell e wl_shm

In un nuovo flag QEMU *esplicito* `MELANIA_NATIVE_LAYER_PIXELS=YES_QEMU_VM`,
il bridge Smithay conserva la superficie wlr-layer-shell, suggerisce la
dimensione 1280x40, riceve il `wl_surface.commit` del client e copia
i pixel reali `wl_shm` con il validatore `owned_shm`.

Il compositor legge ancoraggi, margini, livello e zona esclusiva
direttamente dallo stato del ruolo di Smithay. Il modello 82A calcola
la geometria; la Stage 82B fa disegnare i pixel nell'ordine corretto.
La dimensione del buffer deve coincidere esattamente con la geometria
convalidata; altrimenti i pixel vengono rifiutati. Il buffer accettato
entra nel retained scene e nel pipeline continuo KMS con la stessa
contabilizzazione di callback/buffer release delle finestre normali.

La rimozione di un layer cancella i suoi pixel dalla scena e crea danno.
Le altre modalità e i workflow QEMU già certificati non vengono
modificati: il comportamento è isolato dietro un flag non utilizzato
nelle prove precedenti.

Ancora da dimostrare in Stage 82D: un *vero client layer-shell* che
invia i buffer, la lettura QMP del framebuffer, la corrispondenza
delle coordinate, il clic sulla taskbar e la compatibilità con il
pannello GTK reale. La Stage 82C non equivale a queste prove;
non abilita una sessione Linux produttiva. NATIVE_SESSION_READY=false.
