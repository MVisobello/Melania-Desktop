# Melania Desktop — sessione Wayland autonoma con labwc

Questa via realizza una sessione indipendente da Cinnamon usando il backend
Wayland/DRM/VT già funzionante di labwc/wlroots. Desktop, barra, Start,
file manager, impostazioni e notifiche restano applicazioni Melania.
Il compositore Smithay/Rust prosegue separatamente, senza esporre una sessione
nativa incompleta al display manager.

## Preparazione su Linux Mint 22 (Ubuntu 24.04)

1. Installare i pacchetti: sudo apt update && sudo apt install labwc wlrctl swaylock swayidle
2. Dal checkout compilare: cargo build --release -p melania-panel
   -p melania-desktop-surface -p melania-files -p melania-settings -p melania-notifyd -p melania-services
3. Se il sistema non trova libgtk4-layer-shell.so.0, impostare
   MELANIA_LAYER_LIB_DIR al percorso assoluto della libreria GTK4 privata
   già compilata per Melania (non modificare librerie di sistema).
4. Eseguire: bash scripts/melania-labwc-session --check
5. Eseguire: bash scripts/melania-labwc-session --stage
6. Dentro Cinnamon, provare: bash scripts/melania-labwc-session --nested-test

La prova annidata non sostituisce Cinnamon né acquisisce il DRM/VT principale.
Il comando --stage genera le voci nella directory target/melania-labwc-session
e copia la sola libreria privata indicata nell'area di staging.

## Aggiunta esplicita della sessione al login

SOLO dopo aver superato la prova annidata, eseguire:

sudo install -Dm644 target/melania-labwc-session/wayland-sessions/org.melaniadesktop.Labwc.desktop /usr/share/wayland-sessions/org.melaniadesktop.Labwc.desktop

Uscire da Cinnamon e scegliere Melania Desktop (Labwc) se il display manager
supporta le sessioni Wayland. La sessione Cinnamon preesistente resta intatta.
Per tornare alla condizione originale, selezionare Cinnamon dal login;
per rimuovere la nuova voce usare:

sudo rm /usr/share/wayland-sessions/org.melaniadesktop.Labwc.desktop

## Tasti e funzionamento

- Super+Spazio: Start; Super+E: Melania Files; Super+I: Impostazioni
- Alt+Tab: cambia finestra; Alt+F4: chiude; Super+Invio: terminale
- Super+L: blocco autenticato con swaylock (solo sessione standalone; nella prova annidata è rifiutato)
- La barra riceve le finestre vere da wlrctl tramite un socket IPC privato.
- Disconnetti termina soltanto il processo labwc di questa sessione.
- La configurazione -C integration/labwc non modifica ~/.config/labwc.
- Il frontend non avvia il compositore sperimentale DRM/KMS di Melania.

## Limitazioni e verifica prima di sostituire Cinnamon

Il launcher è una prima implementazione: la sessione completa deve ancora
essere avviata e collaudata su un Linux Mint reale prima di dichiararla pronta.
Il bridge wlrctl non offre anteprime delle finestre, stato attivo completo
o workspace multipli; le finestre con identico app-id/titolo non possono
essere selezionate univocamente e vengono rifiutate in sicurezza.
Il tasto Super da solo non è ancora configurato per Start.
Decorazioni e gestione finestre sono temporaneamente quelle di labwc.

Servono ancora prove di browser/XWayland, multi-monitor, sospensione e resume,
hotplug, schermata di blocco, portali e condivisione schermo, notifiche,
audio, stampa e ritorno a Cinnamon senza perdita di dati.
Non impostare Melania come sessione predefinita finché queste verifiche
non hanno avuto esito positivo.

## Blocco schermo e sospensione: prima integrazione Rust

La sessione vera richiede i pacchetti `swaylock` e `swayidle`.
Il programma Rust `melania-sessionctl` verifica di essere dentro una
sessione Wayland labwc indipendente: rifiuta Cinnamon, la prova annidata,
i test e le sessioni senza permesso esplicito. `swaylock -f` registra
un autentico blocco tramite ext-session-lock/PAM; **non** è un overlay GTK.

Il supervisore lancia `melania-sessionctl idle` soltanto quando non è
in modalità nested. Il servizio usa `swayidle -w`: dopo 300 secondi
di inattività o prima della sospensione domanda l'autenticazione reale;
il supervisore termina la sessione se il servizio critico muore.
Le scelte del timeout e la politica di gestione del coperchio
devono ancora diventare configurabili da Impostazioni.

Nella prima installazione verificare fisicamente Ctrl+Alt+F2,
Super+L, sospensione/riattivazione e autenticazione prima di usare
Melania come desktop quotidiano. La compilazione e il collaudo
Wayland annidato NON costituiscono una prova della sicurezza del
blocco sul computer reale.

Se manca swaylock/swayidle, `--run` blocca il login per non avviare
una sessione priva di protezione, mentre `--nested-test` continua
a funzionare senza eseguire azioni sul sistema ospitante.
