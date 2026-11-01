//! XFCE-style desktop wallpaper and icon visibility, applied to Melania's
//! own GTK4 background layer. The host desktop wallpaper is never altered.
use gtk::{self, prelude::*};
use gtk::{Box as GtkBox, Button, DropDown, Entry, Label, Orientation, Switch};
use melania_config::{self,Preferences};
use std::{fs,path::Path};

fn label(value:&str,class:&str)->Label {
    let l=Label::new(Some(value));l.set_xalign(0.0);l.set_wrap(true);
    l.add_css_class(class);l
}
fn valid_wallpaper(path:&str)->bool {
    if path.is_empty(){return true;}
    if path.len()>1024 || path.chars().any(char::is_control) {return false;}
    let file=Path::new(path);
    if !file.is_absolute() || !fs::metadata(file).is_ok_and(|m|m.is_file()&&m.len()<=40_000_000){
        return false;
    }
    file.extension().and_then(|ext|ext.to_str()).is_some_and(|ext|
        matches!(ext.to_ascii_lowercase().as_str(),"png"|"jpg"|"jpeg"|"webp"|"bmp"))
}
pub fn page()->GtkBox {
    let prefs=melania_config::load().unwrap_or_default();
    let body=GtkBox::new(Orientation::Vertical,12);
    body.add_css_class("settings-body");
    body.append(&label("Scrivania e sfondo","settings-heading"));
    let path=Entry::new();
    path.set_text(&prefs.desktop_wallpaper);
    path.set_placeholder_text(Some("Vuoto = sfondo originale di Melania"));
    path.set_hexpand(true);
    let browse=Button::with_label("Sfoglia…");
    browse.add_css_class("settings-save");
    let chooser_row=GtkBox::new(Orientation::Horizontal,8);
    chooser_row.append(&path);
    chooser_row.append(&browse);
    body.append(&label("Immagine di sfondo","settings-section-heading"));
    body.append(&chooser_row);
    let path_for_browse=path.clone();
    browse.connect_clicked(move |_|{
        let chooser=gtk::FileChooserNative::new(
            Some("Seleziona sfondo Melania"),
            None::<&gtk::Window>,
            gtk::FileChooserAction::Open,
            Some("Seleziona"),Some("Annulla")
        );
        let entry=path_for_browse.clone();
        chooser.connect_response(move |dialog,response|{
            if response==gtk::ResponseType::Accept {
                if let Some(file)=dialog.file().and_then(|file|file.path()) {
                    entry.set_text(&file.to_string_lossy());
                }
            }
        });
        chooser.show();
    });

    body.append(&label("Adattamento dell'immagine","settings-section-heading"));
    let fit=DropDown::from_strings(&[
        "Riempi lo schermo (ritaglia)",
        "Adatta interamente (bordi possibili)",
        "Allunga allo schermo"
    ]);
    fit.set_selected(match prefs.desktop_wallpaper_fit.as_str(){
        "contain"=>1,"fill"=>2,_=>0
    });
    body.append(&fit);

    let row=GtkBox::new(Orientation::Horizontal,12);
    row.add_css_class("settings-card");
    let words=GtkBox::new(Orientation::Vertical,4);
    words.set_hexpand(true);
    words.append(&label("Mostra icone sulla scrivania","settings-row-title"));
    words.append(&label(
        "Nasconde o mostra i file sulla superficie desktop; non elimina nessun file.",
        "settings-row-description"));
    row.append(&words);
    let icons=Switch::builder().active(prefs.desktop_show_icons).build();
    icons.set_valign(gtk::Align::Center);
    row.append(&icons);
    body.append(&row);

    let actions=GtkBox::new(Orientation::Horizontal,8);
    let apply=Button::with_label("Applica alla scrivania");
    apply.add_css_class("settings-save");
    let reset=Button::with_label("Ripristina sfondo e icone");
    reset.add_css_class("settings-save");
    actions.append(&apply);
    actions.append(&reset);
    body.append(&actions);
    let feedback=label("",
        "settings-row-description");
    body.append(&feedback);

    let input=path.clone();
    let mode=fit.clone();
    let visibility=icons.clone();
    let status=feedback.clone();
    apply.connect_clicked(move |_|{
        let image=input.text().trim().to_string();
        if !valid_wallpaper(&image){
            status.set_text("File non valido: seleziona un'immagine locale supportata, fino a 40 MB.");
            return;
        }
        let mut latest=match melania_config::load(){
            Ok(config)=>config,
            Err(error)=>{status.set_text(&format!("Lettura preferenze non riuscita: {error}"));return;}
        };
        latest.desktop_wallpaper=image;
        latest.desktop_wallpaper_fit=match mode.selected(){
            1=>"contain",2=>"fill",_=>"cover"
        }.to_owned();
        latest.desktop_show_icons=visibility.is_active();
        match melania_config::save(&latest){
            Ok(())=>status.set_text(
                "Preferenze salvate. Lo sfondo e le icone verranno aggiornati sulla scrivania Melania."),
            Err(error)=>status.set_text(&format!("Impossibile salvare: {error}")),
        }
    });

    let status=feedback.clone();
    reset.connect_clicked(move |_|{
        let mut latest=match melania_config::load(){
            Ok(config)=>config,
            Err(error)=>{status.set_text(&format!("Lettura preferenze non riuscita: {error}"));return;}
        };
        let defaults=Preferences::default();
        latest.desktop_wallpaper=defaults.desktop_wallpaper;
        latest.desktop_wallpaper_fit=defaults.desktop_wallpaper_fit;
        latest.desktop_show_icons=defaults.desktop_show_icons;
        match melania_config::save(&latest){
            Ok(())=>{
                path.set_text("");
                fit.set_selected(0);
                icons.set_active(true);
                status.set_text("Sfondo originale e icone ripristinati, senza modificare i file.");
            }
            Err(error)=>status.set_text(&format!("Impossibile ripristinare: {error}")),
        }
    });
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_missing_remote_or_non_image_wallpapers(){
        assert!(valid_wallpaper(""));
        assert!(!valid_wallpaper("https://site.example/wallpaper.png"));
        assert!(!valid_wallpaper("../../image.png"));
        assert!(!valid_wallpaper("/tmp/nonexistent-file-melania-wallpaper.png"));
        assert!(!valid_wallpaper("/etc/passwd"));
    }
}
