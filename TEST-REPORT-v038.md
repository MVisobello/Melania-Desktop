# Melania 0.38 — Rapporto di verifica

**Ambiente:** Debian 13, display virtuale X11 disponibile. Cargo, rustc e i pacchetti di sviluppo GTK 4 non rilevati.

| Test | Esito | Significato |
|---|---|---|
| 61 controlli Python del repository | SUPERATI | Verifica strutturale e controllo delle regole di sicurezza dichiarate nei sorgenti |
| 14 test UX Lab Tk/X11 | SUPERATI | Regressioni su schede, riordino, layout e comportamento precedente |
| 4 nuovi test Rust del worker | SCRITTI, NON ESEGUITI | Copia completa, conflitto senza sovrascrittura, annullamento, link simbolici |
| `cargo check -p melania-files` | NON ESEGUITO | Mancano Cargo, rustc e GTK4 di sviluppo |
| Copie GTK reali / annullamento / selezione Ctrl-Shift | NON COLLAUDATI | Necessaria esecuzione GTK su Linux |
| Drag & drop verso Qt/GTK su Wayland | NON COLLAUDATO | Il codice resta COPY-only |
| Integrità archivio ZIP | Verificata nella creazione dell'archivio | Nessun membro corrotto |

## Avvertenze

I test Python esaminano presenza e struttura dei sorgenti; **non ne dimostrano la compilabilità**. I test UX precedenti riguardano il laboratorio grafico, che non implementa le nuove operazioni native. Non usare l'applicazione Rust pre-alpha con file importanti senza eseguire prima i test reali sopra indicati.
