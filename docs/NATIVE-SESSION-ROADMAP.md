# Melania Desktop — Avvio di sessione: stato reale e obiettivi

## Stato verificato
Il compositore corrente usa **Smithay 0.7 con backend Winit** e si avvia dentro una
sessione grafica Linux già funzionante. Il test CI include GTK4/Wayland reali,
un desktop layer, menu Start, finestre, IPC per gli spazi di lavoro e screenshot,
ma **non equivale** a un compositor autonomo DRM/KMS.

## Prova immediata: senza sostituire Linux
Da una sessione Linux funzionante:

```sh
bash scripts/melania-session --nested
```

Facoltativamente aggiungi un lanciatore al menu applicazioni dell'utente:

```sh
bash scripts/install-melania-preview-launcher.sh
```

Questo installa soltanto `~/.local/bin/melania-session` e una voce XDG per la
**anteprima annidata**. Non crea voci di login in
`/usr/share/wayland-sessions` e non modifica il display manager.

Per verificare la disponibilità dell'avvio nativo:

```sh
bash scripts/melania-session --check-native
```

La risposta è intenzionalmente `NATIVE_BACKEND_UNAVAILABLE` (exit code 78)
finché non vengono completate le dipendenze sottostanti; non si tentano
operazioni su KMS, VT, logind o arresto del computer host.

## Diagnostica hardware preliminare, senza privilegi

La nuova verifica legge soltanto informazioni pubbliche del sistema
(dispositivi DRM da `/dev/dri`, stato dei connettori da `/sys/class/drm`,
nodi input e proprietà della directory di sessione). Non prende il controllo
di VT, schede video o dispositivi d'ingresso:

```sh
bash scripts/melania-session --probe-native
```

L'output è JSON per agevolare una successiva pipeline di test hardware, e il
comando termina **sempre con codice 78** finché Melania usa Smithay/Winit.
L'esistenza di un `card0`, di un monitor collegato o di un
`XDG_RUNTIME_DIR` valido non dimostra che libseat e DRM/KMS siano operativi.
Le prove automatiche usano una struttura `/dev` e `/sys` sintetica, mai i
dispositivi reali dell'host CI. Per un confronto riproducibile si può usare
`python3 scripts/probe-native-session.py --root /percorso/fixture --json`.

## Smithay backend nativo — fase 1 (libseat/udev)

Il nuovo crate Rust `melania-native` compila le feature Smithay
`backend_session_libseat`, `backend_udev`, `backend_drm`,
`backend_gbm` e `backend_libinput`. In particolare implementa un registro
delle GPU con hotplug e un ciclo di vita della sessione per gli eventi
`PauseSession` e `ActivateSession`, con test unitari. In questa fase
**non** configura un renderer KMS e non sostituisce il compositor Winit.

Per verificare la compilazione e l'elenco delle capacità senza accedere
all'hardware:

```sh
cargo test -p melania-native
cargo run -p melania-native -- --capabilities
```

Solo per un esperimento consapevole in **VM dedicata**, è disponibile il
monitor Smithay/libseat/udev di durata limitata: richiede una vera console
`/dev/ttyN`, nessuna sessione grafica ospitante e la variabile
`MELANIA_EXPERIMENTAL_SEAT=1`. Questo componente fa solo il monitoraggio
degli eventi: **non** può presentare finestre né aprire un desktop.

Non eseguirlo nel desktop di produzione. Se libseat/udev è indisponibile,
deve terminare con errore, senza usare un backend alternativo nascosto.
Il prossimo passo consiste in opening dei DRM FD **attraverso la sessione**,
creazione GBM/EGL, mapping CRTC/output, pause/resume degli output, input
libinput e avvio dei client Wayland.

## Acquisizione dispositivi DRM — fase 2 sperimentale

Il percorso Rust opt-in --experimental-drm inizializza libseat, rileva i
dispositivi primari con Smithay/UdevBackend, apre i loro file descriptor
attraverso Session::open con i flag RDWR/CLOEXEC/NOCTTY/NONBLOCK, costruisce
DrmDeviceFd e interroga i CRTC disponibili con DrmDevice. Imposta
disable_connectors=false per non azzerare intenzionalmente i connettori
esistenti. Rilascia il dispositivo al termine dell'ispezione, senza creare
buffer, surface KMS, mode setting, renderer o client Wayland.

La modalità è riservata esclusivamente a una VM/TTY Linux dedicata con opt-in
MELANIA_EXPERIMENTAL_SEAT=1; non si avvia da Start, dal login o dalla CI.
Nessun hardware DRM reale viene aperto nei runner GitHub.

