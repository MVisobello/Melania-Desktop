# Melania v0.6 — milestone M4 (codice non ancora convalidato in Linux)

## Modifiche sostanziali

- `melania-core/src/interaction.rs`: calcolo deterministico move/resize per gli 8 bordi XDG, minima 120×80; spostamento calcolato sempre rispetto al punto iniziale, senza drift; input invalidi contenuti; cinque test unitari.
- `melania-compositor`: gestisce le richieste `xdg_toplevel.move` e `.resize` di Smithay 0.7. La richiesta è accettata solo se il suo seriale corrisponde al *pointer press* registrato sulla stessa superficie toplevel. Questo ostacola spostamenti avviati con seriali estranei.
- Per gli spostamenti il compositore aggiorna `Space::relocate_element`. Per i resize invia un `xdg_toplevel.configure` con il nuovo `size` e stato `Resizing`, che viene disattivato al rilascio del pulsante.
- Alla distruzione della toplevel, la finestra viene rimossa dallo Space.
- Avvio di più client test con ripetizioni di `--command PROGRAM`, tutti nel socket annidato di Melania: nessuna modifica all'ambiente del desktop host.
- Corretto `smithay` con feature esplicita `desktop` e `winit` bloccato alla versione `0.31.0-beta.3` utilizzata da Smithay 0.7.0.

## Come provare

**Non sostituire la sessione principale.** Prima effettuare un backup del codice e aprire un desktop Linux già funzionante.

```bash
sudo apt update
sudo apt install -y build-essential pkg-config libgtk-4-dev libwayland-dev \
  libxkbcommon-dev libegl1-mesa-dev libgles2-mesa-dev libx11-dev
# Installare Rust stabile (1.87+) tramite rustup e riaprire il terminale
./scripts/check-melania-v06.sh
./scripts/run-melania-v06.sh
```

Per testare il DnD, aprire un'altra finestra di Files nel socket `WAYLAND_DISPLAY=melania-N` annunciato all'avvio; fare prima le prove con file usa-e-getta, su destinazioni vuote. Per spostare una finestra, trascinare la barra del titolo GTK; per ridimensionarla, trascinarne i bordi client-side. Verificare che l'app GTK invii realmente le richieste XDG.

## Limiti ancora presenti

- Non è stata eseguita la compilazione: dipendenze Rust e GTK4 SDK non disponibili in questo ambiente di produzione. Le API usate sono state confrontate con i documenti di Smithay 0.7, ma potrebbero esserci incompatibilità nel primo build.
- La *shell* rimane un'applicazione GTK dimostrativa, non una superficie layer-shell sempre visibile, né un desktop completo. È aperta nel compositor come client normale.
- I popup XDG non sono ancora gestiti con PopupManager e non sono garantiti nella resa grafica.
- Drag & drop tra applicazioni, clipboard, touchpad, XWayland e pannelli globali devono essere testati end-to-end. Durante il *window move* il compositore sospende il targeting ordinario del puntatore.
- Le operazioni di copia in Files sono sincrone e per il momento sconsigliate su dati importanti. Lo spostamento verso la taskbar, le animazioni Aero e le finestre magnetiche (Snap) sono ancora nel backlog.
- Non è presente ancora un display manager, sessione DRM/GBM, hotplug multi-monitor o portali Flatpak.

## Criteri di uscita M4

1. Compilazione Rust di tutto il workspace su Linux con GTK4 SDK.
2. 5 test unitari di `interaction.rs` e test esistenti su `melania-transfer` / `melania-dnd` verdi.
3. Avvio annidato con tre client contemporanei, uso di mouse e tastiera senza errori.
4. Spostamento da barra titolo e resize dai bordi, senza perdita di focus.
5. DnD Files→Files con copia, MOVE verificato, annullamento e collisioni di nomi, senza perdita dati.

Solo dopo questi passi possiamo chiamare M4 «funzionante» e procedere a layer-shell.
