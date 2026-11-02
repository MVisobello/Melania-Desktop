# Melania Native — Stage 25: xdg-shell e layer-shell in QEMU

## Obiettivo verificato (2 novembre 2026)

Il backend sperimentale `melania-native --experimental-wayland-seat`
ora può pubblicare il protocollo `xdg_wm_base` e il protocollo
`zwlr_layer_shell_v1`, oltre a `wl_compositor` e `wl_seat`.
Questo introduce le primitive Wayland per normali finestre e superfici
di pannello nel backend nativo.

Il collaudo usa **un vero client Wayland Rust**, eseguito in Ubuntu dentro
una VM QEMU TCG isolata con GPU VirtIO e VT dedicata. Il client crea
un `xdg_toplevel` e una `layer_surface`, riceve i messaggi
`configure`, risponde con `ack_configure` per entrambi ed esegue
il roundtrip di verifica.

Evidenze della prima VM riuscita:
[GitHub Actions #41416982697](https://github.com/MVisobello/Melania-Desktop/actions/runs/41416982697)
(artifact `qemu-real-melania-native-evidence`, serial.log).

```text
MELANIA_NATIVE_XDG_CONFIGURE_ACK=true
MELANIA_NATIVE_LAYER_CONFIGURE_ACK=true
MELANIA_NATIVE_SHELL_PROTOCOL_CLIENT_PASS=true
MELANIA_NATIVE_XDG_TOPLEVEL_COUNT=1
MELANIA_NATIVE_LAYER_SURFACE_COUNT=1
MELANIA_NATIVE_SHELL_PROTOCOLS_READY=true
MELANIA_NATIVE_SHELL_RENDERED=false
MELANIA_QEMU_NATIVE_SHELL_PROBE_PASS=true
MELANIA_QEMU_NATIVE_GUEST_EXIT=0
```

Nella stessa VM sono stati ripetuti gli esperimenti precedenti:
libseat/libinput con eventi USB virtuali che raggiungono il client,
TEST_ONLY Rust, framebuffer VM visibile e successivo ripristino CRTC,
e ritorno della console VT. La prova di questi passi precedenti rimane
distinta dal nuovo test dei protocolli.

## Limitazioni esplicite

**La prova non mostra una finestra GTK o un pannello sul monitor.**
Il bridge nativo non offre ancora un output/render loop che esegua lo
scanout dei buffer dei client. Il test di rendering visibile Stage 23
usa un programma libdrm QEMU separato e non può essere presentato come
rendering del compositore Rust.

Non sono ancora supportati:
- Gestione reale degli output/scale/posizioni e dei frame callback.
- Rendering di surface tree, cursor, layer stacking e buffer GBM/EGL su KMS.
- Lancio grafico di Melania Panel, Melania Files o Melania Settings.
- Interazione desktop completa, sicurezza e lifecycle della sessione.
- Avvio tramite display manager e rollback robusto dopo crash.

Restano obbligatori `MELANIA_NATIVE_SESSION_READY=false`,
`MELANIA_NATIVE_SHELL_RENDERED=false`, il gate del rollback Rust
chiuso e il percorso nested esistente invariato. Non installare una
sessione di login sul computer principale.

## Prossima fase

Unificare l'implementazione shell e seat con il compositor Rust
renderizzante: aggiungere `wl_output` e `wl_shm`, implementare lo
spazio per finestre/layer e gli hook dei frame callback; collegare
i buffer del client a un renderer KMS nel guest con un ripristino
robusto prima di provare Melania Panel/Files/Settings nella VM.
La GUI reale richiede un test di screenshot che mostri
**i pixel del client**, non solo il framebuffer sintetico Stage 23.
