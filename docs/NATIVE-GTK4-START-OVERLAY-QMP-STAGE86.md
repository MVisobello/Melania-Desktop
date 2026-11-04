# Stage 86 — menu Start GTK4 reale sopra la taskbar, verificato in QEMU

La Stage 85 dimostra due framebuffer successivi prodotti dal vero pannello
GTK4. Questa nuova fase, **separata e sperimentale**, aggiunge il terzo
frame della stessa sessione QEMU: viene aperta la superficie reale
`melania-start` del progetto, senza screenshot simulati.

## Percorso del rendering

1. La macchina virtuale avvia il vero `melania-panel` collegato al
   compositore Smithay ed esegue i due frame Stage85 già previsti:
   taskbar iniziale e riverniciatura GTK4 a 4,5 secondi.
2. Solo con `MELANIA_QEMU_GTK_SHOW_START_AFTER_PAINT=YES_DISPOSABLE_QEMU`
   la *stessa applicazione GTK4* presenta la finestra Start originale a
   8,5 secondi. La versione normale non apre Start automaticamente.
3. Il compositor accetta due ruoli layer-shell distinti:
   `melania-panel` e `melania-start`. Mantiene la loro identità e
   importa una copia dei buffer `wl_shm` realmente committati.
4. Il ricevitore Rust invia il terzo frame al display DRM/KMS virtuale
   e verifica il nuovo FB_ID prima di stampare
   `MELANIA_RUST_REAL_GTK_START_FRAME_VISIBLE=3`.
5. Un monitor QMP host, attivo già prima del boot, cattura la schermata
   1280×800 mentre il frame è ancora presente sul CRTC. Confronta il
   secondo screenshot con il terzo: almeno 18.000 pixel devono cambiare
   nella regione Start (sinistra, sopra la taskbar), mentre i campioni
   dello sfondo remoto a destra restano identici.
6. Il terzo PPM originale e il PNG lossless entrano fra gli artefatti
   GitHub Actions. La verifica rifiuta catture incomplete, sfondi
   modificati, Start vuoti, immagini riciclate e assenza del terzo
   ACK DRM.
7. Il compositor controlla al termine che esistano effettivamente
   **due superfici GTK4 con proprietari distinti**, la taskbar e Start.

Le condizioni Stage84 e Stage85 restano valide: i primi due screenshot
sono ancora verificati autonomamente, con differenze dentro la barra
e uno sfondo immutato.

## Limiti

Questo test NON sostiene che Melania Desktop sia installabile o possa
eseguire una sessione completa. L'integrazione segue esclusivamente
una VM QEMU disposable e opt-in. Il terzo frame non sostituisce ancora
l'input reale via Super o mouse, né il menu Start di una sessione utente.

La PR resta **draft** fino al successo dei workflow Linux, SFTP,
Wayland, Mint e QEMU sull'ultimo commit.

`MELANIA_NATIVE_SESSION_READY=false`.
