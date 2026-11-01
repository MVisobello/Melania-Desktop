# QA end-to-end del desktop Melania — schermate e input reali

Il workflow `.github/workflows/full-desktop-e2e.yml` costruisce il
**vero compositore annidato Rust/Smithay** assieme a Melania Panel,
Desktop Surface, Files e Impostazioni. Su GitHub Actions Ubuntu 26.04,
un display virtuale Xvfb offre un host X11 isolato: non è necessario
aprire la sessione grafica di un computer reale.

Dopo la compilazione, `scripts/smoke-full-desktop-wayland.sh` avvia il
compositore, aspetta che i veri client GTK siano mappati, verifica socket
Wayland e IPC, cambia workspace e controlla buffer rendered del server.
Campiona i pixel di screenshot effettivi catturati con `scrot` e verifica
le dimensioni dei pulsanti di chiusura delle finestre con Pillow.

Quando il display virtuale consente `xdotool`, il test esercita la
**tastiera del compositore**: Super chiude e riapre Start e la digitazione
entra nella ricerca. Verifica inoltre che il pannello cambi posizione
dal basso all'alto senza riavvio, che lo sfondo Desktop recepisca la nuova
configurazione e che il comando `melania-winctl quit` chiuda tutti i
processi del desktop.

Gli artifact `melania-real-full-desktop-screens` contengono screenshot
PNG, anteprima BMP della finestra Files e log di processo, ove prodotti.
Un colore di sfondo o una finestra GTK mappata senza buffer renderizzato
non sono considerati sufficienti.

**Questa prova è annidata.** Non esercita il backend `melania-native`,
non avvia un login manager, non accede a DRM host, non prova XWayland e non
dimostra un commit KMS o rollback. È il controllo di regressione del
desktop già funzionante durante la costruzione del backend autonomo.

Esecuzione manuale in una sessione X11 o con Xvfb dedicato, dopo aver
costruito i binari Rust:

```sh
xvfb-run -a -s '-screen 0 1280x800x24' \
  bash scripts/smoke-full-desktop-wayland.sh
```

Nessuna modifica alle impostazioni del desktop ospitante: il test usa
`XDG_RUNTIME_DIR` e `XDG_CONFIG_HOME` temporanei.
