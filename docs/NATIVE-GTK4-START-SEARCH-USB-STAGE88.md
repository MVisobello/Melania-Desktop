# Stage 88 — focus tastiera reale e ricerca di Start (QEMU)

Questa è una fase **sperimentale, collegata alla Stage87**. Il compositore
native Smithay può assegnare il focus al menu Start reale solo dopo aver
validato un buffer wl_shm della superficie \`melania-start\`, controllato
la sua geometria nel display QEMU 1280×800 e importato i pixel nella scena.
Il semplice annuncio Wayland della superficie non è una prova sufficiente.

Nel guest disposable, l'input \`meta_l\` via \`usb-kbd\` apre Start tramite
libinput e il canale privato \`melania-control\`. Subito dopo la prova QMP
del terzo framebuffer, lo stesso dispositivo virtuale invia la lettera
\`a\` (pressione e rilascio). Solo l'evento reale
\`GtkEntry::changed\` può produrre il marker
\`MELANIA_QEMU_GTK_START_REAL_SEARCH_TYPED=true\`: niente
inserimento programmatico nel widget, screenshot precostruiti o log del
contenuto digitato. La verifica obbligatoria del terzo framebuffer di
Stage86–87 rimane attiva.

Il focus viene tolto quando la superficie Start viene spostata fuori
schermo o distrutta. Tutto il percorso rimane protetto dai flag di
opt-in \`MELANIA_NATIVE_GTK_START_FOCUS=YES_QEMU_VM\`,
\`MELANIA_NATIVE_REAL_GTK_START_INPUT=YES_QEMU_VM\` e
\`MELANIA_QEMU_GTK_SEARCH_INPUT=YES_DISPOSABLE_QEMU\`.

## Verifica

La prova di riuscita richiede **tutti** questi eventi nello stesso guest:

1. Scansione QMP DRM/KMS del terzo frame di Start, con pixel verificati.
2. Focus tastiera sulla superficie GTK Start reale.
3. Pressione e rilascio del tasto carattere dalla tastiera USB virtuale.
4. Segnale di modifica del campo di ricerca nella vera app GTK4.

La build, i test Python, il controllo di sicurezza dello script e la
verifica QEMU sono obbligatori prima del merge.

Questa fase non dimostra ancora editing interattivo generale, una
sessione nativa di login, gestione completa di input/puntatore o
compositing di frame illimitati. \`MELANIA_NATIVE_SESSION_READY=false\`.
