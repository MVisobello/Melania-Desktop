//! Live, per-user panel preferences rendered in Melania's Settings UI.
//! No xfconf, no host panel changes and no fake controls.
use gtk::{self, prelude::*};
use gtk::{Box as GtkBox, Button, DropDown, Label, Orientation, Scale, SpinButton, Switch};
use melania_config::{self, Preferences};

fn label(txt:&str,style:&str)->Label {
    let l=Label::new(Some(txt));l.set_xalign(0.0);l.set_wrap(true);l.add_css_class(style);l
}
fn row(title:&str,description:&str,control:&impl IsA<gtk::Widget>)->GtkBox {
    let wrapper=GtkBox::new(Orientation::Horizontal,12);
    wrapper.add_css_class("settings-card");
    let text=GtkBox::new(Orientation::Vertical,4);
    text.set_hexpand(true);
    text.append(&label(title,"settings-row-title"));
    text.append(&label(description,"settings-row-description"));
    wrapper.append(&text);
    wrapper.append(control);
    wrapper
}
pub fn page()->GtkBox {
    let prefs=melania_config::load().unwrap_or_default();
    let body=GtkBox::new(Orientation::Vertical,12);
    body.add_css_class("settings-body");
    body.append(&label("Pannello e barra delle applicazioni","settings-heading"));
    body.append(&label(
        "Personalizza il pannello reale di Melania Desktop, come nelle preferenze di XFCE.",
        "settings-subtitle"));
    body.append(&label(
        "Le modifiche vengono applicate senza riavviare il pannello. Menu Start, orologio e ordine degli elementi restano invariati.",
        "settings-row-description"));

    let height=SpinButton::with_range(52.0,96.0,1.0);
    height.set_value(prefs.panel_height as f64);
    height.set_numeric(true);
    height.set_valign(gtk::Align::Center);
    body.append(&row("Altezza del pannello",
        "Da 52 a 96 pixel logici; il valore originale resta 52.",&height));

    let position=DropDown::from_strings(&["In basso","In alto"]);
    position.set_selected(if prefs.panel_position=="top" {1} else {0});
    position.set_valign(gtk::Align::Center);
    body.append(&row("Posizione dello schermo",
        "Sposta anche il menu Start, il calendario e i pannelli rapidi sullo stesso bordo.",
        &position));

    let auto_hide=Switch::builder().active(prefs.panel_auto_hide).build();
    auto_hide.set_valign(gtk::Align::Center);
    body.append(&row("Nascondi automaticamente",
        "Lascia una sottile zona di richiamo sul bordo; sposta il puntatore per riaprire la barra.",
        &auto_hide));

    let opacity=Scale::with_range(Orientation::Horizontal,40.0,100.0,1.0);
    opacity.set_value(prefs.panel_opacity as f64);
    opacity.set_draw_value(true);
    opacity.set_digits(0);
    opacity.set_hexpand(true);
    opacity.set_size_request(160,-1);
    opacity.set_valign(gtk::Align::Center);
    body.append(&row("Opacità del pannello",
        "Dal 40% al 100%. Il valore Aero iniziale è 94%; non modifica le finestre.",
        &opacity));

    let feedback=label("Le impostazioni riguardano soltanto Melania.",
        "settings-row-description");
    let action_row=GtkBox::new(Orientation::Horizontal,10);
    action_row.add_css_class("settings-action-row");
    let save=Button::with_label("Applica al pannello");
    save.add_css_class("settings-save");
    let reset=Button::with_label("Ripristina pannello");
    reset.add_css_class("settings-save");
    action_row.append(&save);
    action_row.append(&reset);
    body.append(&action_row);
    body.append(&feedback);
    let info=feedback.clone();
    let h=height.clone();
    let p=position.clone();
    let hide=auto_hide.clone();
    let alpha=opacity.clone();
    save.connect_clicked(move |_|{
        // Reload before updating to avoid overwriting other Settings values
        // that were changed in another Melania window meanwhile.
        let mut latest=match melania_config::load(){
            Ok(saved)=>saved,
            Err(error)=>{info.set_text(&format!("Impossibile leggere le preferenze: {error}"));return;}
        };
        latest.panel_height=h.value_as_int().clamp(52,96) as u16;
        latest.panel_position=if p.selected()==1 {"top"} else {"bottom"}.to_string();
        latest.panel_auto_hide=hide.is_active();
        latest.panel_opacity=alpha.value().round().clamp(40.0,100.0) as u8;
        match melania_config::save(&latest) {
            Ok(())=>info.set_text(
                "Preferenze salvate. Il pannello Melania applica automaticamente la nuova configurazione."),
            Err(error)=>info.set_text(&format!("Impossibile salvare: {error}")),
        }
    });

    let info=feedback.clone();
    reset.connect_clicked(move |_|{
        // Reset ONLY the panel fields; leave the rest of the user theme,
        // window grouping, workspaces and accessibility preferences intact.
        let mut latest=match melania_config::load() {
            Ok(saved)=>saved,
            Err(error)=>{info.set_text(&format!("Impossibile leggere le preferenze: {error}"));return;}
        };
        let defaults=Preferences::default();
        latest.panel_height=defaults.panel_height;
        latest.panel_position=defaults.panel_position;
        latest.panel_auto_hide=defaults.panel_auto_hide;
        latest.panel_opacity=defaults.panel_opacity;
        match melania_config::save(&latest) {
            Ok(())=>{
                height.set_value(f64::from(defaults.panel_height));
                position.set_selected(0);
                auto_hide.set_active(false);
                opacity.set_value(f64::from(defaults.panel_opacity));
                info.set_text("Pannello ripristinato alla disposizione originale di Melania.");
            }
            Err(error)=>info.set_text(&format!("Impossibile ripristinare: {error}")),
        }
    });
    body.append(&label("Organizzazione e applicazioni","settings-section-heading"));
    body.append(&label(
        "Per aggiungere o rimuovere applicazioni usa il clic destro sul menu Start o sulla barra. "
        ,"settings-row-description"));
    body.append(&label(
        "Per riordinare i preferiti trascinali. Pannelli multipli e plugin saranno aggiunti in fasi successive.",
        "settings-row-description"));
    body
}