L'acquisizione DRM master può temporaneamente interagire con le risorse
grafiche della console: **non usare la prova sulla macchina di lavoro**.
Anche qui NATIVE_SESSION_READY=false: non è una sessione desktop.

## Inventario connettori e modalità — fase 3

Il modulo Rust `melania-native/src/outputs.rs` interroga con il trait drm-rs
`control::Device` i connettori rilevati dal dispositivo già aperto tramite
libseat nella modalità sperimentale `--experimental-drm`.
Legge lo stato connesso/scollegato, dimensioni fisiche dichiarate e modalità
video pubblicate; evita ricerche infinite durante gli eventi hotplug e
ordina/deduplica le modalità. Una scelta **provvisoria** basata su risoluzione
e refresh viene mostrata in sola lettura, non applicata.

La fase 3 **non** determina ancora la compatibilità encoder/CRTC/plane,
il flag EDID preferred, il mapping degli output Wayland, l'acquisizione
GBM/EGL o la gestione delle VT. Nessun modeset o renderer è stato abilitato.
I test includono monitor scollegati, EDID non valido e selezione deterministica
della modalità. Ogni test hardware richiede sempre una VM dedicata e l'opt-in
già presente. Il desktop annidato non viene modificato.

## Instradamento monitor su CRTC — fase 4

La lettura DRM restituisce anche gli encoder associabili a ciascun connettore,
e il filtro `possible_crtcs` è applicato alle risorse disponibili.
Un pianificatore Rust **puramente in memoria** calcola un abbinamento
uno-a-uno fra monitor e CRTC con cammini aumentanti: non assegna mai lo
stesso controller a due display né inventa CRTC mancanti.

La pianificazione è diagnostica e non attiva output, né richiede una
superficie DRM o un renderer GBM/EGL. Essa non garantisce ancora la presenza
di primary plane, memoria grafica o una modalità supportata e applicabile.
Il codice non modifica il comportamento dell'ambiente Wayland annidato.
La CI prova anche il caso in cui un abbinamento greedy ridurrebbe il numero
di monitor gestibili.

## Inizializzazione GBM/EGL/GLES — fase 5

Il nuovo modulo Rust `melania-native/src/renderer.rs` usa il **vero**
file descriptor DRM aperto mediante libseat (nelle prove controllate di
`--experimental-drm`) per inizializzare `GbmDevice`, `EGLDisplay`,
`EGLContext` e `GlesRenderer` attraverso Smithay 0.7. Rileva il nome del
backend GBM e il numero di formati DMA-BUF renderizzabili. Questo dimostra la
possibilità di creare un contesto grafico sulla GPU, non ancora il funzionamento
di uno schermo.

**Non crea** GBM scanout buffer, superfici DRM, framebuffer, render loop,
VT switch, input libinput o sessione di login. Se il driver EGL non è
disponibile, la diagnostica lo segnala senza cambiare modalità video.
Anche con EGL funzionante, `native_scanout_is_implemented()` continua a
restituire `false`.

Il collaudo Linux CI compila e testa il percorso, ma non può inizializzarlo
su un vero dispositivo DRM perché i runner non hanno una console VT
dedicata. Il controllo GPU effettivo deve essere eseguito solo in una
**macchina virtuale dedicata** con l'opt-in già previsto; non sul desktop
principale. Prima del primo frame KMS devono ancora essere verificati:
primary plane, formati compatibili GBM/scanout, gestione pause/resume,
acquisizione/rilascio dello stato CRTCs e ripristino schermo/VT.

## Primo frame in memoria GPU — fase 6 (nessuno scanout)

Un percorso esplicito `melania-native --experimental-offscreen` riutilizza
tutti i controlli su TTY dedicata, libseat e file descriptor DRM delle fasi
precedenti. Inizializza GBM/EGL/GLES, alloca un singolo DMA-BUF **64×64**
con flag di rendering (non scanout), disegna un riquadro di prova con
`GlesRenderer` e attende la sincronizzazione GPU prima di distruggerlo.
È una prova di rendering reale in memoria grafica, distinta dalla sola
rilevazione delle capacità EGL.

**Non** crea superfici KMS, non modifica i CRTC, non cambia frequenza o
risoluzione, non presenta frame su un monitor e non sostituisce la sessione
Wayland annidata. La CI verifica la compilazione e il blocco su TTY non
dedicate; **non** possiede una GPU virtuale da cui ottenere screenshot del
frame. La prova hardware resta da effettuare esclusivamente in una VM
dedicata, consapevoli che libseat/DRM possono interagire con la console.

