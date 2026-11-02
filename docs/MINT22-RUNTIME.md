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
