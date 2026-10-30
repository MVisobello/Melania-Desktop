# Architettura tecnica Melania (0.3, ancora pre-alpha)

```text
                    +-------------------------------+
                    | Melania Shell (GTK4 + Rust)    |
                    | Start, taskbar, quick controls |
                    +------------+------------------+
                                 |
             future private window protocol + layer-shell
                                 |
                    +------------v------------------+
                    | Melania Compositor (DA FARE)   |
                    | Rust, Smithay, Wayland         |
                    | Rendering, input, xdg-shell    |
                    +------------+------------------+
                                 |
                    Wayland clients / XWayland future
```

**Attualmente:** `melania-preview` disegna desktop, taskbar e pannelli *dentro una sola finestra GTK 4*, NON nella vera superficie shell Wayland. La ricerca app usa `gtk::gio::AppInfo::all()`; gli adapter `melania-services` invocano comandi Linux senza usare shell interpolata. `melania-config` salva preferenze utente.

`melania-core` contiene un modello puro dello stato delle finestre, testabile senza grafica; a futuro verrà integrato nel compositore. Il compositore non esiste ancora: il solo runner Wayland è un *sample esterno* recuperabile tramite script (Smithay smallvil v0.7.0).

## Principi di progetto

- La shell non gira come root e non acquisisce privilegi generici.
- Preferire interfacce D-Bus standard in una revisione successiva, e concedere accessi tramite policy del sistema.
- Preferenze UI distinte dai setting hardware; `melania-config` non deve sostituire NetworkManager, BlueZ o PipeWire.
- Una finestra proprietaria del compositore non può essere controllata da qualunque client senza autorizzazione esplicita.
- Test di regressione prima di introdurre blur e animazioni.
- XDG, `xdg-desktop-portal`, Wayland, XWayland e GTK/Qt sono requisiti, non funzionalità oggi presenti.
