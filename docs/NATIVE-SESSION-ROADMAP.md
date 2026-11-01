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
