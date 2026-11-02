# Linux Mint 22.x — pacchetto annidato con libreria GTK4 privata

Questa build è realizzata su **Ubuntu 24.04 LTS (Noble)**, base di
**Linux Mint 22.x**, anziché su Ubuntu 26.04. Un eseguibile compilato con
una `glibc` troppo recente non può essere reso compatibile semplicemente
copiando `libc.so.6`: la `glibc` resta sempre quella di Mint.

## Che cosa include il pacchetto

- Gli otto eseguibili Melania e lo sfondo di default.
- La libreria **`libgtk4-layer-shell.so.0`**, compilata nella stessa CI da
  `wmww/gtk4-layer-shell` v1.0.4 (commit verificato
  `536ff516ed68b9bb34afc4c07f942a54b2b4b03f`), inclusa sotto `lib/`.
- Il launcher `run.sh`, che usa questa libreria **soltanto per i processi
  della preview**, attraverso `LD_LIBRARY_PATH`. Non modifica `/usr`,
  `/lib`, `ldconfig`, il display manager o i driver del computer.
- Un manifest di hash SHA-256 e l'archivio verificabile con
  `SHA256SUMS`.

Non include `glibc`, GTK4 completo, Mesa, driver GPU, moduli GVfs e
l'intero stack di sistema. Questi componenti sono forniti e aggiornati
dal sistema Linux Mint. La build riduce la necessità di librerie extra,
**non è ancora una AppImage completamente autosufficiente**.

## Prova nella macchina virtuale Mint

Apri una sessione grafica Cinnamon/Xfce/MATE nella VM, poi scarica
l'artifact `melania-nested-preview-mint22-linux-x86_64` dal workflow
**Melania Linux Mint 22 compatibility preview** di GitHub Actions.

Estrai prima lo ZIP che GitHub ti consegna. Nella directory contenente
`melania-nested-preview.tar.gz` e `SHA256SUMS`:

```bash
sha256sum -c SHA256SUMS
tar -xzf melania-nested-preview.tar.gz
cd melania-nested-preview
./run.sh --check
./run.sh
```

Il comando `--check` esamina le librerie ELF richieste e segnala
gli eventuali componenti di sistema assenti. **Non modifica nulla**.

Quando vengono segnalate librerie come `libgtk-4.so.1`, installa
soltanto i relativi pacchetti di Mint con il suo gestore pacchetti
(`apt` o Gestore applicazioni), non copiare mai una `glibc`
prelevata da Ubuntu 26.04 o da Internet.

Per identificare esattamente versione e architettura di Mint:

```bash
cat /etc/linuxmint/info
uname -m
getconf GNU_LIBC_VERSION
```

Il risultato deve essere confrontato con il target documentato:
Mint 22.x x86_64 / Ubuntu 24.04 ABI. Mint 21.x **non è coperto**.
La build è sperimentale: la riuscita in CI su Ubuntu 24.04 non sostituisce
un collaudo manuale su una VM Linux Mint reale.

## VirtualBox: finestra Smithay ma barra e Impostazioni assenti

Nelle VM VirtualBox la scheda grafica virtuale può non supportare
correttamente il percorso EGL/DRI3/Vulkan. Messaggi come
`ZINK: failed to choose pdev`, `failed to create dri2 screen`
o client Wayland che si disconnettono indicano che il compositor può
restare avviato mentre alcune finestre GTK non riescono a disegnare.

Il launcher ora riconosce VirtualBox e prova automaticamente
**GTK4 Cairo + Mesa llvmpipe**, senza cambiare driver, impostazioni
di Mint o librerie installate. Si può forzare esplicitamente:

```bash
./run.sh --software
```

Per provare la GPU normale, disabilitando il fallback automatico:

```bash
MELANIA_SOFTWARE_RENDER=0 ./run.sh
```

Per diagnosticare un avvio incompleto:

```bash
MELANIA_DEBUG_RENDER=1 ./run.sh --software 2>&1 | tee "$HOME/melania-virtualbox.log"
```

Nell'output, `MELANIA_RENDER ... top=0` ripetuto può indicare
che la barra non sta producendo pixel. Le righe
`MELANIA_CLIENT_STARTED` e `MELANIA_CLIENT_EXIT` indicano
quale componente si è avviato o si è chiuso, insieme allo stato di uscita.
Lo smoke test CI ora richiede anche un vero layer grafico della barra.

