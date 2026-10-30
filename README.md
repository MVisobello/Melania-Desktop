# Melania Desktop

Desktop environment sperimentale per Linux con design **Melania Aero**, progetto in Rust + GTK 4 + Wayland/Smithay.

> **Stato:** pre-alpha. L'interfaccia dimostrativa X11 non equivale a una sessione Wayland compilata; il codice Rust/GTK necessita ancora di verifica su un runner Linux.

## Componenti
- Compositore Wayland basato su Smithay
- Shell, pannello, menu Start e notifiche
- **Melania Files**, file manager con schede superiori
- Tema e icone **Melania Aero Classic**
- Laboratorio UX separato su Tk/X11

## Repository
Questo repository privato è stato creato per ospitare la sorgente **v0.38** e compilare Melania tramite GitHub Actions. La sincronizzazione dell'intero albero di lavoro è in corso; fino al caricamento dei sorgenti la repository non costituisce il backup completo del progetto.

Non confondere i prototipi grafici con componenti Rust già compilati.

## Build prevista
Il progetto locale include `.github/workflows/linux-verify.yml`, con controlli separati su Rust core, GTK 4, compositor Smithay e laboratorio X11. La build è da verificare dopo la pubblicazione dei sorgenti.

## Riservatezza
Repository privato. Non pubblicare token, credenziali, file di configurazione personali o cache di compilazione.
