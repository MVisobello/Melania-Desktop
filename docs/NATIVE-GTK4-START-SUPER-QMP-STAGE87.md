# Stage 87 — aprire il vero menu Start GTK4 con Super fisico in QEMU

La Stage86 provava il rendering del menu Start con una presentazione GTK4
temporizzata nella macchina virtuale. La Stage87 mantiene **le stesse tre
verifiche del vero framebuffer**, ma sostituisce l'apertura temporizzata con
una pressione e un rilascio del tasto Super della tastiera USB emulata.

## Catena di prova

1. La VM usa la propria tastiera `usb-kbd`, `libseat` e `libinput`, non la
   tastiera dell'host.
2. Dopo la prova QMP della seconda riverniciatura della barra GTK4, lo
   script host `inject-qemu-real-gtk-super-start.py` invia **due veri eventi
   QMP** (`input-send-event`: Meta_L down e up). Non invia comandi al
   pannello, non costruisce un PNG e non modifica lo stato Wayland.
3. Il bridge Rust, in modalità **QEMU-only** e con i dispositivi libinput
   già registrati, riceve i codici evdev 125/126. Solo una pressione
   autonoma seguita da rilascio invia `TOGGLE_START` al canale privato
   derivato da `MELANIA_CONTROL_SOCKET`. Se interviene un altro tasto,
   la scorciatoia è annullata.
4. Il pannello GTK4 esistente riceve il datagramma, apre il vero
   `melania-start` e scrive il marker QEMU di conferma.
5. Smithay importa il buffer `wl_shm` del menu Start, composita la
   taskbar più l'overlay nel terzo frame, e il ricevitore DRM Rust
   richiede un vero FB_ID sul CRTC QEMU.
6. Il monitor QMP host cattura lo screenshot del terzo framebuffer e
   verifica una regione Start realmente cambiata rispetto al secondo,
   con sfondo destro immutato. Le catture PPM e PNG originali restano
   artefatti GitHub Actions.

La CI **fallisce** se non arriva il tasto da libinput, se manca il
`TOGGLE_START` della vera app GTK4, se manca il terzo framebuffer oppure
se i pixel della zona Start non corrispondono alla prova.

## Ambito e limiti

Tutti i nuovi trigger sono vincolati alla VM dedicata e ai flag
`MELANIA_NATIVE_REAL_GTK_START_INPUT=YES_QEMU_VM` e
`MELANIA_QEMU_GTK_START_FROM_SUPER=YES_DISPOSABLE_QEMU`. La sessione
normale non apre Start automaticamente. L'input desktop generale,
tastiera di ricerca e mouse, lock screen, gestione VT/pause-resume e
login nativo **non sono ancora completi**. Questa fase non è una prova
di un desktop autonomo installabile:

`MELANIA_NATIVE_SESSION_READY=false`.
