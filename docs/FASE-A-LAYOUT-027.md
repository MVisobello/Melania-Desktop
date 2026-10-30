# Fase A — Correzioni Shell / Melania Files (v0.27)

## Modifiche ai sorgenti GTK4

In `melania-files/src/main.rs` la vecchia `GtkBox` a tre figli in sequenza diventa una coppia di `gtk::Paned` annidati. Separazione tra barra laterale, `ScrolledWindow` centrale e inspector. Il panello destro ha uno scroller indipendente; il `FlowBox` centrale si adatta alla larghezza. I controlli di filtro e preferiti hanno una riga secondaria, liberando la barra del percorso.

In `melania-files/assets/style.css` sono state aggiunte classi per separatori, sfondo del pannello destro, tile compatti, dimensioni minime e hover. In `melania-panel/assets/panel.css` sono state allineate le densità e le altezze di taskbar, pulsanti e pannelli.

## Convalida UX Lab

10 test dell'interazione e geometria effettuati in una sessione X11 virtuale. Nessuna sovrapposizione fra box della griglia, inspector e sidebar per la dimensione usata; conteggio colonne responsivo verificato. I test esistenti (14 assertion del laboratorio) continuano a passare.

## Limiti

Questa patch non dimostra la compilabilità del codice Rust/GTK4: nell'ambiente di lavoro mancano Cargo e le librerie di sviluppo. Gli screenshot Tk non dimostrano il rendering Wayland né il blur GPU. Le immagini del catalogo fotografico sono campioni del concept fornito dall'utente, usati solo per l'anteprima grafica.
