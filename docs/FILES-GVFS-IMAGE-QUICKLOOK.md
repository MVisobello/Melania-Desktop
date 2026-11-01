# Melania Files — Anteprima immagini PNG/JPEG/WebP

La finestra **Anteprima** (tasto **F3**) distingue in base al tipo MIME visualizzato tra file di testo e immagini. Le immagini PNG, JPEG e WebP vengono lette dal provider GIO/GVfs senza importarle permanentemente su disco. Tutto il percorso (rete e decodifica) viene eseguito fuori dal thread GTK; nella GUI arriva soltanto una texture RGBA ridimensionata.

## Limiti di sicurezza

- File regolari soltanto, senza seguire collegamenti simbolici e senza eseguire nulla; i dati reali devono corrispondere al formato MIME dichiarato.
- **6 MiB** massimo per il file compresso, con un byte addizionale di controllo. I file più grandi vengono rifiutati, non scaricati completamente.
- **2048 pixel per dimensione**, **4 milioni di pixel totali** e limite di allocazione del decoder configurato a 40 MiB. La libreria segnala che quest'ultimo limite è best-effort; non va considerato un isolamento di processo.
- Immagine decodificata in worker Rust (dipendenza `image`, con sole feature PNG/JPEG/WebP); immagine finale ridotta entro 1024×720 prima di passare al thread GTK.
- Chiusura della finestra segnala l'annullamento al provider GVfs. Durante una decodifica CPU già avviata, l'annullamento viene verificato immediatamente dopo il decode; non è garantita l'interruzione preemptive.
- Nessun file originale viene modificato, non vengono creati file temporanei o trasferite immagini a servizi esterni.

## Verifiche e limiti restanti

I test Rust coprono PNG reali, contenuto dei pixel, dimensioni e formati non validi. Il test SFTP usa un server OpenSSH locale autenticato del runner e un PNG generato nello stesso test, controllando dimensioni e pixel attraverso il backend GIO. Linux/GTK e Wayland devono risultare verdi prima del merge.

**Non è un decoder sandboxato**: per file non attendibili sarebbe preferibile un processo isolato per la decodifica. Mancano ancora miniature per file di una directory, PDF/video, caching thumbnails e test di interoperabilità SMB/WebDAV. L'obiettivo di parità con Nautilus resta nella issue #78.
