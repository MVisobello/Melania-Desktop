# Stage 84 — pixel reali del pannello GTK4 sul CRTC QEMU

## Obiettivo

Eseguire l'applicazione **reale** `melania-panel` (Rust + GTK4)
nel guest QEMU Ubuntu 24.04 e verificare l'effettiva presentazione del
suo primo buffer grafico sulla scheda VirtIO virtuale.

Non è una schermata sintetica, non è una sessione installabile,
non è un test di login, non è una prova di interazione con la taskbar.

## Percorso verificato

1. `MELANIA_NATIVE_REAL_GTK_PIXEL_KMS=YES_QEMU_VM` abilita un nuovo
   percorso, separato da Stage83G e dai vecchi test multi-finestra.
   Restano obbligatori DMI QEMU, TTY1, libseat e i flag opt-in KMS.
2. Il vero processo `melania-panel`, con `GDK_BACKEND=wayland` e
   `GSK_RENDERER=cairo`, crea la superficie
   `zwlr_layer_shell_v1` `melania-panel` inizialmente configurata a 1280×52.
   Nel tema GTK4 attuale, l'altezza naturale reale del widget è **60 px**:
   il primo buffer `wl_shm` acquisito e mostrato è **1280×60**.
   La differenza è stata riscontrata in una vera cattura del framebuffer
   QEMU (4 novembre 2026); il test non forza una grafica diversa.
3. Smithay copia il buffer `wl_shm` **realmente inviato dal processo GTK**,
   verifica formato, dimensioni e posizionamento, compone il pannello in
   basso allo schermo e consegna il frame al worker DRM/KMS Rust.
4. Il worker presenta il buffer su `/dev/dri/card0` del guest e controlla
   che il CRTC abbia effettivamente il nuovo FB_ID.
5. Durante la presentazione (prima del ripristino del CRTC) il monitor
   QMP cattura il framebuffer PPM **dalla VM**.
6. Un verificatore indipendente controlla risoluzione, integrità del PPM,
   sfondo fuori dal pannello, pixel modificati nell'area degli ultimi
   **60 pixel** (anche sul bordo superiore a y=740) e varietà grafica. Il risultato deve essere
   `MELANIA_QEMU_GTK4_PANEL_PIXELS_FROM_KMS=true`.
7. Il guest deve confermare che il buffer apparteneva proprio a
   `melania-panel`, il CRTC è stato ripristinato e
   `MELANIA_QEMU_REAL_GTK_PANEL_KMS_SCANOUT_PASS=true`.

La prova è **fail-closed**: assenza di superficie, buffer, ACK,
schermata QMP o ripristino del framebuffer causa un fallimento CI.

Il test deve essere dichiarato superato solo dopo l'esecuzione di
tutti e tre i workflow (Linux, Files/SFTP, QEMU) sul medesimo SHA.
Durante lo sviluppo questa PR resta separata da `main`.

## Confini deliberati

La prova usa una sola presentazione reale in VM, non un compositor
continuo pronto per l'uso. Restano da sviluppare input interattivi
del pannello, gestione di più frame reali GTK, menu Start attivo,
sessione standalone con display manager, hardware e QA bare-metal.

`MELANIA_NATIVE_SESSION_READY=false`. Nessun cambiamento grafico
ai temi Light, Dark o Aero.
