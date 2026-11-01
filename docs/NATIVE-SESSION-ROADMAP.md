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
