# Stage 85 — due frame reali GTK4 su CRTC QEMU

## Obiettivo

Partendo dal collaudo a singolo frame Stage84 (già integrato in `main`),
dimostrare due presentazioni **effettive e differenti** del pannello
`melania-panel`, generato da GTK4, tramite Smithay e il framebuffer
DRM/KMS della stessa VM QEMU.

Questa è una prova di avanzamento del rendering, NON è un desktop
installabile, una shell GTK completamente interattiva o uno screenshot
simulato.

## Percorso della prova

1. La VM disposable avvia una **seconda sessione indipendente** dopo il
   collaudo Stage84. Tutte le protezioni QEMU DMI, TTY1, libseat e
   opt-in DRM restano obbligatorie.
2. Il vero binario `melania-panel` avvia la taskbar con tema originale
   `Melania-Fluent-Light`. Il primo `wl_shm` è composto e presentato
   sul CRTC; il receiver emette
   `MELANIA_RUST_REAL_GTK_TWO_FRAME_VISIBLE=1`.
3. **Solo nell'esperimento QEMU** il codice GTK modifica una classe CSS
   del proprio widget taskbar dopo 4,5 secondi. È GTK a disegnare e
   inviare il secondo buffer, non un client di test o bitmap inventati.
4. Il compositore mantiene la stessa superficie
   `zwlr_layer_shell_v1`, registra una nuova generazione grafica, la
   consegna al worker Rust e attende il secondo FB_ID reale. Il receiver
   emette `MELANIA_RUST_REAL_GTK_TWO_FRAME_VISIBLE=2`.
5. Un processo host separato, già in ascolto prima dell'avvio QEMU,
   cattura due screendump PPM tramite il protocollo QMP **durante**
   ogni scanout. Verifica risoluzione 1280×800, area GTK4 alta 60px,
   identità assoluta dello sfondo esterno e almeno 2400 componenti RGB
   modificate all'interno della taskbar.
6. I due file originali PPM rimangono fra gli artefatti GitHub Actions;
   vengono esportati anche due PNG lossless, senza ritocchi.
7. Guest, host e CI dichiarano il risultato positivo solo dopo doppio
   ACK real KMS, conferma dell'effettiva modifica GTK, conservazione
   della proprietà del buffer e ripristino del CRTC originario.

## Confini e regressioni

La modifica CSS, i timer e il conteggio a due frame esistono soltanto
dietro flag VM espliciti. Non vengono alterati i temi reali Light,
Dark, Aero né i controlli del pannello in una sessione utente.

Le prove precedenti, inclusi Stage84 a **un frame** e i vecchi
12–14 frame dei client QEMU di test, mantengono i valori originali.
La prova deve rimanere in PR **draft** finché Linux, SFTP, nested
desktop e QEMU non superano tutti i controlli sul commit finale.

`MELANIA_NATIVE_SESSION_READY=false`: mancano ancora un compositore
continuo realmente utilizzabile, input/taskbar completo, login manager,
clipboard/DnD nativo e validazione su hardware fisico.
