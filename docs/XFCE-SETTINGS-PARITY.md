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

## Parità ancora da sviluppare

- Desktop: sfondo per monitor/spazio, posizionamento icone, menu desktop.
- Finestre: focus, decorazioni, scorciatoie personalizzate e opzioni compositor.
- Pannello: dimensioni, posizione, auto-hide, elementi, plugin e più pannelli.
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