La sessione autonoma non è ancora pronta: prima dello scanout servono
primary plane, formato compatibile render/scanout, DRM framebuffer, gestione
VT pause/resume, ripristino delle impostazioni precedenti e test reali in VM.

## Piano KMS — fase 7: verifica dei primary plane (sola lettura)

Dopo aver identificato i CRTC ed eseguito il frame offscreen da 64×64,
Melania ora controlla i **primary plane** disponibili per ogni CRTC tramite
`DrmDevice::planes`. Confronta i formati e i modificatori realmente
pubblicizzati dal kernel con quelli del buffer di prova
(`ABGR8888 + LINEAR`). Un CRTC senza primary plane o senza corrispondenza
esatta viene escluso dai candidati per un futuro scanout.

Si tratta di una **verifica preliminare, non di un'abilitazione**:
la corrispondenza dei formati non dimostra che il kernel accetterà la
composizione, non verifica le property KMS richieste, non preserva ancora
lo stato del display e non gestisce il ciclo di page-flip. Nessuna
`DrmSurface`, `set_crtc`, framebuffer, atomic commit o page flip viene
creata da questa fase. Nessun test GPU viene effettuato sul computer
dell'utente.

## Preparazione pause/resume e sincronizzazione frame — fase 8

Il nuovo `NativeFrameGate` collega la gestione reale degli eventi Smithay
`PauseSession` e `ActivateSession` e gli eventi di hotplug udev a un
contatore di generazione. Ogni pausa, ripresa o cambio di dispositivo
**invalida i frame preparati prima della transizione**. Il modello rifiuta
candidati quando la sessione è inattiva, il piano primario non è stato
verificato o un altro candidato è ancora in attesa, e non accetta la
conclusione di un frame appartenente a una generazione precedente.

Questa fase non esegue ancora `DrmSurface::page_flip`, atomic commit o
ripristino del display: si tratta di **un modello di sicurezza integrato
nel ciclo di vita, non di un driver KMS operativo**. I test Rust verificano
concorrenza, sospensione, ripresa, ripetizione di eventi e rimozione GPU.
Per il rendering reale serviranno inoltre sospensione libinput, rilascio
dei buffer pendenti, un backup del vecchio stato KMS e una prova di
recupero della VT in VM con timeout e procedura di rollback.

## Fase 9 — prova atomica KMS TEST_ONLY, riservata a QEMU

Melania ora dispone di un esperimento che costruisce una superficie DRM
sulla prima uscita virtuale idonea, crea uno swapchain GBM con
`RENDERING | SCANOUT`, richiede al driver una validazione **atomic
TEST_ONLY** tramite `GbmBufferedSurface::new`, quindi disegna un
frame GLES nel buffer e lo sincronizza **senza inviarlo allo schermo**.

La prova è disattivata per impostazione predefinita e richiede
simultaneamente: vera console `/dev/ttyN`, nessun server grafico
ospitante, libseat, riconoscimento QEMU/KVM da DMI e le due variabili di
opt-in. **Non provarla sul computer principale o su una VM condivisa.**

Solo dopo aver predisposto una VM Linux dedicata, da una console testuale
della VM:

```sh
MELANIA_EXPERIMENTAL_SEAT=1 MELANIA_KMS_TEST_ONLY=YES_QEMU_VM \
  cargo run -p melania-native -- --experimental-kms-test
```

Se l'hardware virtuale non supporta DRM atomic, EGL, formati GBM o plane
compatibili, il comando termina con errore. La CI compila questo percorso
ma non lo esegue su dispositivi grafici. In particolare, **non esegue
mai commit, page flip, modeset o configurazione del display manager**.
Neanche un TEST_ONLY superato costituisce una prova di scanout o
ripristino della console. Prima di abilitare un primo frame visibile sono
necessari un event loop DRM/vblank, gestione corretta di pausa/ripresa
libseat, un rollback della configurazione KMS e collaudo manuale su VM.

## Fase 10 — preflight KMS a memoria limitata e CRTC in sola lettura

La prova atomica QEMU `--experimental-kms-test` non assume più che la prima
modalità pubblicizzata sia quella più sicura. Fra le modalità reali supportate
seleziona quella con area minore entro **640×480–1920×1080**; se nessuna è
ammissibile, non viene allocato alcuno swapchain GBM. Prima di costruire la
superficie DRM, interroga inoltre lo stato del CRTC tramite `get_crtc` in
**sola lettura**; se la lettura non riesce, quella combinazione non è provata.

