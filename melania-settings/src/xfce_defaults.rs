//! XDG preferred applications and MIME editor for Melania Settings.
//!
//! These are REAL user-level MIME associations through GIO. GIO also shares
//! them with other XDG desktop environments; never claim they are Melania-only.
//! No shell, arbitrary Exec lines or privileged command is run.
use gtk::{self, gio, prelude::*};
use gtk::{Box as GtkBox, Button, DropDown, Entry, Label, Orientation};
use std::collections::HashSet;

fn message(value:&str,class:&str)->Label {
    let label=Label::new(Some(value));
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.add_css_class(class);
    label
}

pub fn is_valid_type(mime:&str)->bool {
    let Some((group,subtype))=mime.split_once('/') else {return false;};
    if mime.len()>160 || group.is_empty() || subtype.is_empty(){return false;}
    [group,subtype].iter().all(|part|part.bytes().all(|c|
        c.is_ascii_alphanumeric() || matches!(c,b'.'|b'_'|b'-'|b'+')))
}

/// Each button modifies the *actual* XDG MIME association after an explicit
/// click and shows an error if the backend declines it.
fn association_row(label:&str,description:&str,mime:&str,also_https:bool)->GtkBox {
    let line=GtkBox::new(Orientation::Vertical,6);
    line.add_css_class("settings-card");
    line.append(&message(label,"settings-row-title"));
    line.append(&message(description,"settings-row-description"));
    if !is_valid_type(mime){
        line.append(&message("Tipo MIME non valido.","settings-row-description"));
        return line;
    }
    let mut applications=gio::AppInfo::all_for_type(mime);
    applications.sort_by_key(|app|app.name().to_string().to_lowercase());
    let mut seen=HashSet::new();
    applications.retain(|app|{
        let id=app.id().map(|i|i.to_string()).unwrap_or_else(||app.name().to_string());
        seen.insert(id)
    });
    if applications.is_empty(){
        line.append(&message("Nessuna applicazione registrata per questo tipo MIME.",
            "settings-row-description"));
        return line;
    }
    let labels=applications.iter().map(|app|app.name().to_string()).collect::<Vec<_>>();
    let strings=labels.iter().map(String::as_str).collect::<Vec<_>>();
    let chooser=DropDown::from_strings(&strings);
    chooser.set_hexpand(true);
    let existing=gio::AppInfo::default_for_type(mime,mime.starts_with("x-scheme-handler/"));
    if let Some(index)=applications.iter().position(|app|
        app.id().is_some() && app.id()==existing.as_ref().and_then(|x|x.id())) {
        chooser.set_selected(index as u32);
    }
    let controls=GtkBox::new(Orientation::Horizontal,8);
    let save=Button::with_label("Imposta come predefinita");
    save.add_css_class("settings-save");
    controls.append(&chooser);
    controls.append(&save);
    line.append(&controls);
    let status=message("","settings-row-description");
    line.append(&status);
    let kind=mime.to_string();
    let name=label.to_string();
    save.connect_clicked(move |_|{
        let Some(app)=applications.get(chooser.selected() as usize) else {return;};
        let result=app.set_as_default_for_type(&kind);
        let result=if result.is_ok() && also_https {
            app.set_as_default_for_type("x-scheme-handler/https")
        } else {result};
        match result {
            Ok(())=>status.set_text(&format!(
                "{name}: {} è l'app predefinita. Modifica XDG valida anche per altri desktop.",
                app.name()
            )),
            Err(error)=>status.set_text(&format!(
                "Impossibile aggiornare l'associazione XDG: {error}"
            )),
        }
    });
    line
}

pub fn preferred_apps_page() -> GtkBox {
    let body=GtkBox::new(Orientation::Vertical,12);
    body.add_css_class("settings-body");
    body.append(&message("Applicazioni preferite","settings-heading"));
    body.append(&message(
        "Come in XFCE: scegli le applicazioni predefinite usando associazioni reali del sistema. "
        ,"settings-subtitle"));
    body.append(&message(
        "Le preferenze XDG sono condivise con le altre sessioni Linux dell'utente. "
        ,"settings-row-description"));
    body.append(&association_row("Browser web",
        "Per i collegamenti http e https.","x-scheme-handler/http",true));
    body.append(&association_row("Posta elettronica",
        "Per i collegamenti mailto.","x-scheme-handler/mailto",false));
    body.append(&message("Gestore file e terminale",
        "settings-section-heading"));
    body.append(&message(
        "La scelta del file manager e del terminale predefiniti richiede l'integrazione dei launcher "
        ,"settings-row-description"));
    body.append(&message(
        "della sessione Melania. Non viene cambiata di nascosto l'applicazione del desktop ospitante.",
        "settings-row-description"));
    body
}

pub fn mime_page() -> GtkBox {
    let body=GtkBox::new(Orientation::Vertical,12);
    body.add_css_class("settings-body");
    body.append(&message("Associazioni MIME","settings-heading"));
    body.append(&message(
        "Seleziona l'applicazione predefinita per un tipo di file o protocollo. "
        ,"settings-subtitle"));
    body.append(&message(
        "La modifica avviene mediante GIO/XDG, è persistente ed è condivisa con le altre sessioni dell'utente.",
        "settings-row-description"));
    for (title,mime) in [
        ("Testi semplici","text/plain"),("Documenti PDF","application/pdf"),
        ("Immagini PNG","image/png"),("Immagini JPEG","image/jpeg"),
        ("Video MP4","video/mp4"),("Audio MP3","audio/mpeg"),
    ] {
        body.append(&association_row(title,mime,mime,false));
    }
    body.append(&message("Altri tipi di file","settings-section-heading"));
    let entry=Entry::new();
    entry.set_placeholder_text(Some("Esempio: application/zip"));
    entry.set_max_length(160);
    let open=Button::with_label("Apri tipo MIME");
    open.add_css_class("settings-save");
    let controls=GtkBox::new(Orientation::Horizontal,8);
    controls.append(&entry);
    controls.append(&open);
    body.append(&controls);
    let dynamic=GtkBox::new(Orientation::Vertical,8);
    body.append(&dynamic);
    open.connect_clicked(move |_|{
        while let Some(child)=dynamic.first_child(){dynamic.remove(&child);}
        let value=entry.text();
        let mime=value.trim();
        if !is_valid_type(mime){
            dynamic.append(&message(
                "Inserisci un tipo MIME valido, ad esempio application/zip.",
                "settings-row-description"));
            return;
        }
        dynamic.append(&association_row(mime,mime,mime,false));
    });
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_invalid_or_command_like_mime_types() {
        for name in ["", "text", "../exec", "text/plain\nExec=bad", "x-scheme-handler/../bad",
                     "image/png;rm", "image//png", "/pdf"] {
            assert!(!is_valid_type(name),"{name}");
        }
        for name in ["text/plain","image/png","application/vnd.oasis.opendocument.text",
                     "x-scheme-handler/http"]{
            assert!(is_valid_type(name),"{name}");
        }
    }
}
