//! First-class GIO/GVfs URI tabs using the existing Melania Files tab rail.
//! Blocking GVfs filesystem I/O is isolated in worker threads. GTK widgets
//! and GDK clipboard stay on the main loop; only Rust values cross threads.
use gtk::prelude::*;
use gtk::{gio, glib, gdk};
use gtk::{Box as GtkBox, Button, Entry, Label, Orientation, ScrolledWindow};
use std::{cell::{Cell,RefCell},collections::BTreeSet,rc::Rc,sync::{Arc,atomic::{AtomicBool,Ordering}},
          time::Duration};
use super::gio_backend::{self,Item};

#[derive(Clone)]
struct Remote {
    current: Rc<RefCell<String>>,
    back: Rc<RefCell<Vec<String>>>,
    forward: Rc<RefCell<Vec<String>>>,
    selected: Rc<RefCell<Option<Item>>>,
    selected_button: Rc<RefCell<Option<Button>>>,
    selected_paths: Rc<RefCell<BTreeSet<String>>>,
    selection_anchor: Rc<RefCell<Option<String>>>,
    visible_uris: Rc<RefCell<Vec<String>>>,
    row_buttons: Rc<RefCell<Vec<(String,Button)>>>,
    rows: GtkBox,
    location: Entry,
    search: Entry,
    status: Label,
    title: Label,
    hidden: Rc<Cell<bool>>,
    generation: Rc<Cell<u64>>,
    listing_cancel: Rc<RefCell<Option<Arc<AtomicBool>>>>,
    job_cancel: Rc<RefCell<Option<Arc<AtomicBool>>>>,
}

