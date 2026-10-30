# Melania Desktop — impostazioni GNOME-like e controlli finestre classici

Aggiornamento del design della sessione Melania con focus su usabilità e
coerenza. Questa revisione **non modifica le preferenze di GNOME o del desktop
Linux ospitante**.

## Impostazioni

La finestra `melania-settings` resta implementata in GTK4, ma riorganizza
la UI ispirandosi alla chiarezza di GNOME Settings:

- Sidebar sinistra da 258 px con simboli freedesktop, selezione persistente
  e ricerca nelle categorie. Navigazione sincronizzata anche quando una
  categoria viene richiesta dal pannello tramite linea di comando.
- Pannello contenuti a fondo chiaro neutro, titoli principali, gruppi,
  preferenze in righe bianche con descrizioni secondarie e interruttori.
- Temi Light e Dark sobri, contrasto elevato e focus visibile.
- Stesse opzioni persistenti: Aero Glass, animazioni, tema scuro,
  pacchetto icone Fluent/Aero, intensità, snap, anteprime, raggruppamento,
  spazi di lavoro, accessibilità e informazioni.
- Gli interruttori e i salvataggi usano `melania_config`; i servizi di rete
  non vengono presentati come configurabili se l'interfaccia non li implementa.

Non si tratta di una copia del sorgente GNOME e non richiede libadwaita:
si mantengono le dipendenze GTK4 già presenti.

## Pulsanti in stile Windows 7

Solo nelle finestre Melania con HeaderBar nativa (Files e Impostazioni):

- tre pulsanti compatti affiancati, riflessi azzurri
- chiusura rossa con hover più intenso
- angoli inferiori arrotondati all'esterno del gruppo
- operazioni GTK native di minimizza/massimizza/chiudi, non pulsanti finti

Le decorazioni delle app esterne richiedono la loro collaborazione o supporto
specifico del compositore e non sono modificate globalmente da questo CSS.

## Icona Ufficio

`tools/melania_office_artwork.py` fornisce un SVG originale indipendente.
Tre oggetti (testo, tabella, presentazione) indicano l'insieme dei software
di produttività: **nessun marchio o silhouette Microsoft Office**.
`tools/melania_fluent_reference.py` e `tools/melania_fluent_artwork.py`
usano questo disegno per i nomi Office; le altre icone del concept Fluent
approvato rimangono invariate, incluso il globo del browser.

## Test

```bash
python3 tests/test_settings_fluent_chrome.py
python3 tools/test_melania_fluent_reference.py
python3 tools/test_melania_fluent_artwork.py
cargo check -p melania-settings -p melania-files
```

La pipeline Linux su GitHub genera anche uno screenshot della finestra
Impostazioni **realmente avviata sotto Xvfb** e lo pubblica come artifact
`reports/melania-settings-gnome-style.png`; non è un rendering AI.
Il test visivo non sostituisce il controllo su una sessione Wayland reale.
