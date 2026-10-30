# Importazione completa Melania Desktop v0.38

Il repository e' privato. L'accesso di scrittura e' stato verificato tramite il connettore GitHub.

## Un solo caricamento (senza scompattare)

1. Scarica `Melania-Desktop-v0.38-Git-Initial.bundle` dal messaggio ChatGPT. Dimensione: circa 14 MB.
2. Apri `https://github.com/MVisobello/Melania-Desktop/upload/main`.
3. Seleziona dal telefono/computer il file **con lo stesso nome**, senza rinominarlo e senza estrarlo.
4. In basso seleziona `Commit directly to the main branch` e conferma.

Questo attiva il workflow **Import Melania source bundle** in GitHub Actions. Il workflow:
- verifica SHA-256 esatto `00f17cddaced84e6b81d460357b4b8ba4d9f3af101931594c67121db0ea62227`;
- verifica che il file sia un bundle Git valido, con il branch `main`;
- ricostruisce tutti i file tracciati del progetto sorgente (7.005 nel backup originario);
- salva i sorgenti direttamente sul branch `main`;
- elimina dal branch il bundle di importazione e lo stesso workflow temporaneo.

Dopo il completamento, il workflow standard `.github/workflows/linux-verify.yml` diventa disponibile. La compilazione Rust/GTK/Smithay **non e' ancora garantita**: bisogna eseguire e leggere i log della CI.

## Sicurezza

Il file viene caricato solo nel tuo repository privato. Non condividere pubblicamente il bundle se vuoi tenere il codice privato.

## Se il telefono non mostra `Add file`

Apri la pagina GitHub dal browser e abilita **Sito desktop** nel menu del browser; puoi anche usare direttamente il link `/upload/main` sopra.

## Stato

Bootstrap su GitHub pronto; il caricamento del bundle e l'importazione non risultano ancora avvenuti finche' non vedi il commit di import sulla branch main.