fn navigation(remote:&Remote,uri:String,remember:bool) {
    if let Err(error)=gio_backend::validate_uri(&uri) {
        remote.status.set_text(&error);
        return;
    }
    if remember && *remote.current.borrow()!=uri {
        let old=remote.current.borrow().clone();
        remote.back.borrow_mut().push(old);
        remote.forward.borrow_mut().clear();
    }
    *remote.current.borrow_mut()=uri;
    refresh(remote);
}
fn refresh(remote:&Remote) {
    if let Some(old)=remote.listing_cancel.borrow_mut().take() {
        old.store(true,Ordering::Release);
    }
    let gen=remote.generation.get().wrapping_add(1);
    remote.generation.set(gen);
    *remote.selected.borrow_mut()=None;
    *remote.selected_button.borrow_mut()=None;
    remote.selected_paths.borrow_mut().clear();
    remote.visible_uris.borrow_mut().clear();
    remote.row_buttons.borrow_mut().clear();
    *remote.selection_anchor.borrow_mut()=None;
    while let Some(child)=remote.rows.first_child(){remote.rows.remove(&child);}
    let uri=remote.current.borrow().clone();
    remote.location.set_text(&uri);
    let place=gio::File::for_uri(&uri).basename()
        .map(|s|s.to_string_lossy().into_owned()).unwrap_or_else(||String::from("Rete"));
    remote.title.set_text(&place);
    remote.status.set_text("Connessione e lettura in corso…");
    let filter=remote.search.text().to_string();
    let show_hidden=remote.hidden.get();
    let cancelled=Arc::new(AtomicBool::new(false));
    *remote.listing_cancel.borrow_mut()=Some(cancelled.clone());
    let (tx,rx)=std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let output=gio_backend::list_uri(&uri,&filter,show_hidden,&cancelled);
        let _=tx.send(output);
    });
    let ui=remote.clone();
    glib::timeout_add_local(Duration::from_millis(45),move || {
        if ui.generation.get()!=gen {return glib::ControlFlow::Break;}
        match rx.try_recv() {
            Ok(Ok(listing)) => {
                let count=listing.items.len();
                *ui.visible_uris.borrow_mut()=listing.items.iter()
                    .map(|item|item.uri.clone()).collect();
                for item in listing.items {render_row(&ui,item);}
                ui.status.set_text(&format!("{count} elementi{} · GIO/GVfs",
                    if listing.truncated {" (limite visualizzazione raggiunto)"} else {""}));
                if std::env::var_os("MELANIA_SMOKE_GIO_FILE_URI").is_some() {
                    eprintln!("MELANIA_GIO_LISTING_READY count={count}");
                }
                glib::ControlFlow::Break
            },
            Ok(Err(message)) => {
                ui.status.set_text(&message);
                glib::ControlFlow::Break
            },
            Err(std::sync::mpsc::TryRecvError::Empty)=>glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
                ui.status.set_text("Connessione interrotta");
                glib::ControlFlow::Break
            },
        }
    });
}
fn render_row(remote:&Remote,item:Item) {
    let button=Button::new();
    button.add_css_class("files-list-row");
    button.add_css_class("files-tile");
    button.set_tooltip_text(Some(&item.uri));
    let row=GtkBox::new(Orientation::Horizontal,10);
    row.set_margin_top(4);
    row.set_margin_bottom(4);
    let icon=gtk::Image::from_icon_name(if item.directory {"folder-symbolic"}else {"text-x-generic-symbolic"});
    icon.set_pixel_size(22);
    row.append(&icon);
    let name=Label::new(Some(&item.name));
    name.set_xalign(0.0);
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&name);
    let size=Label::new(Some(if item.directory {"Cartella"} else {"File"}));
    size.set_width_chars(9);
    row.append(&size);
    button.set_child(Some(&row));
    remote.row_buttons.borrow_mut().push((item.uri.clone(),button.clone()));
    let modifiers=Rc::new(Cell::new(gdk::ModifierType::empty()));
    let capture=gtk::GestureClick::new();
    capture.set_button(gdk::BUTTON_PRIMARY);
    let observed=modifiers.clone();
    capture.connect_pressed(move |gesture,_,_,_| {
        observed.set(gesture.current_event_state());
    });
    button.add_controller(capture);
    let state=remote.clone();
    let selected=item.clone();
    button.connect_clicked(move |clicked| {
        let mode=modifiers.get();
        let mut paths=state.selected_paths.borrow_mut();
        if mode.contains(gdk::ModifierType::SHIFT_MASK) {
            let anchor=state.selection_anchor.borrow().clone();
            let visible=state.visible_uris.borrow();
            if let (Some(a),Some(b))=(visible.iter().position(|p|p==&selected.uri),
                anchor.as_ref().and_then(|p|visible.iter().position(|v|v==p))) {
                paths.clear();
                paths.extend(visible[a.min(b)..=a.max(b)].iter().cloned());
            } else {
                paths.insert(selected.uri.clone());
            }
        } else if mode.contains(gdk::ModifierType::CONTROL_MASK) {
            if !paths.insert(selected.uri.clone()){paths.remove(&selected.uri);}
            *state.selection_anchor.borrow_mut()=Some(selected.uri.clone());
        } else {
            paths.clear();
            paths.insert(selected.uri.clone());
            *state.selection_anchor.borrow_mut()=Some(selected.uri.clone());
        }
        let count=paths.len();
        for (uri,widget) in state.row_buttons.borrow().iter() {
            if paths.contains(uri){widget.add_css_class("files-tile-selected");}
            else {widget.remove_css_class("files-tile-selected");}
        }
        let chosen=paths.contains(&selected.uri);
        *state.selected.borrow_mut()=if chosen {Some(selected.clone())}else {None};
        *state.selected_button.borrow_mut()=if chosen {Some(clicked.clone())}else {None};
        state.status.set_text(&format!("{count} elementi selezionati · GIO/GVfs"));
    });
    let double=gtk::GestureClick::new();
    double.set_button(gdk::BUTTON_PRIMARY);
    let state=remote.clone();
    let to_open=item.clone();
    double.connect_released(move |_,n,_,_| {
        if n!=2 {return;}
        if to_open.directory {
            navigation(&state,to_open.uri.clone(),true);
        } else if let Err(e)=gio::AppInfo::launch_default_for_uri(&to_open.uri,None::<&gio::AppLaunchContext>) {
            state.status.set_text(&format!("Impossibile aprire il file: {e}"));
        }
    });
    button.add_controller(double);
    // Export standard GDK FileList, compatible with other GTK applications.
    let drag=gtk::DragSource::new();
    drag.set_actions(gdk::DragAction::COPY);
    let dragged=item.uri.clone();
    let selection=remote.selected_paths.clone();
    drag.connect_prepare(move |_,_,_| {
        let paths=selection.borrow();
        let sources:Vec<String>=if paths.contains(&dragged){
            paths.iter().take(100).cloned().collect()
        }else{vec![dragged.clone()]};
        let files:Vec<gio::File>=sources.iter().map(|uri| gio::File::for_uri(uri)).collect();
        let list=gdk::FileList::from_array(&files);
        Some(gdk::ContentProvider::for_value(&list.to_value()))
    });
    button.add_controller(drag);
    remote.rows.append(&button);
}

