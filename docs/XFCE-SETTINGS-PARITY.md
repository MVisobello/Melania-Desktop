# Parità funzionale delle Impostazioni: XFCE 4.20 → Melania

**Obiettivo di prodotto:** includere in Melania tutte le preferenze utili
presenti in XFCE 4.20, senza adottarne la grafica e senza imporre
`xfconf`, `xfwm4`, `xfdesktop` o `xfce4-panel` al sistema Melania.

Fonti di riferimento: https://xfce.org/about/tour ,
https://xfce.org/download/changelogs/4.20 ,
https://xfce.readthedocs.io/latest/ .

L'interfaccia rimane GNOME-like con categorie a sinistra e contenuto a
destra. Il catalogo XFCE espone **24 aree**, comprese quelle per cui non
esiste ancora un backend Melania. Ogni pagina separa funzioni realmente
operative dalle opzioni non implementate: nessun interruttore fittizio.
La corrispondenza non va confusa con parità già ottenuta.

## Funzioni già collegate

| Famiglia XFCE | Disponibilità attuale Melania |
|---|---|
| Aspetto / icone | Tema Light/Dark/Aero e tema icone Melania |
| Gestore finestre | Snap, anteprime e gruppi nella barra (parziali) |
| Workspace | Numero 1–8, applicato al riavvio |
| Notifiche | Non disturbare |
| Accessibilità | Riduzione animazioni e contrasto parziale |
| Audio | Volume uscita PipeWire, se disponibile |
| Rete / Bluetooth | Informazioni NetworkManager e BlueZ |
| Display | Enumerazione monitor e sensori, lettura |
| Energia | Stato batterie e selezione esplicita di profili power-profiles-daemon, se disponibili |
| App preferite | Browser http/https e gestore email mailto tramite GIO |
| Sessione e avvio | Abilita/disabilita voci di autostart già presenti nell'XDG_CONFIG_HOME dell'utente, con salvataggio atomico e protezione symlink |
| Associazioni MIME | Editor GIO per tipi comuni o MIME inserito dall'utente |
| File manager | Melania Files gestisce già file, cartelle, Cestino e trasferimenti |

Le associazioni XDG/GIO sono **impostazioni dell'utente Linux**, quindi
valgono anche fuori dalla sessione Melania. Il cambio è possibile soltanto
con un clic esplicito su "Imposta come predefinita", con esito visibile
nella stessa pagina.

## Impostazioni del pannello — prima integrazione funzionale

La pagina `melania-settings xfce-panel` applica direttamente le
preferenze al vero pannello GTK4 layer-shell:

- Altezza da 52 a 96 pixel, con valore originale 52.
- Barra in alto oppure in basso: Start, Quick Settings, notifiche e
  calendario seguono il bordo scelto.
- Nascondimento automatico: il layer-shell rimane mappato lasciando una
  zona di richiamo di 3 pixel, così non perde il ruolo Wayland.
- Opacità 40–100%, rispettando i colori dei tre temi Aero/Light/Dark.
- Ripristino esplicito dei soli valori del pannello, senza cancellare
  icone, temi, preferiti o configurazioni delle finestre.

I controlli rileggono il file XDG al momento del salvataggio, conservano
le altre impostazioni e aggiornano la taskbar in esecuzione. Il valore
predefinito non modifica l'estetica già approvata. La funzione auto-hide
richiede comunque un collaudo manuale approfondito nelle sessioni Linux
reali, soprattutto per popup, dispositivi touchscreen e più monitor.

Pannelli multipli e plugin di terze parti restano da sviluppare.

## Scrivania e sfondo — seconda integrazione funzionale

La pagina `melania-settings xfce-desktop` permette di selezionare uno
sfondo locale PNG/JPEG/WebP/BMP (massimo 40 MB), cambiarne la modalità di
adattamento (ritaglio, intera immagine o riempimento) e nascondere le
icone. Il file viene scelto con il selettore GTK e validato prima del
salvataggio. Il desktop GTK4 layer-shell osserva le nuove preferenze e
aggiorna l'immagine e la visibilità delle icone **senza riavvio**.

Il ripristino riporta allo sfondo originale e mostra nuovamente le
icone, senza eliminare alcun file e senza cambiare lo sfondo del desktop
ospitante. L'override `MELANIA_WALLPAPER` resta prioritario per i
collaudi isolati. Le preferenze di monitor multipli, disposizione
manuale delle icone e rotazione ciclica restano da sviluppare.

## Parità ancora da sviluppare

- Desktop: sfondo per monitor/spazio, disposizione manuale delle icone, rotazione sfondi.
- Finestre: focus, decorazioni, scorciatoie personalizzate e opzioni compositor.
- Pannello: editor dell'ordine dei singoli elementi, plugin, supporto più pannelli e ulteriore collaudo auto-hide.
- Input: libinput mouse/touchpad, layout e ripetizione tastiera, mappature tasti.
- Display: modi, rotazioni, profili monitor con rollback.
- Energia: sleep, ibernazione, coperchio laptop, tasti e profili energia.
- Sessione: gestione delle voci autostart di sistema, creazione nuove voci, salvataggio, logout e ripristino applicazioni.
- Blocca schermo, salvaschermo, policy di inattività.
- Applicazioni: terminale, file manager predefinito, azioni speciali e MIME avanzati.
- Gestore file: tutte le preferenze persistenti di visualizzazione, colonne e supporti remoti.
- Rimovibili: auto-mount, autorun e associazioni per tipo di supporto.
- Stampanti, ICC/colord, XDG portal, varie azioni hardware.
- Editor di preferenze con validazione, import/export e ripristino ai valori iniziali.

## Criterio per dichiarare "stesse impostazioni di XFCE"

Ogni singola opzione XFCE dovrà avere: controllo GTK reale, backend che
applica la modifica nella sessione Melania o servizio Linux pertinente,
persistenza, indicazione degli effetti immediati o al riavvio, test di
regressione e verifica nella sessione Wayland. **Una pagina nel catalogo
senza backend non conta come funzione completata.**

Questa milestone costituisce **fase 1: copertura navigabile dell'inventario,
più applicazioni predefinite, MIME e autostart personale operativi**. Non dichiara completezza funzionale.