Gli avvisi `xdg-desktop-portal`/FUSE possono essere indipendenti dal
problema EGL: non cambiare i permessi di `/run/user/UID` e non avviare
Melania come root. In caso di blocchi persistenti si può esaminare
separatamente l'interazione tra la sessione D-Bus isolata della preview
e i servizi desktop Mint.

Per le versioni del pacchetto *precedenti* a questa correzione,
il workaround manuale equivalente è:

```bash
GSK_RENDERER=cairo LIBGL_ALWAYS_SOFTWARE=1 \
  GALLIUM_DRIVER=llvmpipe MESA_LOADER_DRIVER_OVERRIDE=llvmpipe ./run.sh
```

## Correzione barra: icone e Centro notifiche (Mint / VirtualBox)

Le prime preview mostravano la barra senza le icone Melania perché il pacchetto
non conteneva il tema di icone usato dalla barra. La release aggiornata include
`share/icons/Melania-Preview/index.theme`, tre icone originali per Start, Files
e Settings e le varianti chiare/scure dei simboli Wi-Fi, Bluetooth, audio,
batteria e notifiche. Nessun tema GTK di Mint viene sovrascritto.

C'era anche un bug di gestione delle superfici layer-shell GTK4: premendo
l'icona del Centro notifiche accanto al gruppo Wi-Fi/Bluetooth, la chiusura
di `quick` tramite `set_visible(false)` poteva scollegare l'intero processo
Panel. Ora quick, notifiche e calendario si nascondono fuori dallo schermo,
con il layer Wayland ancora valido, proprio come il menu Start.

La verifica automatica della build prova davvero:
- avvio del pannello con tema `Melania-Preview` dal pacchetto estratto;
- attivazione dei controlli rapidi e segnale GTK `clicked` del Centro notifiche;
- presenza dei pixel del pannello dopo il clic e assenza dell'uscita del processo.

Per provare la versione corretta, scarica il nuovo ZIP
`melania-nested-preview-mint22-linux-x86_64` dal workflow GitHub Actions
**Melania Linux Mint 22 compatibility preview** dell'ultimo commit verde.
Estrai in una **cartella nuova** per evitare di mescolare i file vecchi e nuovi.

```bash
./run.sh --software
```

Per raccogliere le righe di errore se il pannello scompare ancora:

```bash
MELANIA_DEBUG_RENDER=1 ./run.sh --software 2>&1 | tee "$HOME/melania-panel.log"
```

La preview è sempre annidata e sperimentale: test CI verdi non sostituiscono
il collaudo manuale sul VirtualBox dell'utente.

## Fluidità Melania Files su VirtualBox

Con rendering software (Cairo oppure `LIBGL_ALWAYS_SOFTWARE=1`) Files ora
elabora al massimo 16 miniature nella prima scansione di una cartella, anziché
48. Nelle sessioni grafiche accelerate resta il limite precedente di 48.
Non viene eliminato o modificato alcun file personale.

Per distinguere un rallentamento del lettore GIO da uno del disegno GTK o della
generazione delle miniature, avvia la **nuova build** con diagnostica facoltativa:

```bash
MELANIA_PROFILE_FILES=1 ./run.sh --software 2>&1 | tee "$HOME/melania-files-performance.log"
```

Il log contiene `MELANIA_FILES_SCAN_MS` (lettura),
`MELANIA_FILES_RENDER_MS` (creazione delle tessere GTK),
`MELANIA_FILES_THUMBNAILS_MS` (decodifica in background) ed eventualmente
`MELANIA_FILES_UI_STALL_MS` (intervalli dell'event loop superiori a 250 ms).
La diagnostica non è attiva per impostazione predefinita. Il log può riportare
nomi e percorsi di cartelle: controllalo prima di condividerlo.

## Disinstallazione

Chiudi la finestra Melania ed elimina la directory estratta. Non
essendo installata una sessione di login, Mint continua a funzionare
normalmente; non servono privilegi `sudo` per avvio o rimozione.

## Per sviluppatori

La CI compila in un prefisso privato con:

```bash
bash scripts/build-mint-layer-shell.sh "$PWD/target/mint-runtime"
export PKG_CONFIG_PATH="$PWD/target/mint-runtime/lib/pkgconfig"
export LD_LIBRARY_PATH="$PWD/target/mint-runtime/lib"
```

La libreria è copiata nel pacchetto con
`scripts/package-nested-preview.py --gtk-layer-shell-lib ...`.
La CI prova l'avvio del **pacchetto estratto** in una sessione Xvfb isolata,
verifica la presenza delle finestre Melania Files/Settings e l'acquisizione
di pixel realmente renderizzati.
