# Melania Files — miniature locali asincrone e limitate

Questa modifica sostituisce la lettura sincrona `Picture::for_filename`
nelle celle della **vista a icone locale** con miniature PNG/JPEG/WebP
calcolate su un worker Rust, utilizzando il backend GIO già convalidato
per le anteprime F3. Le altre tipologie conservano l'icona del tema.

## Limiti e sicurezza

- Fino a **48 miniature per cartella** per ogni render; le immagini successive
  continuano a mostrare l'icona normale. Non è ancora virtualizzazione della lista.
- Un solo worker attivo e una richiesta pendente per scheda: nuove navigazioni,
  modifiche al filtro e F5 cancellano le miniature non più pertinenti.
- I risultati vengono applicati a GTK solo se appartengono alla generazione
  di elenco corrente. Le operazioni GIO e la decodifica restano fuori dal
  thread grafico. Il canale sincrono contiene un solo risultato per volta.
- Il decoder preesistente rifiuta i symlink e i file troppo grandi, verifica
  davvero PNG/JPEG/WebP, limita l'input a 6 MiB, la geometria a 2048×2048
  e 4 milioni di pixel. La miniatura finale è **al massimo 112×84 RGBA**.
- Nessuna scrittura sui file originali, download permanente o decoder esterno.

## Verifica

I test Rust verificano il ridimensionamento 320×240 a 112×84, la
conservazione dei pixel 4×3, l'identità del colore e l'annullamento
prima dell'accesso a un provider remoto. Sono necessari `cargo test
-p melania-files --bin melania-files`, il build Linux GTK e lo smoke
Wayland su GitHub Actions.

**Limitazioni:** la vista elenco mantiene soltanto le icone; i file PDF,
video e GIF non ottengono miniature, l'anteprima laterale locale continua
a utilizzare il percorso GTK esistente, la cache e la virtualizzazione
di directory molto grandi sono da realizzare. La parità con Nautilus
non è completa e non viene dichiarata.