La risposta della prova espone `crtc_readable` separatamente da
`visible=false`: **leggere il CRTC non significa averne salvato l'intero
stato ripristinabile**, comprese le proprietà atomiche di plane/connettori.
Né questa prova né il suo risultato consentono commit, page flip, modeset,
ripristino della console o apertura di una sessione nativa. Restano attivi
tutti i blocchi precedenti: QEMU/KVM, TTY dedicata, due opt-in, nessuna
sessione grafica ospitante. In CI vengono eseguiti i test Rust del selettore
delle modalità e il controllo che non si usino dispositivi grafici reali.

Il prossimo vero traguardo richiede uno snapshot *completo e ripristinabile*
dello stato KMS, un event loop vblank/page-flip e un rollback sperimentato
in una macchina virtuale isolata **prima** di abilitare la presentazione.
Non usare l'esperimento per prendere il controllo del PC principale.

## Fase 11 — inventario atomico KMS e confronto prima/dopo (nessun ripristino)

Il percorso **QEMU/TTY opt-in** `--experimental-kms-test` esegue una lettura
completa, con limiti sul numero di oggetti/proprietà, delle proprietà atomiche
di **tutti** i CRTC, connettori e plane pubblicati dal driver. Memorizza in
memoria del solo processo gli identificativi e i valori grezzi; rifiuta
proprietà critiche mancanti o duplicate, inclusi `ACTIVE`/`MODE_ID`
dei CRTC, `CRTC_ID` dei connettori e i riferimenti `FB_ID`, `SRC_*`
e `CRTC_*` dei plane. Se la lettura fallisce, la prova termina senza
preparare buffer o superfici.

Dopo il test atomico `TEST_ONLY` e il disegno GLES *non visualizzato*,
Melania ripete la lettura e confronta **identità, numero e valori**
di tutti gli oggetti. Qualsiasi differenza, per esempio hotplug o intervento
di un altro client, produce errore. Il log espone
`baseline_unchanged=true` soltanto quando il confronto riesce e
`restore_ready=false` **sempre**.

Questo è un **inventario in sola lettura**, non un backup ripristinabile:
gli ID delle proprietà non preservano automaticamente gli oggetti framebuffer,
i blob `MODE_ID`, la configurazione VT, lo stato del seat e gli eventi DRM.
Non è presente alcuna funzione che applica uno snapshot: niente atomic
commit, page flip, modeset o ripristino delle risoluzioni. La prova non
può partire nel desktop ospitante e nessuna GPU reale viene usata nei runner
GitHub.

**Passo successivo necessario:** un protocollo di acquisizione dei
framebuffer/blob e delle risorse con ownership esplicita, gestione pausa/ripresa,
test di ripristino dell'output in una VM QEMU dedicata e watchdog che garantisca
la restituzione della console. Solo dopo potrà esistere un primo frame nativo.

## Fase 12 — conservazione temporanea dei payload KMS (senza ripristino)

Il test opt-in `--experimental-kms-test`, esclusivamente su **QEMU con una
console TTY dedicata**, conserva ora in memoria del processo una copia dei
payload *già esistenti* associati a `MODE_ID`, `GAMMA_LUT`,
`DEGAMMA_LUT`, `CTM` e `HDR_OUTPUT_METADATA`. Memorizza inoltre le
associazioni `FB_ID` dei plane, ma queste sono **soltanto identificativi
osservati**, non handle o storage grafico posseduti e garantiti vivi.

L'inventario è limitato a 128 blob da massimo 256 KiB ciascuno,
**2 MiB totali** e 128 riferimenti framebuffer. Rifiuta CRTC attivi
senza `MODE_ID`, letture non riuscite, payload vuoti o troppo grandi.
Dopo il test atomico `TEST_ONLY` e il rendering non visibile, confronta
non soltanto gli ID/proprietà della fase 11, ma anche i **byte dei payload**
e i riferimenti FB. Se anche uno solo è diverso, il test fallisce.

Questo avanzamento **non** prende possesso dei vecchi framebuffer/GEM,
non trattiene in vita i blob del kernel e non ricrea un valore `MODE_ID`
in DRM: `restore_ready=false` e `visible=false` restano invariati.
Senza conservazione degli oggetti kernel e transazioni atomiche reversibili,
la copia dei byte NON permette un ripristino sicuro dello schermo.

### Procedura di collaudo opzionale, solo in VM QEMU isolata

