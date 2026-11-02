# Melania Native — Stage 30: frame callback dopo la presentazione KMS

## Incremento verificabile

Lo Stage 29 aveva collegato tre aggiornamenti reali di un client Wayland
a un processo DRM/KMS Rust concorrente, con conferma privata del framebuffer.
Ora il client richiede anche l'evento standard Wayland **`wl_surface.frame`**
prima di ogni commit.

Nel guest QEMU, Smithay inoltra la superficie a Rust DRM/KMS, attende la
conferma della presentazione del framebuffer e **solo dopo** invia
`wl_callback.done` al client. Il client deve ricevere tre callback
distinti e non può procedere al frame successivo senza la conferma del
precedente. I callback sono elaborati attraversando anche l'albero delle
eventuali subsurface, anziché limitarsi a un singolo oggetto Wayland.

La prova di accettazione richiede contemporaneamente:
- 3 commit reali `wl_shm` di un client `xdg_toplevel`;
- 3 eventi `wl_callback.done` dopo gli ACK KMS;
- 3 screenshot QMP autentici, con differenti pixel del client;
- ripristino del CRTC originale;
- Linux build e test GVfs/SFTP senza regressioni.

## Limiti e prossima fase

Questa è una prova finita in una VM isolata; **non è ancora un unico
event loop Smithay/DRM**. Il processo KMS Rust resta separato, e gli
ACK bloccano deliberatamente i commit. Mancano scheduler asincrono,
composizione simultanea di più finestre e pannelli, corretta gestione
`wl_surface.enter/leave`, scale, damage, popup, EGL/GBM e pausa/ripresa
della sessione con libseat.

Il passo successivo sarà progettare un render scheduler non bloccante
e un piccolo scene graph con finestre e layer-shell nello stesso output.
Nessun login nativo o accesso DRM sul computer principale viene attivato.
La sessione annidata funzionante non viene modificata.
