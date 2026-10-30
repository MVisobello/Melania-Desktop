# Melania Desktop — import GitHub, v0.38

Questo albero di lavoro contiene i sorgenti Rust, la pipeline `.github/workflows/linux-verify.yml`, le risorse Aero e il laboratorio grafico X11.

## Stato reale

Sorgenti **pre-alpha**. Non ancora compilati e verificati con Cargo/GTK/Smithay. La pipeline e' predisposta, ma il suo risultato non e' garantito finche' GitHub Actions non viene eseguito.

## Import

Il repository GitHub di destinazione deve essere **privato** e idealmente vuoto, con nome `Melania-Desktop`.

Dopo aver consentito a ChatGPT GitHub App l'accesso al nuovo repository, sara' possibile caricare i sorgenti dal collegamento GitHub. La connessione del solo profilo OAuth non basta: l'app deve avere accesso al repository.

Per caricare da Linux con un client Git autenticato:

    git remote add origin https://github.com/ACCOUNT/Melania-Desktop.git
    git push -u origin main

Sostituire ACCOUNT con il proprio login. Il comando richiede autenticazione GitHub e non e' stato eseguito qui.

Una volta pubblicato il branch `main`, il workflow di GitHub Actions parte sul push.
