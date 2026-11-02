# Stage 35 — geometria, ordine e input per due finestre Wayland

Nel percorso nativo sperimentale (QEMU-only) lo scene graph Stage33 conserva
ora **coordinate per ogni finestra**, separate dall'ordine z. Le identità
dei buffer `wl_shm` rimangono stabili durante gli aggiornamenti. Il
compositore limita le finestre a due e verifica rigorosamente l'area
1280×800; `move_to` rifiuta coordinate fuori schermo, `raise` modifica
l'ordine senza alterare i buffer, e `hit_test` sceglie la finestra
visibile più in alto nel punto indicato.

Il backend Smithay `wayland_bridge.rs` usa la nuova geometria per
inoltrare gli eventi **libinput effettivi** al `wl_surface` corretto.
Nel percorso multi-finestra, il puntatore usa l'output reale QEMU a
1280×800; il clic premuto sposta il focus della tastiera sulla finestra
colpita e la porta in primo piano. Gli eventi puntatore sono gestiti
anche prima che una finestra abbia il focus, preservando il percorso
legacy 640×480 utilizzato dalla prova input precedente.

Una prova dedicata in GitHub Actions invia inoltre **clic veri tramite il
tablet USB virtuale QEMU**, durante il secondo e terzo frame KMS. Verifica
che il focus Wayland passi alla finestra B e poi alla finestra A, dopo
l'hit-test. La verifica fallisce se i messaggi di focus non arrivano: non
si tratta di simulare il risultato con dati Python, ma di osservare la
catena QMP → libinput → Smithay. Rimangono obbligatori gli screenshot QMP
dei frame reali e il ripristino CRTC.

I test Rust includono modifica della posizione, hit-testing di finestre
sovrapposte, cambio d'ordine, conservazione degli ID, rimozione e rifiuto
di coordinate fuori schermo. CI esegue i test Rust e la prova QEMU
completa introdotta negli Stage33/34.

**Non ancora disponibile:** gesture drag/move/resize interattive,
ricomposizione immediata di un frame quando cambia soltanto l'ordine
delle finestre, azioni dei pannelli layer-shell, gestione dinamica
di più di due finestre o output, input multi-seat, GBM/EGL e sessione
nativa di login. Non abilitare il backend fisico o il display manager.
