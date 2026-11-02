# Melania Desktop — Stage 27: output Wayland e screenshot reale della sessione

## Due verifiche differenti

**Backend nativo sperimentale (QEMU):** Smithay pubblica un vero `wl_output`
chiamato `MELANIA-QEMU-1` con modalità 1280×800 a 60 Hz e un
`xdg-output`. Un client Wayland di test deve ricevere tramite protocollo
l'evento `wl_output.mode` prima di poter inviare la propria finestra
`xdg-shell` con buffer `wl_shm`. La CI rifiuta risultati basati solo
su messaggi del server: l'evidenza del client e il ripristino KMS sono entrambi
obbligatori.

**Interfaccia completa reale (modalità annidata):** la CI esegue il compositore
Smithay con backend Winit su Xvfb e lancia le applicazioni GTK reali Melania
Desktop Surface, Panel, Files e Settings. Cattura lo schermo attraverso
`scrot`, richiede le due finestre effettivamente mappate e la differenza
visiva dopo aver digitato nella ricerca Start. Un piccolo programma di QA
conserva **una copia byte-per-byte** della foto originale come
`melania-real-desktop-preview.png`, con manifest JSON che indica provenienza,
dimensioni, hash SHA256 e test superati.

Le immagini non sono mockup, screenshot sintetici o anteprime prodotte
da un generatore. La copia pubblicata rappresenta il desktop **annidato**,
non una sessione KMS nativa.

## Esecuzioni

- `.github/workflows/qemu-melania-native.yml`:
  il client deve attestare `MELANIA_NATIVE_CLIENT_WL_OUTPUT_MODE=1280x800`,
  con la prova esistente dei pixel wl_shm, dello screenshot QMP e del
  ripristino originale del CRTC.
- `.github/workflows/wayland-smoke.yml`:
  produrre `reports/melania-real-desktop-preview.png` e
  `reports/melania-real-desktop-preview.json` e caricarli negli artefatti.

## Cosa manca ancora

Il backend nativo **non** ha un ciclo EGL/GBM + KMS che presenti
continuamente tutti i buffer delle finestre; non è un sostituto di
`melania-compositor` annidato. Non può ancora eseguire Melania Panel/Files/
Settings come sessione nativa. Sono ancora necessari frame callbacks, window
scene graph, layering, input focus complesso, robusto rollback del renderer,
test multi-output e un display manager. La sessione di login resta disabilitata.

Non equiparare un `wl_output` pubblicato a un vero display fisico pronto.
