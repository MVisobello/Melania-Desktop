# Centro rapido 0.3: funzionalità reali e limiti

La GUI può controllare i servizi disponibili con comandi esterni senza `sudo` e senza invocare una shell. Le operazioni richiedono pacchetti installati, daemon avviati e autorizzazioni utente. La GUI mostra errori invece di dichiarare sempre successo.

| Azione | Adapter | Nota |
|---|---|---|
| Wi-Fi | `nmcli radio wifi` | NetworkManager richiesto |
| Bluetooth | `bluetoothctl show/power` | BlueZ con controller presente |
| Volume | `wpctl get-volume/set-volume` | PipeWire/WirePlumber richiesti |
| Luminosità | `brightnessctl -m/set` | Hardware + autorizzazione; desktop mostra non disponibile se assente |
| Profilo energetico | `powerprofilesctl get/set` | power-profiles-daemon e profilo supportato |
| Non disturbare | File configurazione Melania | Solo preferenza; **non** silenzia le notifiche di altre app finché non esiste `melania-notify` |
| Tema scuro | File configurazione Melania | App preview legge la preferenza all'avvio; un cambio nell'app Impostazioni non aggiorna una sessione già aperta |

`melania-control status` interroga le capacità. `melania-control wifi off` disattiva veramente il Wi-Fi **se** NetworkManager consente l'operazione. Evita di farlo su una macchina collegata esclusivamente in Wi-Fi da remoto.

I servizi di controllo sono sincroni in questo primo prototipo GTK; in futuro saranno convertiti in operazioni asincrone D-Bus con stato reattivo.
