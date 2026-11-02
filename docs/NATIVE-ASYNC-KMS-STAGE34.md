# Stage 34 — trasporto KMS non bloccante nel ciclo eventi Wayland

Lo Stage33 acquisisce e conserva due veri `xdg_toplevel` e li compone nella
stessa scena. Lo Stage34 separa l'invio di quei frame a Rust drm-rs dal loop
di Smithay/calloop. Un worker dedicato scrive i pixel sul socket Unix e
attende la conferma KMS, mentre il loop Wayland continua a gestire eventi
e invoca soltanto `try_send`/`try_recv` non bloccanti.

## Invarianti

- Una sola presentazione in corso (scheduler Stage32) e canali
  `sync_channel(1)`: nessun accumulo arbitrario di framebuffer.
- `FrameTicket` per generazione e sequenza: un vecchio ACK non può
  autorizzare callback e buffer release dopo un cambio seat/epoch.
- La callback `wl_surface.frame` e `wl_buffer.release` avvengono
  **solo nel thread Wayland** dopo aver ricevuto il vero ACK del worker
  e dopo la convalida del ticket. Gli errori causano arresto fail-closed.
- La vecchia prova singola rimane sincrona per controllare regressioni;
  la seconda prova QEMU a due finestre esegue il nuovo worker. I campioni
  QMP devono mostrare i pixel reali A→A+B→A'+B su tre frame distinti
  e l'originale CRTC deve essere ripristinato.
- Unit test Rust con coppie reali di UnixStream verificano che il chiamante
  può procedere mentre un ACK non è arrivato, e che ACK sbagliati non
  producono successo.

## Limiti

È un trasporto e un protocollo di riconciliazione asincroni in una VM, **non**
un compositore Wayland completo. Non dispone ancora di un event loop unificato
Smithay/DRM, gestione dinamica delle finestre, throttling al refresh,
GPU GBM/EGL, input/focus per finestra, output multi-monitor o recupero
completo della sessione libseat/VT. Non abilitare display-manager login
o accesso KMS sull'host. La versione annidata resta invariata.