È disponibile `scripts/smoke-qemu-kms-resources.sh`, con timeout
di 45 secondi. Lo script **non crea né avvia la VM**. L'operatore deve
prima predisporre una VM Linux QEMU/KVM completamente dedicata, avviata
su una console testuale `/dev/ttyN` locale (non SSH/PTY), senza
`DISPLAY` o `WAYLAND_DISPLAY`. Compilare il progetto **dentro la VM**
con `cargo build -p melania-native`, verificare il dispositivo virtuale
DRM, quindi eseguire **dentro la VM**:

```sh
MELANIA_QEMU_RESOURCE_AUDIT=I_AM_IN_AN_ISOLATED_QEMU_VM \\
MELANIA_EXPERIMENTAL_SEAT=1 \\
MELANIA_KMS_TEST_ONLY=YES_QEMU_VM \\
bash scripts/smoke-qemu-kms-resources.sh
```

Il programma ripete i controlli DMI QEMU, seat e TTY; gli opt-in da soli
non sono sufficienti. La CI GitHub **non esegue questa prova GPU**, ma
testa l'algoritmo Rust e verifica che lo script rifiuti un runner condiviso,
anche se vengono impostate le variabili di opt-in.

**Prima di qualsiasi primo frame nativo** restano indispensabili
l'ownership dei vecchi framebuffer, dei blob DRM e dello stato VT,
una transazione atomica di ripristino, un watchdog di recovery verificato
e prove distruttive **solo** in una VM con ritorno certo alla console.
Non eseguire la procedura sul computer principale.

## Fase 13 — guardia RAII dello snapshot, scadenza e controlli del seat

Il test QEMU `--experimental-kms-test` ora trasferisce i dati atomici
e le copie temporanee dei blob a una **guardia Rust RAII**
(`melania-native/src/preflight_guard.rs`). La guardia ne controlla
insieme la validità, lo stato del seat e una scadenza interna massima
di 40 secondi (nell'esperimento 36 secondi). I checkpoint operativi
precedono la preparazione del buffer, seguono la sincronizzazione GPU e
precedono il risultato. Se il seat non è attivo, la scadenza è superata,
cambia uno dei valori KMS o variano i byte dei blob, l'esperimento
è respinto e la guardia non può essere riutilizzata. I test Rust
simulano pausa/ripresa, hotplug, scadenza e modifiche dello stato.

La guardia è proprietaria **delle copie in memoria degli snapshot**,
non dei vecchi oggetti kernel DRM, e ne ripulisce le copie dei blob
alla distruzione in modalità *best effort*. Non vengono creati
framebuffer o blob DRM per ripristinare una configurazione. Il controllo
`session.is_active()` senza dispatch completo dei notifier libseat
non è un rilevamento in tempo reale garantito della perdita della VT.
Analogamente, il deadline cooperativo non interrompe un ioctl bloccato:
rimane necessario il `timeout` esterno allo script QEMU. Nessuno dei
due meccanismi garantisce da solo la restituzione della console.

**Restano quindi disabilitati scanout visibile e restore**. La fase
successiva richiede la gestione effettiva degli eventi libseat/udev
mentre il test GPU è in esecuzione, la conservazione dei vecchi oggetti
framebuffer e blob DRM nel kernel, rollback atomico verificabile e
prove reali del passaggio di VT nella sola macchina virtuale isolata.

## Lavoro necessario per una vera sessione di login
1. Backend DRM/KMS (o libdisplay-info + GBM/EGL) nel compositore, discovery di
   output reali e hotplug; renderer/headless fallback e test espliciti.
2. Seat management con libseat/logind, gestione VT e permessi,
   suspend/resume e chiusura pulita senza sottrarre il controllo all'host.
3. Input libinput e udev; keyboard, pointer e touchscreen, scaling e
   sincronizzazione delle coordinate di output.
4. Runtime per utenti: D-Bus per-sessione, XDG_RUNTIME_DIR,
   xdg-desktop-portal, polkit e chiusura sicura dei client.
5. Prima un avvio su VM/TTY **dedicata**, poi packaging `wayland-sessions`
   e compatibilità con almeno un display manager; documentare rollback.
6. Test dopo il logout, ritorno a login, blocco schermo, docking e monitor
   multipli prima di abilitare azioni di alimentazione native.

**Criterio di accettazione:** sessione DRM/seat indipendente che apre davvero
GTK/Qt, avvia Melania Panel/Files e restituisce la VT senza blocchi.
Non dichiarare completata la issue #24 usando la sola anteprima nested.
