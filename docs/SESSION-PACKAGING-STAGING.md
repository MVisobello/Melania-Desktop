# Melania Desktop — preparazione dei launcher senza sessione di login non sicura

## Stato reale

Il compositore `melania-compositor` funziona attualmente **annidato**
in una sessione X11/Wayland già esistente. Il backend
`melania-native` è sperimentale: i test QEMU di DRM `TEST_ONLY`
non costituiscono ancora una sessione Wayland utilizzabile.

Questo repository quindi **non** installa alcun file
`/usr/share/wayland-sessions/melania.desktop` o
`/usr/share/xsessions/melania.desktop`: un login manager non deve
offrire un'opzione che potrebbe lasciare uno schermo nero.

## Comandi disponibili

```sh
python3 scripts/stage-melania-session.py --status
python3 scripts/stage-melania-session.py --stage-preview
python3 scripts/stage-melania-session.py --stage-native
```

`--status` mostra un resoconto JSON dei prerequisiti: stato
nativo non pronto, possesso dei framebuffer, rollback KMS, input
Wayland hardware e ciclo sessione mancanti.

`--stage-preview` prepara **soltanto** il launcher della modalità
annidata sotto
`target/melania-session-staging/applications/org.melaniadesktop.NestedPreview.desktop`.
Non modifica `/usr`, `~/.local/share` o le directory di sessione.
L'applicazione usa la modalità `scripts/melania-session --nested`,
che esige un desktop grafico ospitante.

`--stage-native` restituisce invece codice **78** e segnala
`NATIVE_LOGIN_SESSION_BLOCKED`, a prescindere dai dispositivi
DRM visibili. In questa fase non esiste una procedura di
installazione nativa o una voce del login manager.

## Soglia per l'installazione futura

Prima di creare un pacchetto `wayland-sessions`, serviranno
tutte le seguenti prove reali, non soltanto marker statici:
possesso/ripristino delle risorse KMS con un vero frame visibile
in una VM isolata; completa gestione VT/libseat e input
`wl_seat` in pausa/ripresa; avvio del compositor Wayland che
accetta client GTK/Qt, lancia pannello, Files e Settings e torna
al login manager dopo logout senza blocchi.

La nuova procedura e i test di regressione non cambiano
`NATIVE_SESSION_READY=false`.
