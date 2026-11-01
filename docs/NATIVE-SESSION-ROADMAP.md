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
