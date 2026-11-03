# Melania Files — diagnosticare i rallentamenti su Linux Mint 22 / VirtualBox

Questa procedura è **facoltativa** e non cambia la configurazione di Mint,
il sistema host, i file personali o il compositore. Riferimento: issue #123.

## 1. Raccogliere misure in una sessione annidata

Nel checkout GitHub di Melania, da un desktop Linux già aperto:

```bash
MELANIA_PROFILE_FILES=1 bash scripts/melania-session --nested 2>&1 | tee /tmp/melania-files-session.log
```

Se si usa il bundle portatile estratto per Mint 22, lanciare invece
`MELANIA_PROFILE_FILES=1 ./run.sh --software` dalla directory prevista
dal bundle, redirigendo standard output ed errori allo stesso modo.
I log possono contenere percorsi locali: **rimuovere i dati privati prima di condividerli**.

Ripetere nella stessa sessione le medesime operazioni: aprire Home,
una cartella vuota, una cartella con molti file; passare da lista a griglia;
cercare un nome, cambiare cartella, aprire e chiudere schede. Per un confronto,
eseguire la stessa sequenza sulla vecchia versione e su quella nuova.

## 2. Leggere le misure

Dal checkout del repository:

```bash
python3 scripts/summarize-files-performance.py /tmp/melania-files-session.log
```

Per mettere a fianco due esecuzioni:

```bash
python3 scripts/summarize-files-performance.py vecchia.log nuova.log
```

Il programma legge solo le righe `MELANIA_FILES_*`, stampa
**numero di campioni, mediana (p50), p95 e massimo**, in millisecondi, con
metodo *nearest-rank*. Non modifica i file di input.

- `FIRST_WINDOW_MS` / `FIRST_TAB_MS`: disponibilità iniziale di finestra e scheda.
- `SCAN_MS`: tempo del worker GIO.
- `CLEAR_MS`: rimozione sincrona delle vecchie righe dalla FlowBox GTK.
- `RENDER_MS`: costruzione delle nuove righe GTK a piccoli blocchi.
- `THUMBNAILS_MS`: durata del worker miniature.
- `UI_STALL_MS`: ritardi dell'event loop **solo quando almeno 250 ms**;
  l'assenza di campioni non dimostra che tutti i frame siano fluidi.

## 3. Interpretazione

I campioni non sono intercambiabili tra cartelle differenti: usare la stessa
quantità e tipologia di file, stesso renderer (Cairo/llvmpipe o accelerato),
stessa risoluzione e stessa modalità lista/griglia. Il p95 è poco stabile
quando i campioni sono pochi. Non dedurre la causa da un unico numero:
un `SCAN_MS` basso con `CLEAR_MS` o `RENDER_MS` alti suggerisce lavoro GTK;
`SCAN_MS` alto richiede analisi I/O/GVfs. I test CI non sostituiscono
un vero confronto di frame time nella VM.

La PR #146 aggiunge la misura `CLEAR_MS` e interrompe il lavoro grafico
ormai inutile delle schede chiuse, senza modificare il loro aspetto.
