# M9 — vere miniature per finestra (primo renderer controllato)

## Scopo
Due finestre Melania Files condividono una sola icona taskbar. Passando il
mouse sull'icona, il popover mostra le miniature **reali** di quelle finestre,
con il nome della cartella, e permette di attivare quella desiderata.

## Architettura di protezione
- Nessuna API di screenshot GTK/X11 e nessuna fotografia dell'intero desktop:
  Smithay rende **soltanto** la surface tree del window ID richiesto in un
  framebuffer GLES separato, su sfondo scuro.
- Il compositore permette massimo due catture per frame, 256×160 pixel,
  validando geometria, ID finestra, risposta e dimensione buffer.
- Canale IPC su socket privato 0600 già esistente; nuove richieste
  `PREVIEW <window_id>`, risposta bounded `PREVIEW <private-path>`.
- BMP a 32 bit senza codec o processi esterni, file creati con `create_new`
  in directory privata 0700 sotto XDG_RUNTIME_DIR e pulizia in shutdown.
- I client minimizzati non vengono catturati; selezione e chiusura finestra
  continuano tramite IPC esistente.
- La preferenza `preview_windows=false` disattiva le catture del pannello
  dopo il suo riavvio (il selettore icona/titolo rimane utilizzabile).
- Al massimo sei miniature per popover sono caricate in una singola apertura;
  sopra sei finestre restano presenti i titoli e i pulsanti di selezione.

## Qualità
Le miniature sono acquisizioni on-demand a 256×160 e non ancora flussi video
a refresh continuo. L'utente può riaprire il popover per aggiornarle. Resta
una pre-alpha: bisogna testare orientamento dei pixel e composizione alpha
con GlesRenderer sui driver hardware, oltre al CI llvmpipe.

## Da verificare prima di distribuire
- `cargo test -p melania-control -p melania-compositor`;
- `cargo check -p melania-panel -p melania-compositor`;
- due finestre Files + popup hover + cattura distinta e selezione corretta;
- nessun pixel proveniente da finestre sovrapposte o wallpaper;
- controllo FPS con due preview e fallback se lettura GPU non disponibile;
- contenuto dei BMP leggibile da GdkPixbuf/GTK in ambiente grafico reale;
- nessun percorso accessibile a utenti diversi, cleanup dopo EXIT.
- Smoke test del compositor Wayland annidato.

## Aero blur: ambito separato
Le miniature sono il primo utilizzo di un rendering offscreen con GLES.
**Non** attivano automaticamente la sfocatura Aero delle finestre.
Per quest'ultima serviranno pass separabili sul backdrop e compositing
delle sole superfici che dichiarano trasparenza, con fallback energetico.