fn run_job<F>(remote:&Remote,description:&str,work:F)
where F:FnOnce(Arc<AtomicBool>)->Result<String,String> + Send + 'static {
    if remote.job_cancel.borrow().is_some(){
        remote.status.set_text("Un trasferimento è già in corso; annullalo prima di riprovare.");
        return;
    }
    let cancelled=Arc::new(AtomicBool::new(false));
    *remote.job_cancel.borrow_mut()=Some(cancelled.clone());
    remote.status.set_text(description);
    let (tx,rx)=std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {let _=tx.send(work(cancelled));});
    let state=remote.clone();
    glib::timeout_add_local(Duration::from_millis(70),move || match rx.try_recv() {
        Ok(Ok(message))=>{
            state.job_cancel.borrow_mut().take();
            refresh(&state);
            state.status.set_text(&message);
            glib::ControlFlow::Break
        },
        Ok(Err(error))=>{
            state.job_cancel.borrow_mut().take();
            refresh(&state);
            state.status.set_text(&error);
            glib::ControlFlow::Break
        },
        Err(std::sync::mpsc::TryRecvError::Empty)=>glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
            state.job_cancel.borrow_mut().take();
            state.status.set_text("Operazione interrotta; controlla la destinazione.");
            glib::ControlFlow::Break
        },
    });
}

fn copy_sources(remote:&Remote,uris:Vec<String>) {
    let dest=remote.current.borrow().clone();
    run_job(remote,"Copia tramite GIO/GVfs in corso…",move |cancel| {
        let copied=gio_backend::copy_uris(&uris,&dest,&cancel)?;
        Ok(format!("{copied} elementi copiati senza sovrascrivere."))
    });
}

fn open_bookmarks_dialog(remote:&Remote){
    let window=gtk::Window::builder().title("Posizioni di rete preferite — Melania Files")
        .default_width(530).default_height(390).build();
    let content=GtkBox::new(Orientation::Vertical,8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);
    let scroller=ScrolledWindow::new();
    scroller.set_vexpand(true);
    let rows=GtkBox::new(Orientation::Vertical,4);
    match gio_backend::remote_bookmarks_path()
        .and_then(|path|gio_backend::read_bookmarks(&path)){
        Ok(locations) if locations.is_empty()=>{
            rows.append(&Label::new(Some("Nessun indirizzo di rete salvato.")));
        },
        Ok(locations)=>{
            for uri in locations {
                let button=Button::with_label(&uri);
                let active=remote.clone();
                let dialog=window.clone();
                button.connect_clicked(move |_|{
                    navigation(&active,uri.clone(),true);
                    dialog.close();
                });
                rows.append(&button);
            }
        },
        Err(e)=>rows.append(&Label::new(Some(&e))),
    }
    scroller.set_child(Some(&rows));
    content.append(&scroller);
    let close=Button::with_label("Chiudi");
    let win=window.clone();
    close.connect_clicked(move |_|win.close());
    content.append(&close);
    window.set_child(Some(&content));
    window.present();
}

