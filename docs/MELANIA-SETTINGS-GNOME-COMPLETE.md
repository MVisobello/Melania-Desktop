# Melania Desktop — Impostazioni GNOME-like e controlli finestra Windows 7

Questa revisione completa l'interfaccia **GTK 4 reale**, senza sostituirla con screenshot o un prototipo Tk.

## Interfaccia

- Impostazioni: sidebar scorrevole con ricerca delle categorie, icone del tema Melania Fluent, evidenziazione della sezione corrente e pagina iniziale **Sistema**.
- Panoramica Sistema: gruppi Hardware e Software e utenti, con righe cliccabili e freccia che conducono alle pagine effettivamente esistenti.
- Schermo, Audio, Alimentazione, Archiviazione, Bluetooth, App, Account, Data e ora, Dispositivi, Privacy, Lingua e regione, Informazioni.
- Restano disponibili le opzioni persistenti precedenti: tema icone Fluent/Aero, Aspetto, desktop, Snap, spazi di lavoro, accessibilità, ecc.
- Notifiche: interruttore **Non disturbare** collegato a `melania-config`.

Le pagine di hardware ancora prive del relativo backend sono **informative** e non mostrano interruttori che fingono di aver modificato il sistema. Dove possibile leggono lo stato direttamente da GTK, `nmcli`, `wpctl`, `bluetoothctl`, `date`, `df` e sysfs, senza privilegi o scritture.

## Barra del titolo

La finestra Impostazioni mantiene il drag nativo `GtkHeaderBar`. I tre controlli personalizzati evocano Windows 7 (azzurri con gradienti; chiusura rossa) e chiamano le vere API GTK `minimize`, `maximize`, `unmaximize` e `close`. Il pulsante centrale modifica simbolo in base allo stato massimizzato. Melania Files mantiene i `windowcontrols` GTK nativi con lo stesso stile visivo attraverso `branding/window-chrome-win7.css`.

## Icone

Il tema principale resta **Melania Fluent Light**, con Dark e Light on Dark Panel. L'icona di categoria **Ufficio** è il disegno SVG originale e neutrale già presente in `tools/melania_office_artwork.py` (documenti, foglio, presentazione), senza il marchio Microsoft Office. Le icone delle applicazioni reali restano indipendenti.

## Verifiche

I file modificati sono sulla branch `feature/settings-gnome-complete-pages`.
La pipeline `.github/workflows/linux-verify.yml` controlla il codice GTK e Rust; `wayland-smoke.yml` la sessione annidata.
La nuova pipeline `.github/workflows/settings-visual.yml` costruisce il vero programma GTK e acquisisce su display virtuale gli screenshot delle pagine Sistema e Aspetto. Queste immagini sono **catture dell'applicazione**, non concept art.

La revisione non modifica il tema GTK dell'host, GNOME, Cinnamon o le configurazioni hardware di Linux.
