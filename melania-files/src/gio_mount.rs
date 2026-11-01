//! Interactive GVfs mount lifecycle, executed only from user actions.
//! All GTK and GIO async operations run on the GTK main context: GTK's
//! MountOperation shows system-native auth, questions and busy-file dialogs.
//! Credentials are never read, stored or logged by Melania Files.
//! No FORCE unmount/eject is ever requested.
use gtk::{gio,glib};
use gtk::prelude::*;
use super::gio_backend;

fn error_text(error: glib::Error, label: &str) -> String {
    // Do not echo URI or credentials in errors.
    if error.matches(gio::IOErrorEnum::Cancelled) {
        return format!("{label} annullato.");
    }
    format!("{label} non riuscito: {error}")
}

/// A cancellable interactive connection. Call ONLY after the user presses
/// Connect, never automatically as part of a directory scan.
pub fn connect_uri<F>(uri: &str, parent: Option<&gtk::Window>, done: F)
    -> Result<gio::Cancellable,String>
where F: FnOnce(Result<(),String>) + 'static {
    gio_backend::validate_uri(uri)?;
    let scheme=uri.split_once("://").map(|(scheme,_)|scheme).unwrap_or("");
    if matches!(scheme,"file"|"network") {
        return Err("Questa posizione non richiede una connessione autenticata.".to_string());
    }
    let cancellable=gio::Cancellable::new();
    let op=gtk::MountOperation::new(parent);
    // GTK's own operation handles user interaction and remains referenced
    // by GIO until completion. Never pull its password into app state.
    gio::File::for_uri(uri).mount_enclosing_volume(
        gio::MountMountFlags::NONE, Some(&op), Some(&cancellable),
        move |result| {
            let result=match result {
                Ok(())=>Ok(()),
                Err(e) if e.matches(gio::IOErrorEnum::AlreadyMounted)=>Ok(()),
                Err(e)=>Err(error_text(e,"Connessione")),
            };
            done(result);
        }
    );
    Ok(cancellable)
}

/// Mount an attached but unmounted volume through the same native credential
/// prompt. No automatic mounts and no shell/udisks subprocesses.
pub fn mount_volume<F>(volume: &gio::Volume, parent: Option<&gtk::Window>, done:F)
    -> gio::Cancellable
where F: FnOnce(Result<(),String>) + 'static {
    let op=gtk::MountOperation::new(parent);
    let cancellable=gio::Cancellable::new();
    volume.mount(gio::MountMountFlags::NONE,Some(&op),Some(&cancellable),
        move |result| done(result.map_err(|e|error_text(e,"Montaggio"))));
    cancellable
}

/// The caller has shown a dedicated confirmation step. FORCE is forbidden:
/// busy volumes must produce an error/dialog rather than dropping writes.
pub fn unmount<F>(mount: &gio::Mount, parent: Option<&gtk::Window>, done:F)
    -> Result<gio::Cancellable,String>
where F: FnOnce(Result<(),String>) + 'static {
    if !mount.can_unmount() {
        return Err("Questo dispositivo non può essere smontato.".into());
    }
    let op=gtk::MountOperation::new(parent);
    let cancellable=gio::Cancellable::new();
    mount.unmount_with_operation(gio::MountUnmountFlags::NONE,
        Some(&op),Some(&cancellable),
        move |result|done(result.map_err(|e|error_text(e,"Smontaggio"))));
    Ok(cancellable)
}

/// Eject only explicitly ejectable mounts, with no FORCE. The caller displays
/// a second, user-confirmed action before invoking this function.
pub fn eject<F>(mount:&gio::Mount,parent:Option<&gtk::Window>,done:F)
    -> Result<gio::Cancellable,String>
where F:FnOnce(Result<(),String>)+'static {
    if !mount.can_eject(){
        return Err("Questo dispositivo non supporta l'espulsione.".into());
    }
    let op=gtk::MountOperation::new(parent);
    let cancellable=gio::Cancellable::new();
    mount.eject_with_operation(gio::MountUnmountFlags::NONE,Some(&op),
        Some(&cancellable),
        move |result|done(result.map_err(|e|error_text(e,"Espulsione"))));
    Ok(cancellable)
}

/// Confirmation is always required for disconnect/eject and does not auto
/// activate anything if the window is closed.
pub fn confirm_mount_action<F>(parent:Option<&gtk::Window>,title:&str,detail:&str,
    button_label:&str,action:F)
where F:FnOnce()+'static {
    let dialog=gtk::MessageDialog::new(parent,gtk::DialogFlags::MODAL,
        gtk::MessageType::Question,gtk::ButtonsType::None,title);
    dialog.set_secondary_text(Some(detail));
    dialog.add_button("Annulla",gtk::ResponseType::Cancel);
    dialog.add_button(button_label,gtk::ResponseType::Accept);
    let once=std::rc::Rc::new(std::cell::RefCell::new(Some(action)));
    dialog.connect_response(move |dialog,response|{
        dialog.close();
        if response==gtk::ResponseType::Accept {
            if let Some(action)=once.borrow_mut().take() {action();}
        }
    });
    dialog.present();
}
