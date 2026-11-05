# Melania — scorciatoie configurabili nella sessione labwc

La pagina **Impostazioni → Scorciatoie** non è più un elenco statico. Consente di scegliere una combinazione distinta per **Start, Melania Files, Impostazioni, blocco schermo e terminale**.

## Backend
- `melania-inputctl shortcuts-show` legge la configurazione personale `$XDG_CONFIG_HOME/melania/shortcut-prefs` o restituisce i valori predefiniti.
- `melania-inputctl shortcuts-apply <start> <files> <settings> <lock> <terminal>` valida cinque combinazioni **univoche** in una lista chiusa di 12 scelte e applica la modifica alla sessione principale labwc.
- La configurazione di labwc è sempre generata dal template versionato. I soli attributi XML dei cinque keybind noti vengono sostituiti con valori validati; i comandi associati restano immutati.
- Il salvataggio è isolato in `$XDG_CONFIG_HOME/melania`; non modifica il laboratorio XFCE/Cinnamon o il labwc dell'utente. Il comando applicativo è disabilitato in una preview nested.
- I controlli sono esplicitamente disabilitati se manca la sessione principale o il backend disponibile.

Le opzioni di mouse/touchpad e scorciatoie vengono combinate durante ogni `prepare`, in modo che salvare una non ripristini accidentalmente l'altra.

## Limiti verso XFCE
Le 5 combinazioni sono una prima copertura reale, non un editor universale di scorciatoie. Gestione focus, scorciatoie arbitrarie, tasti multimediali e modifica dei binding Alt+Tab/Alt+F4 non sono ancora implementati. Il compositore nativo Melania non supporta ancora questo editor e richiede una propria integrazione.

## Test
Unit Rust: valori ammessi e duplicati, parsing, round-trip, rendering esatto del template e conservazione delle impostazioni input. Contratti Python: comando backend, GTK e session guard. Prima dell'integrazione serve CI verde e collaudo nella sessione labwc.