pub fn create(initial:&str,tab_title:Label)->GtkBox {
    let root=GtkBox::new(Orientation::Vertical,0);
    let toolbar=GtkBox::new(Orientation::Horizontal,5);
    toolbar.add_css_class("files-toolbar");
    let back=Button::from_icon_name("go-previous-symbolic");
    back.set_tooltip_text(Some("Indietro"));
    let next=Button::from_icon_name("go-next-symbolic");
    next.set_tooltip_text(Some("Avanti"));
    let up=Button::from_icon_name("go-up-symbolic");
    up.set_tooltip_text(Some("Cartella superiore"));
    let reload=Button::from_icon_name("view-refresh-symbolic");
    reload.set_tooltip_text(Some("Aggiorna"));
    for b in [&back,&next,&up,&reload] {toolbar.append(b);}
    let location=Entry::new();
    location.set_placeholder_text(Some("smb://server/condivisione, sftp://server/cartella…"));
    location.set_hexpand(true);
    toolbar.append(&location);
    let search=Entry::new();
    search.set_placeholder_text(Some("Filtra…"));
    search.set_width_chars(16);
    toolbar.append(&search);
    root.append(&toolbar);

    let actionbar=GtkBox::new(Orientation::Horizontal,6);
    actionbar.add_css_class("files-actions");
    let copy=Button::with_label("Copia");
    let paste=Button::with_label("Incolla");
    let rename_field=Entry::new();
    rename_field.set_placeholder_text(Some("Nuovo nome"));
    rename_field.set_width_chars(15);
    let rename=Button::with_label("Rinomina");
    let folder_field=Entry::new();
    folder_field.set_placeholder_text(Some("Nome nuova cartella"));
    folder_field.set_width_chars(16);
    let folder=Button::with_label("Nuova cartella");
    let trash=Button::with_label("Cestino");
    let add_bookmark=Button::with_label("☆ Preferito");
    let remove_bookmark=Button::with_label("− Preferito");
    let bookmarks=Button::with_label("Preferiti rete");
    let cancel=Button::with_label("Annulla operazione");
    for button in [&copy,&paste] {actionbar.append(button);}
    actionbar.append(&rename_field);
    actionbar.append(&rename);
    actionbar.append(&folder_field);
    actionbar.append(&folder);
    actionbar.append(&trash);
    actionbar.append(&add_bookmark);
    actionbar.append(&remove_bookmark);
    actionbar.append(&bookmarks);
    actionbar.append(&cancel);
    let actions_scroll=ScrolledWindow::new();
    actions_scroll.set_policy(gtk::PolicyType::Automatic,gtk::PolicyType::Never);
    actions_scroll.set_child(Some(&actionbar));
    root.append(&actions_scroll);

    let scroll=ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hexpand(true);
    let rows=GtkBox::new(Orientation::Vertical,2);
    rows.add_css_class("files-list-view");
    scroll.set_child(Some(&rows));
    root.append(&scroll);
    let status=Label::new(Some("Percorsi GIO/GVfs: copie e modifiche non sovrascriventi"));
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.add_css_class("files-footer");
    root.append(&status);
    let state=Remote {
        current:Rc::new(RefCell::new(initial.to_string())),
        back:Rc::new(RefCell::new(Vec::new())),
        forward:Rc::new(RefCell::new(Vec::new())),
        selected:Rc::new(RefCell::new(None)),
        selected_button:Rc::new(RefCell::new(None)),
        selected_paths:Rc::new(RefCell::new(BTreeSet::new())),
        selection_anchor:Rc::new(RefCell::new(None)),
        visible_uris:Rc::new(RefCell::new(Vec::new())),
        row_buttons:Rc::new(RefCell::new(Vec::new())),
        rows, location, search, status, title:tab_title,
        hidden:Rc::new(Cell::new(false)),
        generation:Rc::new(Cell::new(0)),
        listing_cancel:Rc::new(RefCell::new(None)),
        job_cancel:Rc::new(RefCell::new(None)),
    };
    let ui=state.clone();
    state.location.connect_activate(move |input| {
        let typed=input.text().to_string();
        if typed.contains("://") {
            navigation(&ui,typed,true);
        } else {
            // Relative names stay inside the selected GIO parent.
            let dir=gio::File::for_uri(&ui.current.borrow());
            let child=dir.child(&typed);
            navigation(&ui,child.uri().to_string(),true);
        }
    });
    let ui=state.clone();
    state.search.connect_changed(move |_|refresh(&ui));
    let ui=state.clone();
    reload.connect_clicked(move |_|refresh(&ui));
    let ui=state.clone();
    up.connect_clicked(move |_|{
        let from=gio::File::for_uri(&ui.current.borrow());
        if let Some(parent)=from.parent(){navigation(&ui,parent.uri().to_string(),true);}
    });
    let ui=state.clone();
    back.connect_clicked(move |_|{
        if let Some(uri)=ui.back.borrow_mut().pop(){
            ui.forward.borrow_mut().push(ui.current.borrow().clone());
            navigation(&ui,uri,false);
        }
    });
    let ui=state.clone();
    next.connect_clicked(move |_|{
        if let Some(uri)=ui.forward.borrow_mut().pop(){
            ui.back.borrow_mut().push(ui.current.borrow().clone());
            navigation(&ui,uri,false);
        }
    });
    let ui=state.clone();
    copy.connect_clicked(move |_|{
        let selected:Vec<String>=ui.selected_paths.borrow().iter().cloned().collect();
        if selected.is_empty() {
            ui.status.set_text("Seleziona uno o più elementi.");return;
        }
        if selected.len()>100 {
            ui.status.set_text("Massimo 100 elementi per trasferimento remoto.");return;
        }
        let Some(display)=gdk::Display::default() else {
            ui.status.set_text("Appunti indisponibili.");return;
        };
        let files:Vec<gio::File>=selected.iter().map(|uri| gio::File::for_uri(uri)).collect();
        let list=gdk::FileList::from_array(&files);
        let provider=gdk::ContentProvider::for_value(&list.to_value());
        match display.clipboard().set_content(Some(&provider)){
            Ok(())=>ui.status.set_text("Elemento copiato negli appunti del sistema."),
            Err(e)=>ui.status.set_text(&format!("Appunti non aggiornati: {e}")),
        }
    });
    let ui=state.clone();
    paste.connect_clicked(move |_|{
        let Some(display)=gdk::Display::default() else {
            ui.status.set_text("Appunti indisponibili.");return;
        };
        let done=ui.clone();
        display.clipboard().read_value_async(
            gdk::FileList::static_type(),glib::Priority::DEFAULT,
            None::<&gio::Cancellable>,move |result|{
                let Ok(value)=result else {done.status.set_text("Gli appunti non contengono file.");return;};
                let Ok(files)=value.get::<gdk::FileList>() else {
                    done.status.set_text("Formato degli appunti non supportato.");return;
                };
                let sources=files.files().iter().map(|f|f.uri().to_string()).collect::<Vec<_>>();
                copy_sources(&done,sources);
            }
        );
    });
    let ui=state.clone();
    let rename_name=rename_field.clone();
    rename.connect_clicked(move |_|{
        if ui.selected_paths.borrow().len()!=1 {
            ui.status.set_text("Per rinominare seleziona un solo elemento.");return;
        }
        let Some(selected)=ui.selected.borrow().clone() else {
            ui.status.set_text("Seleziona un file da rinominare.");return;
        };
        let name=rename_name.text().to_string();
        run_job(&ui,"Rinomina tramite GIO…",move |_|{
            gio_backend::rename_item(&selected.uri,&name)?;
            Ok("Elemento rinominato.".to_string())
        });
    });
    let ui=state.clone();
    folder.connect_clicked(move |_| {
        let name=folder_field.text().to_string();
        let dest=ui.current.borrow().clone();
        run_job(&ui,"Creazione cartella…",move |_|{
            gio_backend::make_folder(&dest,&name)?;
            Ok("Cartella creata.".to_string())
        });
    });
    let ui=state.clone();
    trash.connect_clicked(move |_|{
        let selected:Vec<String>=ui.selected_paths.borrow().iter().cloned().collect();
        if selected.is_empty() {
            ui.status.set_text("Seleziona uno o più elementi per il Cestino.");return;
        }
        run_job(&ui,"Spostamento nel Cestino…",move |cancel|{
            let mut moved=0;
            for uri in selected {
                if cancel.load(Ordering::Acquire){
                    return Err(format!("{moved} elementi nel Cestino; operazione interrotta."));
                }
                gio_backend::move_to_trash(&uri).map_err(|e|
                    format!("{moved} elementi già spostati; file restante non eliminato: {e}"))?;
                moved+=1;
            }
            Ok(format!("{moved} elementi spostati nel Cestino."))
        });
    });
    let ui=state.clone();
    add_bookmark.connect_clicked(move |_|{
        let uri=ui.current.borrow().clone();
        match gio_backend::add_bookmark(&uri){
            Ok(_)=>ui.status.set_text("Percorso GVfs salvato nei preferiti di Melania."),
            Err(error)=>ui.status.set_text(&format!("Preferito non salvato: {error}")),
        }
    });
    let ui=state.clone();
    remove_bookmark.connect_clicked(move |_|{
        let uri=ui.current.borrow().clone();
        match gio_backend::remove_bookmark(&uri){
            Ok(_)=>ui.status.set_text("Percorso rimosso dai preferiti."),
            Err(error)=>ui.status.set_text(&format!("Impossibile aggiornare i preferiti: {error}")),
        }
    });
    let ui=state.clone();
    bookmarks.connect_clicked(move |_|open_bookmarks_dialog(&ui));
    let ui=state.clone();
    cancel.connect_clicked(move |_|{
        if let Some(flag)=ui.job_cancel.borrow().as_ref(){
            flag.store(true,Ordering::Release);
            ui.status.set_text("Annullamento richiesto; nessun originale viene cancellato.");
        } else if let Some(flag)=ui.listing_cancel.borrow().as_ref(){
            flag.store(true,Ordering::Release);
            ui.status.set_text("Lettura interrotta.");
        }
    });
    // GNOME/Qt interop: accept GDK file lists from other file managers; COPY only.
    let drop=gtk::DropTarget::new(gdk::FileList::static_type(),gdk::DragAction::COPY);
    let ui=state.clone();
    drop.connect_drop(move |_,value,_,_|{
        let Ok(files)=value.get::<gdk::FileList>() else {return false;};
        let uris=files.files().iter().map(|f|f.uri().to_string()).collect::<Vec<_>>();
        if uris.is_empty() || ui.job_cancel.borrow().is_some() {return false;}
        copy_sources(&ui,uris);
        true
    });
    scroll.add_controller(drop);
    let keyboard=gtk::EventControllerKey::new();
    let ui=state.clone();
    let refresh_button=reload.clone();
    let location_entry=state.location.clone();
    let search_entry=state.search.clone();
    let copy_button=copy.clone();
    let paste_button=paste.clone();
    keyboard.connect_key_pressed(move |_,key,_,modifiers|{
        if key==gdk::Key::F5 {refresh_button.emit_clicked();return glib::Propagation::Stop;}
        if modifiers.contains(gdk::ModifierType::CONTROL_MASK){
            if key==gdk::Key::l {location_entry.grab_focus();location_entry.select_region(0,-1);return glib::Propagation::Stop;}
            if key==gdk::Key::f {search_entry.grab_focus();return glib::Propagation::Stop;}
            if !location_entry.has_focus() && !search_entry.has_focus(){
                if key==gdk::Key::c {copy_button.emit_clicked();return glib::Propagation::Stop;}
                if key==gdk::Key::v {paste_button.emit_clicked();return glib::Propagation::Stop;}
            }
        }
        if modifiers.contains(gdk::ModifierType::CONTROL_MASK)
            && key==gdk::Key::a
            && !location_entry.has_focus() && !search_entry.has_focus(){
            let visible=ui.visible_uris.borrow();
            let mut selected=ui.selected_paths.borrow_mut();
            selected.clear();
            selected.extend(visible.iter().cloned());
            for (_,row) in ui.row_buttons.borrow().iter(){
                row.add_css_class("files-tile-selected");
            }
            ui.status.set_text(&format!("{} elementi selezionati",selected.len()));
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    root.add_controller(keyboard);
    refresh(&state);
    root
}
