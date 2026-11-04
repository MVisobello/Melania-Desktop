# Stage 83G — avvio del vero pannello GTK4 nella VM QEMU

Questo step è il primo che **esegue davvero il binario Rust/GTK4
`melania-panel`** nel guest Ubuntu 24.04 isolato, anziché ricorrere
esclusivamente ai precedenti client Wayland di prova.

## Procedura obbligatoria

1. Il binario e la libreria reale `libgtk4-layer-shell.so.0` provengono
   dalla ISO in sola lettura già sottoposta alla prova del loader Stage83C.
2. Il bridge Smithay è avviato in una modalità separata e limitata al
   guest QEMU: `MELANIA_NATIVE_GTK_MULTISURFACE`,
   `MELANIA_NATIVE_SHELL_PROBE` e `MELANIA_NATIVE_REAL_GTK_SMOKE`,
   tutti con valore `YES_QEMU_VM`.
3. Il vero pannello GTK viene avviato con `GDK_BACKEND=wayland`,
   `GSK_RENDERER=cairo`, impostazioni GSettings volatili e
   `MELANIA_PROFILE_START=1`. Non viene chiamato un simulatore.
4. Il server deve rilevare una **superficie Top reale** con namespace
   `melania-panel` e configure `1280×52`.
5. L'applicazione deve emettere il proprio marker di profilo
   `MELANIA_PANEL_BOOT_MS`, registrato dopo `bar.present()`.
6. Solo dopo la presenza **di entrambi** i riscontri il guest emette
   `MELANIA_QEMU_REAL_GTK_PANEL_STARTUP_PASS=true`. Il controllo host
   rifiuta la CI se manca tale marker.

Il processo GTK si avvia dentro una sessione sperimentale a tempo
limitato. Dopo la prova, il server e il client vengono terminati
prima di proseguire con i precedenti test DRM/KMS, senza toccare
la GPU dell'host o sessioni utente.

## Interpretazione corretta del risultato

Questo test è **avvio del vero pannello GTK e handshake layer-shell**.
Non è ancora una prova di pixel GTK arrivati al framebuffer, né una
cattura autentica della taskbar, né un test dei suoi pulsanti o del
menu Start. Tali traguardi richiederanno una sessione che presenti
i buffer GTK sul KMS guest, una cattura QMP indipendente e controlli
sugli input.

Se GTK non parte, la sessione Wayland non viene creata o le
dipendenze sono incomplete, la nuova CI deve **fallire**, senza
rimpiazzare la prova con buffer sintetici.

Il pannello approvato non subisce alterazioni grafiche.
`NATIVE_SESSION_READY=false`.
