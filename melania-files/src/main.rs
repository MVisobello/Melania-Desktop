//! Early working filesystem browser, not a replacement for a mature file manager yet.
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use melania_transfer as transfer;
use melania_files_core as model;
use model::nautilus_features as nautilus;
use gtk::{Align, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, Entry, FlowBox, Image, Label, Orientation, Picture, ScrolledWindow};
use std::{cell::{Cell, RefCell}, collections::BTreeSet, fs, path::{Path, PathBuf}, process::Command, rc::Rc, sync::{Arc, atomic::{AtomicBool, Ordering}}, time::Duration};

#[derive(Clone)]
struct PendingTransfer { paths: Vec<PathBuf>, is_move: bool }

#[derive(Clone)]
struct Browser {
    current: Rc<RefCell<PathBuf>>,
    selected: Rc<RefCell<Option<PathBuf>>>,
    selected_paths: Rc<RefCell<BTreeSet<PathBuf>>>,
    selection_anchor: Rc<RefCell<Option<PathBuf>>>,
    visible_paths: Rc<RefCell<Vec<PathBuf>>>,
    tile_buttons: Rc<RefCell<Vec<(PathBuf, Button)>>>,
    tiles: FlowBox,
    empty_view: GtkBox,
    empty_icon: Image,
    empty_title: Label,
    empty_explanation: Label,
    list_mode: Rc<Cell<bool>>,
    list_header: GtkBox,
    preview_icon: Image,
    preview_heading: Label,
    location: Entry,
    status: Label,
    preview: Label,
    preview_image: Picture,
    search: Entry,
    back: Rc<RefCell<Vec<PathBuf>>>,
    forward: Rc<RefCell<Vec<PathBuf>>>,
    show_hidden: Rc<Cell<bool>>,
    only_images: Rc<Cell<bool>>,
    pending: Rc<RefCell<Option<PendingTransfer>>>,
    sort_by: Rc<Cell<model::SortBy>>,
    descending: Rc<Cell<bool>>,
    favorites_box: GtkBox,
    bookmarks: Rc<RefCell<model::Bookmarks>>,
    tab_title: Label,
    highlighted: Rc<RefCell<Option<Button>>>,
    // GIO filesystem events refresh the active tab without polling.
    monitor: Rc<RefCell<Option<gio::FileMonitor>>>,
    refresh_pending: Rc<Cell<bool>>,
    mounts_box: GtkBox,
    copy_action: Button,
    cut_action: Button,
    trash_action: Button,
    rename_field: Entry,
    transfer_progress: gtk::ProgressBar,
    transfer_cancel: Button,
    active_job: Rc<RefCell<Option<Arc<AtomicBool>>>>,
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"))
}

fn human_size(size: u64) -> String {
    if size < 1024 { return format!("{size} B"); }
    if size < 1024 * 1024 { return format!("{:.1} KB", size as f64 / 1024.0); }
    if size < 1024 * 1024 * 1024 { return format!("{:.1} MB", size as f64 / 1048576.0); }
    format!("{:.1} GB", size as f64 / 1073741824.0)
}

fn is_picture(path: &Path) -> bool { model::is_picture(path) }

/// Give users a meaningful reason when a folder has no visible files.
/// Filtering can hide every result even when the directory is not empty.
fn empty_folder_message(query: &str, pictures_only: bool, show_hidden: bool)
    -> (&'static str, &'static str, &'static str) {
    if !query.trim().is_empty() {
        ("system-search", "Nessun risultato",
         "Prova un altro nome oppure cancella la ricerca.")
    } else if pictures_only {
        ("image-x-generic", "Nessuna immagine visibile",
         "Disattiva il filtro Foto per visualizzare tutti i file.")
    } else if !show_hidden {
        ("folder", "Questa cartella è vuota",
         "Puoi creare una cartella o trascinare qui i tuoi file. I file nascosti non sono visualizzati.")
    } else {
        ("folder", "Nessun elemento in questa cartella",
         "Puoi creare una cartella o trascinare qui i tuoi file.")
    }
}


/// Display metadata supplied by the bounded filesystem model, never
/// recursively probe symlink targets just to populate a table row.
fn file_type_label(entry: &model::Entry) -> String {
    if entry.is_directory { return "Cartella".into(); }
    if entry.is_symlink { return "Collegamento".into(); }
    match entry.path.extension().and_then(|s| s.to_str())
        .unwrap_or("").to_ascii_lowercase().as_str() {
        "pdf" => "Documento PDF".into(),
        "txt"|"md"|"odt"|"doc"|"docx" => "Documento".into(),
        "ods"|"xls"|"xlsx"|"csv" => "Foglio di calcolo".into(),
        "odp"|"ppt"|"pptx" => "Presentazione".into(),
        "png"|"jpg"|"jpeg"|"webp"|"gif"|"svg"|"bmp" => "Immagine".into(),
        "zip"|"7z"|"tar"|"gz"|"xz"|"rar" => "Archivio".into(),
        "mp3"|"flac"|"ogg"|"wav" => "Audio".into(),
        "mp4"|"mkv"|"webm"|"avi" => "Video".into(),
        "" => "File".into(),
        other => other.to_uppercase(),
    }
}
fn modified_time_label(modified: Option<std::time::SystemTime>) -> String {
    use std::time::UNIX_EPOCH;
    let Some(epoch) = modified.and_then(|m|m.duration_since(UNIX_EPOCH).ok()) else {
        return "—".into();
    };
    let secs = i64::try_from(epoch.as_secs()).unwrap_or(i64::MAX);
    glib::DateTime::from_unix_local(secs).ok()
        .and_then(|d| d.format("%d/%m/%Y %H:%M").ok())
        .map(|s|s.to_string()).unwrap_or_else(||"—".into())
}

fn list_heading_label(title: &str, width: i32, expand: bool) -> Label {
    let label = Label::new(Some(title));
    label.set_xalign(0.0);
    label.set_hexpand(expand);
    if width > 0 {label.set_size_request(width, -1);}
    label.add_css_class("files-list-heading-label");
    label
}

/// Use the actual XDG user directories, including localized names such as
/// "Documenti" / "Scrivania". Never assume ~/Documents exists.
fn xdg_user_directory(kind: &str, fallback: &str) -> PathBuf {
    if let Ok(output) = Command::new("xdg-user-dir").arg(kind).output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let value = text.trim();
            if !value.is_empty() {
                let path = PathBuf::from(value);
                if path.is_dir() { return path; }
            }
        }
    }
    home().join(fallback)
}

/// Prefer native theme icons instead of emoji; users can change Melania's
/// icon theme without modifying the file manager.
fn entry_icon(path: &Path, is_dir: bool) -> &'static str {
    if is_dir {
        let folder = path.file_name().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        return match folder.as_str() {
            "documenti"|"documents" => "folder-documents",
            "immagini"|"pictures"|"images" => "folder-pictures",
            "musica"|"music" => "folder-music",
            "video"|"videos" => "folder-videos",
            "scaricati"|"download"|"downloads" => "folder-download",
            "progetti"|"projects" => "folder-projects",
            "condivisi"|"public" => "folder-publicshare",
            _ => "folder",
        };
    }
    match path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "pdf" => "application-pdf",
        "odt"|"doc"|"docx"|"txt"|"md" => "x-office-document",
        "ods"|"xls"|"xlsx"|"csv" => "x-office-spreadsheet",
        "odp"|"ppt"|"pptx" => "x-office-presentation",
        "zip"|"tar"|"gz"|"xz"|"7z"|"rar" => "package-x-generic",
        "mp3"|"flac"|"ogg"|"wav" => "audio-x-generic",
        "mp4"|"mkv"|"webm"|"avi" => "video-x-generic",
        "rs"|"py"|"c"|"h"|"cpp"|"js"|"json"|"toml" => "text-x-script",
        _ => "text-x-generic",
    }
}

/// Offer local files to GTK/Qt/Wayland applications that accept GDK file lists.
/// COPY only for this pre-alpha; MOVE cannot be enabled safely until source
/// deletion and MIME/action negotiation have been tested end-to-end.
fn attach_drag(widget: &impl IsA<gtk::Widget>, path: PathBuf, browser: Browser) {
    let source = gtk::DragSource::new();
    source.set_actions(gdk::DragAction::COPY);
    let origin = path.clone();
    let selected = browser.selected_paths.clone();
    source.connect_prepare(move |_, _, _| {
        // Drag the complete selected set if the grabbed tile was selected.
        // COPY only: GTK must never delete originals after the drop.
        let paths = selected.borrow();
        let origins: Vec<PathBuf> = if paths.contains(&origin) {
            paths.iter().take(256).cloned().collect()
        } else { vec![origin.clone()] };
        let files: Vec<gio::File> = origins.iter().map(gio::File::for_path).collect();
        let list = gdk::FileList::from_array(&files);
        Some(gdk::ContentProvider::for_value(&list.to_value()))
    });
    let status = browser.status.clone();
    source.connect_drag_end(move |_, _, _| {
        status.set_text("Trascinamento terminato. L'originale non viene cancellato.");
    });
    widget.add_controller(source);
}

/// Run a validated local COPY off GTK's main thread. A cancelled copy rolls
/// back only the destination entries created by this job, never the sources.
/// No GTK object crosses the thread boundary.
fn start_background_copy(browser: &Browser, sources: Vec<PathBuf>, target: PathBuf) {
    if browser.active_job.borrow().is_some() {
        browser.status.set_text("Attendi o annulla il trasferimento già in corso.");
        return;
    }
    let job = match transfer::background::start_copy(sources, target) {
        Ok(job) => job,
        Err(err) => { browser.status.set_text(&format!("Copia non avviata: {err}")); return; }
    };
    browser.transfer_progress.set_fraction(0.0);
    browser.transfer_progress.set_visible(true);
    browser.transfer_cancel.set_visible(true);
    let cancellation = job.cancelled.clone();
    *browser.active_job.borrow_mut() = Some(cancellation);
    let state = browser.clone();
    glib::timeout_add_local(Duration::from_millis(60), move || {
        while let Ok(event) = job.events.try_recv() {
            match event {
                transfer::background::Event::Progress(p) => {
                    state.transfer_progress.set_fraction(p.fraction());
                    state.status.set_text(&format!("Copia: {} / {} elementi · {} / {} byte",
                        p.completed_items, p.total_items, p.completed_bytes, p.total_bytes));
                },
                transfer::background::Event::Finished(created) => {
                    state.active_job.borrow_mut().take();
                    state.transfer_cancel.set_visible(false);
                    state.transfer_progress.set_visible(false);
                    refresh(&state);
                    state.status.set_text(&format!("Copia completata: {} elementi.", created.len()));
                    return glib::ControlFlow::Break;
                },
                transfer::background::Event::Cancelled => {
                    state.active_job.borrow_mut().take();
                    state.transfer_cancel.set_visible(false);
                    state.transfer_progress.set_visible(false);
                    refresh(&state);
                    state.status.set_text("Copia annullata; gli originali sono intatti.");
                    return glib::ControlFlow::Break;
                },
                transfer::background::Event::Failed(message) => {
                    state.active_job.borrow_mut().take();
                    state.transfer_cancel.set_visible(false);
                    state.transfer_progress.set_visible(false);
                    refresh(&state);
                    state.status.set_text(&format!("Copia non completata: {message}"));
                    return glib::ControlFlow::Break;
                },
            }
        }
        // Distinguish a dropped worker channel from a stalled UI.
        if matches!(job.events.try_recv(), Err(std::sync::mpsc::TryRecvError::Disconnected)) {
            state.active_job.borrow_mut().take();
            state.transfer_cancel.set_visible(false);
            state.transfer_progress.set_visible(false);
            state.status.set_text("Il processo di copia è terminato in modo inatteso.");
            return glib::ControlFlow::Break;
        }
        glib::ControlFlow::Continue
    });
}

/// Accepts standard GDK file-list transfers, including drops from other apps.
/// Only advertise COPY: the source must never mistake a copy for a move.
fn attach_drop(widget: &impl IsA<gtk::Widget>, destination: Option<PathBuf>, browser: Browser) {
    // COPY ONLY until full action negotiation has been verified end-to-end.
    // Never advertise MOVE if we unconditionally perform a copy in this handler:
    // the source could otherwise delete the original after an ambiguous result.
    let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    let hover_widget: gtk::Widget = widget.as_ref().clone();
    drop.connect_enter(move |_, _, _| {
        hover_widget.add_css_class("drop-ready");
        gdk::DragAction::COPY
    });
    let hover_widget: gtk::Widget = widget.as_ref().clone();
    drop.connect_leave(move |_| hover_widget.remove_css_class("drop-ready"));
    drop.connect_drop(move |_, value, _, _| {
        let Ok(list) = value.get::<gdk::FileList>() else { return false; };
        let mut sources = Vec::new();
        for file in list.files() {
            let Some(path) = file.path() else {
                browser.status.set_text("Trascinamento da archivio remoto non ancora supportato.");
                return false;
            };
            sources.push(path);
        }
        let target = destination.clone().unwrap_or_else(|| browser.current.borrow().clone());
        // The drop is accepted for COPY only after preflight validation.
        // Work proceeds asynchronously; returning true NEVER advertises MOVE.
        if browser.active_job.borrow().is_some() { return false; }
        if let Err(err) = transfer::plan_copy(&sources, &target) {
            browser.status.set_text(&format!("Trascinamento non accettato: {err}"));
            return false;
        }
        start_background_copy(&browser, sources, target);
        true
    });
    widget.add_controller(drop);
}

/// File properties from symlink_metadata: never follows a symbolic link.
fn show_properties(path: &Path) {
    let result = nautilus::properties(path);
    let data = match result {
        Ok(p) => {
            let modified = p.modified.and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| format!("{} secondi da Unix epoch", d.as_secs()))
                .unwrap_or_else(|| "Non disponibile".into());
            let link = p.symlink_target.map(|x| format!("\nCollegamento a: {}", x.display()))
                .unwrap_or_default();
            #[cfg(unix)] let permissions = format!("\nPermessi (ottale): {:04o}", p.mode);
            #[cfg(not(unix))] let permissions = String::new();
            format!("Percorso: {}\n\nTipo: {}\nDimensione: {}\nUltima modifica: {}\nSola lettura: {}{}{}",
                p.path.display(), p.kind, human_size(p.bytes), modified,
                if p.readonly { "Sì" } else { "No" }, permissions, link)
        },
        Err(e) => format!("Impossibile leggere le proprietà: {e}"),
    };
    let win = gtk::Window::builder().title("Proprietà — Melania Files")
        .default_width(480).default_height(280).build();
    let layout = GtkBox::new(Orientation::Vertical, 12);
    layout.set_margin_top(20); layout.set_margin_bottom(20);
    layout.set_margin_start(20); layout.set_margin_end(20);
    let info = Label::new(Some(&data));
    info.set_selectable(true); info.set_wrap(true); info.set_xalign(0.0);
    layout.append(&info);
    let close = Button::with_label("Chiudi");
    let win_for_close = win.clone();
    close.connect_clicked(move |_| win_for_close.close());
    layout.append(&close);
    win.set_child(Some(&layout));
    win.present();
}

fn watch_directory(browser: &Browser) {
    // Drop the previous monitor when switching folders.
    *browser.monitor.borrow_mut() = None;
    let dir = browser.current.borrow().clone();
    if let Ok(monitor) = gio::File::for_path(dir).monitor_directory(
        gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>) {
        let state = browser.clone();
        monitor.connect_changed(move |_, _, _, _| {
            if state.refresh_pending.replace(true) { return; }
            let update = state.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(220), move || {
                update.refresh_pending.set(false);
                refresh(&update);
            });
        });
        *browser.monitor.borrow_mut() = Some(monitor);
    }
}

fn repaint_volumes(browser: &Browser) {
    while let Some(child) = browser.mounts_box.first_child() { browser.mounts_box.remove(&child); }
    let heading = Label::new(Some("DISPOSITIVI E VOLUMI"));
    heading.add_css_class("files-sidebar-heading");
    heading.set_xalign(0.0);
    browser.mounts_box.append(&heading);
    // Only local mount roots are navigable in this PathBuf-based version.
    // Remote GIO URIs require the planned backend migration to gio::File.
    for mount in gio::VolumeMonitor::get().mounts() {
        let Some(root) = mount.root().path() else { continue; };
        if !root.is_dir() { continue; }
        browser.mounts_box.append(&navigation_button(&mount.name(), root, browser));
    }
}

fn refresh(browser: &Browser) {
    // Previous tiles will be destroyed. Never keep a strong reference to an old tile.
    browser.highlighted.borrow_mut().take();
    while let Some(child) = browser.tiles.first_child() {
        if let Ok(tile) = child.downcast::<gtk::FlowBoxChild>() {
            browser.tiles.remove(&tile);
        } else { break; }
    }
    let dir = browser.current.borrow().clone();
    browser.location.set_text(&dir.display().to_string());
    // The label belongs to this browser page. Never update another tab's title.
    let caption = dir.file_name().filter(|n| !n.is_empty())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| dir.display().to_string());
    browser.tab_title.set_text(&caption);
    let query = browser.search.text().to_lowercase();
    let options = model::ListingOptions {
        show_hidden: browser.show_hidden.get(),
        pictures_only: browser.only_images.get(),
        sort_by: browser.sort_by.get(),
        descending: browser.descending.get(),
        ..Default::default()
    };
    let entries = match model::list_directory(&dir, &query, options) {
        Ok(entries) => entries,
        Err(err) => {
            browser.empty_view.set_visible(true);
            browser.empty_icon.set_icon_name(Some("dialog-error"));
            browser.empty_title.set_text("Impossibile leggere la cartella");
            browser.empty_explanation.set_text(&format!("{err}"));
            browser.list_header.set_visible(false);
            browser.status.set_text(&format!("Impossibile accedere: {err}"));
            return;
        }
    };
    let is_empty = entries.is_empty();
    browser.empty_view.set_visible(is_empty);
    browser.list_header.set_visible(!is_empty && browser.list_mode.get());
    if is_empty {
        let (icon, title, description) = empty_folder_message(
            &query, browser.only_images.get(), browser.show_hidden.get());
        browser.empty_icon.set_icon_name(Some(icon));
        browser.empty_title.set_text(title);
        browser.empty_explanation.set_text(description);
    }
    // A selection filtered out of this tab must not remain actionable.
    // In particular, never allow Trash/Rename to operate on a hidden stale item.
    let visible: Vec<PathBuf> = entries.iter().map(|entry| entry.path.clone()).collect();
    *browser.visible_paths.borrow_mut() = visible.clone();
    browser.selected_paths.borrow_mut().retain(|path| visible.contains(path));
    let selection_is_visible = browser.selected.borrow().as_ref()
        .is_some_and(|path| browser.selected_paths.borrow().contains(path));
    if !selection_is_visible {
        *browser.selected.borrow_mut() = browser.selected_paths.borrow().iter().next().cloned();
        browser.preview.set_text("Seleziona un elemento per visualizzarne i dettagli.");
        browser.preview_image.set_visible(false);
        browser.preview_image.set_filename(None::<&Path>);
    }
    browser.tile_buttons.borrow_mut().clear();
    let mut count = 0;
    let list_mode = browser.list_mode.get();
    for entry in entries {
        let type_name = file_type_label(&entry);
        let file_size = if entry.is_directory {"—".to_string()} else {human_size(entry.size)};
        let changed = modified_time_label(entry.modified);
        let path = entry.path;
        let name = entry.name;
        count += 1;
        let folder = entry.is_directory;
        let button = Button::new();
        button.add_css_class("files-tile");
        button.set_tooltip_text(Some(&path.display().to_string()));
        if list_mode {
            // Four useful columns without changing selection, DnD, context
            // menus, default app opening or the underlying file model.
            button.add_css_class("files-list-row");
            let row = GtkBox::new(Orientation::Horizontal, 9);
            row.add_css_class("files-list-row-content");
            let name_cell = GtkBox::new(Orientation::Horizontal, 9);
            name_cell.set_hexpand(true);
            name_cell.set_halign(Align::Fill);
            let icon = Image::from_icon_name(entry_icon(&path, folder));
            icon.set_pixel_size(23);
            name_cell.append(&icon);
            let text = Label::new(Some(&name));
            text.set_xalign(0.0);
            text.set_hexpand(true);
            text.set_ellipsize(gtk::pango::EllipsizeMode::End);
            text.add_css_class("files-list-name");
            name_cell.append(&text);
            row.append(&name_cell);
            let size_cell = list_heading_label(&file_size, 84, false);
            size_cell.add_css_class("files-list-meta");
            row.append(&size_cell);
            let type_cell = list_heading_label(&type_name, 124, false);
            type_cell.set_ellipsize(gtk::pango::EllipsizeMode::End);
            type_cell.add_css_class("files-list-meta");
            row.append(&type_cell);
            let changed_cell = list_heading_label(&changed, 132, false);
            changed_cell.add_css_class("files-list-meta");
            row.append(&changed_cell);
            button.set_child(Some(&row));
        } else {
            let stack = GtkBox::new(Orientation::Vertical, 6);
            stack.set_halign(Align::Center);
            if is_picture(&path) && path.is_file() {
                let picture = Picture::for_filename(&path);
                picture.set_size_request(90, 70);
                picture.set_content_fit(gtk::ContentFit::Contain);
                stack.append(&picture);
            } else {
                let symbol = Image::from_icon_name(entry_icon(&path, folder));
                symbol.set_pixel_size(42);
                symbol.add_css_class("files-icon");
                stack.append(&symbol);
            }
            let short = if name.chars().count() > 20 {
                format!("{}…", name.chars().take(19).collect::<String>())
            } else { name.clone() };
            let label = Label::new(Some(&short));
            label.add_css_class("files-name");
            stack.append(&label);
            button.set_child(Some(&stack));
        }
        if browser.selected_paths.borrow().contains(&path) {
            button.add_css_class("files-tile-selected");
        }
        browser.tile_buttons.borrow_mut().push((path.clone(), button.clone()));
        attach_drag(&button, path.clone(), browser.clone());
        if folder { attach_drop(&button, Some(path.clone()), browser.clone()); }
        let state = browser.clone();
        // One click selects, the second activates. Do not navigate on the first click:
        // that would make multi-selection and drag & drop difficult to use.
        let selected_path = path.clone();
        let modifiers = Rc::new(Cell::new(gdk::ModifierType::empty()));
        let modifier_capture = gtk::GestureClick::new();
        modifier_capture.set_button(gdk::BUTTON_PRIMARY);
        let captured = modifiers.clone();
        modifier_capture.connect_pressed(move |gesture, _, _, _| {
            captured.set(gesture.current_event_state());
        });
        button.add_controller(modifier_capture);
        button.connect_clicked(move |_| {
            let mode = modifiers.get();
            let mut selection = state.selected_paths.borrow_mut();
            if mode.contains(gdk::ModifierType::SHIFT_MASK) {
                let anchor = state.selection_anchor.borrow().clone();
                let visible = state.visible_paths.borrow();
                if let (Some(a), Some(b)) = (
                    visible.iter().position(|p| p == &selected_path),
                    anchor.as_ref().and_then(|p| visible.iter().position(|v| v == p))) {
                    selection.clear();
                    for p in &visible[a.min(b)..=a.max(b)] { selection.insert(p.clone()); }
                } else { selection.insert(selected_path.clone()); }
            } else if mode.contains(gdk::ModifierType::CONTROL_MASK) {
                if !selection.insert(selected_path.clone()) { selection.remove(&selected_path); }
                *state.selection_anchor.borrow_mut() = Some(selected_path.clone());
            } else {
                selection.clear();
                selection.insert(selected_path.clone());
                *state.selection_anchor.borrow_mut() = Some(selected_path.clone());
            }
            for (path, widget) in state.tile_buttons.borrow().iter() {
                if selection.contains(path) { widget.add_css_class("files-tile-selected"); }
                else { widget.remove_css_class("files-tile-selected"); }
            }
            let count = selection.len();
            *state.selected.borrow_mut() = if selection.contains(&selected_path) {
                Some(selected_path.clone())
            } else { selection.iter().next().cloned() };
            drop(selection);
            if count > 1 {
                state.preview_heading.set_text(&format!("{count} elementi selezionati"));
                state.preview_icon.set_icon_name(Some("folder-multiple-symbolic"));
                state.preview_icon.set_visible(true);
                state.preview.set_text(&format!("{count} elementi selezionati · Ctrl+C/Ctrl+X, Canc o trascina per copiarli."));
                state.preview_image.set_visible(false);
                return;
            }
            if count == 0 {
                state.preview_heading.set_text("Nessun elemento selezionato");
                state.preview_icon.set_visible(false);
                state.preview.set_text("Nessun elemento selezionato.");
                state.preview_image.set_visible(false);
                return;
            }
            let metadata = fs::symlink_metadata(&selected_path).ok();
            let size = metadata.map(|m| human_size(m.len())).unwrap_or_else(|| "n/d".into());
            let excerpt = if !folder && matches!(selected_path.extension().and_then(|e| e.to_str())
                .unwrap_or("").to_ascii_lowercase().as_str(), "txt"|"md"|"rs"|"toml"|"json"|"csv") {
                use std::io::Read;
                fs::File::open(&selected_path).ok().and_then(|f| {
                    let mut bytes = Vec::new();
                    f.take(2048).read_to_end(&mut bytes).ok()?;
                    Some(String::from_utf8_lossy(&bytes).to_string())
                }).unwrap_or_default()
            } else { String::new() };
            let excerpt = if excerpt.is_empty() { String::new() }
                else { format!("\n\nANTEPRIMA TESTO\n{excerpt}") };
            state.preview_heading.set_text(&name);
            state.preview_icon.set_icon_name(Some(entry_icon(&selected_path, folder)));
            state.preview_icon.set_visible(!is_picture(&selected_path));
            state.preview.set_text(&format!("Tipo: {}\nDimensione: {}\nPosizione: {}{}",
                if folder {"Cartella"} else {"File"}, size, selected_path.parent().unwrap_or(&selected_path).display(), excerpt));
            if is_picture(&selected_path) && selected_path.is_file() {
                state.preview_image.set_filename(Some(&selected_path));
                state.preview_image.set_visible(true);
            } else {
                state.preview_image.set_visible(false);
                state.preview_image.set_filename(None::<&Path>);
            }
        });
        let activation = gtk::GestureClick::new();
        activation.set_button(gdk::BUTTON_PRIMARY);
        let active_path = path.clone();
        let active_browser = browser.clone();
        activation.connect_released(move |_, count, _, _| {
            if count != 2 { return; }
            if folder {
                navigate(&active_browser, active_path.clone(), true);
            } else {
                let uri = gio::File::for_path(&active_path).uri();
                if let Err(error) = gio::AppInfo::launch_default_for_uri(
                    &uri, None::<&gio::AppLaunchContext>) {
                    active_browser.status.set_text(&format!("Impossibile aprire il file: {error}"));
                }
            }
        });
        button.add_controller(activation);
        // Nautilus-like secondary-click menu attached to the real tile.
        // Never delete permanently as a fallback when Trash is unavailable.
        let popover = gtk::Popover::new();
        popover.set_parent(&button);
        popover.set_has_arrow(false);
        let menu = GtkBox::new(Orientation::Vertical, 3);
        menu.add_css_class("files-context-menu");
        for (caption, command) in [
            ("Apri", "open"), ("Copia", "copy"), ("Taglia", "cut"),
            ("Rinomina…", "rename"), ("Sposta nel Cestino", "trash"),
            ("Proprietà", "properties")
        ] {
            let entry = Button::with_label(caption);
            entry.add_css_class("files-context-entry");
            let state = browser.clone();
            let target = path.clone();
            let pop = popover.clone();
            entry.connect_clicked(move |_| {
                if !state.selected_paths.borrow().contains(&target) {
                    let mut selected=state.selected_paths.borrow_mut();
                    selected.clear();
                    selected.insert(target.clone());
                }
                *state.selected.borrow_mut() = Some(target.clone());
                pop.popdown();
                match command {
                    "open" => {
                        if target.is_dir() { navigate(&state, target.clone(), true); }
                        else {
                            let uri = gio::File::for_path(&target).uri();
                            if let Err(e) = gio::AppInfo::launch_default_for_uri(&uri,None::<&gio::AppLaunchContext>) {
                                state.status.set_text(&format!("Apertura non riuscita: {e}"));
                            }
                        }
                    },
                    "copy" => state.copy_action.emit_clicked(),
                    "cut" => state.cut_action.emit_clicked(),
                    "rename" => { state.rename_field.set_text(&target.file_name().unwrap_or_default().to_string_lossy()); state.rename_field.grab_focus(); },
                    "trash" => state.trash_action.emit_clicked(),
                    "properties" => show_properties(&target),
                    _ => (),
                }
            });
            menu.append(&entry);
        }
        popover.set_child(Some(&menu));
        let secondary_click = gtk::GestureClick::new();
        secondary_click.set_button(gdk::BUTTON_SECONDARY);
        let pop = popover.clone();
        let state = browser.clone();
        let selection = path.clone();
        secondary_click.connect_pressed(move |_, _, _, _| {
            // Right-click on a selected tile preserves its multi-selection.
            // Right-click elsewhere selects only that tile before opening actions.
            if !state.selected_paths.borrow().contains(&selection) {
                let mut selected=state.selected_paths.borrow_mut();
                selected.clear();
                selected.insert(selection.clone());
            }
            *state.selected.borrow_mut() = Some(selection.clone());
            for (path, widget) in state.tile_buttons.borrow().iter() {
                if state.selected_paths.borrow().contains(path) { widget.add_css_class("files-tile-selected"); }
                else { widget.remove_css_class("files-tile-selected"); }
            }
            pop.popup();
        });
        button.add_controller(secondary_click);
        browser.tiles.insert(&button, -1);
    }
    if browser.selected_paths.borrow().is_empty() {
        browser.preview_heading.set_text(&caption);
        browser.preview_icon.set_icon_name(Some("folder"));
        browser.preview_icon.set_visible(true);
        browser.preview_image.set_visible(false);
        browser.preview_image.set_filename(None::<&Path>);
        browser.preview.set_text(&format!(
            "Cartella · {} elementi\nPosizione: {}", count, dir.display()
        ));
    }
    browser.status.set_text(&format!("{count} elementi visibili · Melania Files"));
}

// Keep a bounded navigation history, independent of the directory renderer.
fn navigate(browser: &Browser, target: PathBuf, keep_history: bool) {
    if !target.is_dir() {
        browser.status.set_text(&format!("Non è una cartella: {}", target.display()));
        return;
    }
    let previous = browser.current.borrow().clone();
    if target == previous { return; }
    if keep_history {
        let mut history = browser.back.borrow_mut();
        history.push(previous);
        if history.len() > 100 { history.remove(0); }
        browser.forward.borrow_mut().clear();
    }
    *browser.current.borrow_mut() = target;
    *browser.selected.borrow_mut() = None;
    browser.selected_paths.borrow_mut().clear();
    *browser.selection_anchor.borrow_mut() = None;
    browser.preview.set_text("Seleziona un file per visualizzarne i dettagli.");
    browser.preview_image.set_visible(false);
    browser.preview_image.set_filename(None::<&std::path::Path>);
    browser.search.set_text("");
    refresh(browser);
    watch_directory(browser);
}

/// The GTK icon theme resolves these to Melania's compact 16–24 px
/// vector navigation icons (with the distro icon theme as a fallback).
fn navigation_icon_button(icon_name: &str, tooltip: &str) -> Button {
    let button=Button::new();
    button.add_css_class("files-nav-button");
    let image=Image::from_icon_name(icon_name);
    image.set_pixel_size(20);
    button.set_child(Some(&image));
    button.set_tooltip_text(Some(tooltip));
    button
}

/// Compact command button with icon and label; actions keep their existing
/// handlers rather than cloning transfer/rename/trash logic.
fn action_icon_button(title: &str, icon_name: &str, tooltip: &str) -> Button {
    let button = Button::new();
    button.add_css_class("files-command");
    button.set_tooltip_text(Some(tooltip));
    let content = GtkBox::new(Orientation::Horizontal, 6);
    let icon = Image::from_icon_name(icon_name);
    icon.set_pixel_size(19);
    content.append(&icon);
    content.append(&Label::new(Some(title)));
    button.set_child(Some(&content));
    button
}

fn navigation_button(label: &str, target: PathBuf, browser: &Browser) -> Button {
    let button = Button::new();
    let row = GtkBox::new(Orientation::Horizontal, 9);
    // Use the compact, purpose-built symbolic icons in Melania Files' sidebar.
    // Detailed PNGs remain in the main file grid at larger sizes.
    let icon_name = if label.contains("Home") { "user-home-symbolic" }
        else if label.contains("Scrivania") { "user-home-symbolic" }
        else if label.contains("Documenti") { "folder-documents-symbolic" }
        else if label.contains("Immagini") { "folder-pictures-symbolic" }
        else if label.contains("Download") { "folder-download-symbolic" }
        else if label.contains("Musica") { "folder-music-symbolic" }
        else if label.contains("Video") { "folder-videos-symbolic" }
        else if label.contains("File system") { "drive-harddisk-symbolic" }
        else { "user-bookmarks-symbolic" };
    let icon = Image::from_icon_name(icon_name);
    icon.set_pixel_size(22);
    row.append(&icon);
    let text = label.split_once("  ").map(|(_,name)|name).unwrap_or(label);
    let caption = Label::new(Some(text));
    caption.set_xalign(0.0);
    row.append(&caption);
    button.set_child(Some(&row));
    let state = browser.clone();
    let target_for_navigation = target.clone();
    button.connect_clicked(move |_| {
        navigate(&state, target_for_navigation.clone(), true);
    });
    if target.is_dir() { attach_drop(&button, Some(target), browser.clone()); }
    button
}

fn repaint_favorites(browser: &Browser) {
    let container = browser.favorites_box.clone();
    while let Some(child) = container.first_child() { container.remove(&child); }
    let label = Label::new(Some("PREFERITI"));
    label.add_css_class("files-sidebar-heading");
    label.set_xalign(0.0);
    container.append(&label);
    for path in browser.bookmarks.borrow().paths().iter() {
        if !path.is_dir() { continue; }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        container.append(&navigation_button(&format!("☆  {name}"), path.clone(), browser));
    }
}

fn file_tab(start_dir: PathBuf, shared_pending: Rc<RefCell<Option<PendingTransfer>>>, tab_title: Label) -> GtkBox {
    let root = GtkBox::new(Orientation::Vertical, 0);
    let toolbar = GtkBox::new(Orientation::Horizontal, 9);
    toolbar.add_css_class("files-toolbar");
    // The application icon belongs to the top-level shell, not the location bar.
    let back = navigation_icon_button("go-previous-symbolic", "Indietro (Alt+←)");
    let forward = navigation_icon_button("go-next-symbolic", "Avanti (Alt+→)");
    let up = navigation_icon_button("go-up-symbolic", "Cartella superiore (Alt+↑)");
    let home_button = navigation_icon_button("go-home-symbolic", "Vai alla cartella personale");
    let refresh_button = navigation_icon_button("view-refresh-symbolic", "Aggiorna (F5)");
    toolbar.append(&back);
    toolbar.append(&forward);
    toolbar.append(&up);
    toolbar.append(&home_button);
    toolbar.append(&refresh_button);
    let sidebar_toggle = navigation_icon_button("view-sidebar-symbolic", "Mostra o nascondi la barra laterale (Ctrl+B)");
    sidebar_toggle.set_tooltip_text(Some("Mostra o nascondi la barra laterale (Ctrl+B)"));
    toolbar.append(&sidebar_toggle);
    // Nautilus' Ctrl+L location entry: real, local filesystem paths only.
    // GIO URI navigation needs a separate backend migration.
    let location = Entry::builder().placeholder_text("Percorso della cartella (Ctrl+L)").build();
    location.set_hexpand(true);
    location.set_tooltip_text(Some("Digita un percorso assoluto, ~/cartella o un percorso relativo"));
    toolbar.append(&location);
    let search = Entry::builder().placeholder_text("Cerca in questa cartella...").build();
    search.set_width_chars(19);
    toolbar.append(&search);
    // Native counterparts of the executable UX Lab's view/inspector controls.
    // They are ordinary GTK widgets; no custom compositor support required.
    let list_view_toggle = gtk::ToggleButton::with_label("Elenco");
    list_view_toggle.set_active(true);
    list_view_toggle.set_tooltip_text(Some("Elenco dettagliato (attivo) o griglia con icone"));

    let inspector_toggle = gtk::ToggleButton::with_label("Dettagli");
    inspector_toggle.set_active(true);
    inspector_toggle.set_tooltip_text(Some("Mostra o nasconde il pannello dettagli"));

    let hidden_toggle = CheckButton::with_label("Nascosti");
    hidden_toggle.set_tooltip_text(Some("Mostra file nascosti (nomi che iniziano con punto)"));

    let images_toggle = CheckButton::with_label("Foto");
    images_toggle.set_tooltip_text(Some("Mostra solo immagini e cartelle"));

    let sort = gtk::DropDown::from_strings(&["Nome", "Data", "Dimensione", "Tipo"]);
    sort.set_tooltip_text(Some("Ordina elementi, mantenendo le cartelle all'inizio"));

    let recursive_search_button = Button::with_label("Cerca nelle sottocartelle…");
    recursive_search_button.set_tooltip_text(Some("Ricerca per nome fino a 150 risultati / 120 cartelle; non segue collegamenti"));
    let reverse = CheckButton::with_label("Inverti");

    let mark = Button::with_label("☆ Preferito");
    mark.set_tooltip_text(Some("Aggiungi la cartella corrente ai preferiti"));

    let unmark = Button::with_label("− Preferito");
    unmark.set_tooltip_text(Some("Rimuovi la cartella corrente dai preferiti"));

    // Modern single-row command ribbon. Popovers retain ALL existing actions
    // while keeping the main toolbar readable, including on narrow windows.
    let actions = GtkBox::new(Orientation::Horizontal, 3);
    actions.add_css_class("files-actions");
    let actions_scroll = ScrolledWindow::new();
    actions_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    actions_scroll.set_child(Some(&actions));

    let new_menu = gtk::MenuButton::new();
    new_menu.add_css_class("files-command");
    new_menu.set_tooltip_text(Some("Crea una cartella, un file vuoto o da modello"));
    let new_label = GtkBox::new(Orientation::Horizontal, 6);
    new_label.append(&Image::from_icon_name("list-add-symbolic"));
    new_label.append(&Label::new(Some("Nuovo ▾")));
    new_menu.set_child(Some(&new_label));
    let new_choices = GtkBox::new(Orientation::Vertical, 6);
    new_choices.add_css_class("files-command-menu");
    let new_popover = gtk::Popover::new();
    new_popover.set_child(Some(&new_choices));
    new_menu.set_popover(Some(&new_popover));
    actions.append(&new_menu);

    let sort_menu = gtk::MenuButton::new();
    sort_menu.add_css_class("files-command");
    sort_menu.set_tooltip_text(Some("Ordina i file"));
    let sort_label = GtkBox::new(Orientation::Horizontal, 5);
    sort_label.append(&Image::from_icon_name("view-sort-ascending-symbolic"));
    sort_label.append(&Label::new(Some("Ordina ▾")));
    sort_menu.set_child(Some(&sort_label));
    let sort_choices = GtkBox::new(Orientation::Vertical, 7);
    sort_choices.add_css_class("files-command-menu");
    let sort_popover = gtk::Popover::new();
    sort_popover.set_child(Some(&sort_choices));
    sort_menu.set_popover(Some(&sort_popover));

    let view_menu = gtk::MenuButton::new();
    view_menu.add_css_class("files-command");
    view_menu.set_tooltip_text(Some("Visualizzazione e dettagli"));
    let view_label = GtkBox::new(Orientation::Horizontal, 5);
    view_label.append(&Image::from_icon_name("view-list-symbolic"));
    view_label.append(&Label::new(Some("Visualizza ▾")));
    view_menu.set_child(Some(&view_label));
    let view_choices = GtkBox::new(Orientation::Vertical, 7);
    view_choices.add_css_class("files-command-menu");
    let view_popover = gtk::Popover::new();
    view_popover.set_child(Some(&view_choices));
    view_menu.set_popover(Some(&view_popover));

    let more_menu = gtk::MenuButton::new();
    more_menu.add_css_class("files-command");
    more_menu.set_tooltip_text(Some("Altre operazioni e strumenti"));
    let more_label = GtkBox::new(Orientation::Horizontal, 5);
    more_label.append(&Image::from_icon_name("view-more-symbolic"));
    more_label.append(&Label::new(Some("Altro ▾")));
    more_menu.set_child(Some(&more_label));
    let other_choices = GtkBox::new(Orientation::Vertical, 6);
    other_choices.add_css_class("files-command-menu");
    let other_popover = gtk::Popover::new();
    other_popover.set_child(Some(&other_choices));
    more_menu.set_popover(Some(&other_popover));
    let new_folder_name=Entry::builder().placeholder_text("Nome nuova cartella").build();
    new_folder_name.set_width_chars(22);
    let create_blank=Button::with_label("+ File vuoto");
    let new_file_name=Entry::builder().placeholder_text("Nome nuovo documento").build();
    new_file_name.set_width_chars(20);
    let create_folder=Button::with_label("+ Nuova cartella");
    create_folder.add_css_class("files-create-button");
    new_choices.append(&Label::new(Some("Nuova cartella")));
    new_choices.append(&new_folder_name);
    new_choices.append(&create_folder);
    new_choices.append(&gtk::Separator::new(Orientation::Horizontal));
    new_choices.append(&Label::new(Some("Nuovo file vuoto")));
    new_choices.append(&new_file_name);
    new_choices.append(&create_blank);
    sort_choices.append(&sort);
    sort_choices.append(&reverse);
    view_choices.append(&list_view_toggle);
    view_choices.append(&inspector_toggle);
    view_choices.append(&hidden_toggle);
    view_choices.append(&images_toggle);
    other_choices.append(&recursive_search_button);
    other_choices.append(&mark);
    other_choices.append(&unmark);
    root.append(&actions_scroll);
    root.append(&toolbar);
    // A pair of native GTK Paned containers guarantee that the sidebar,
    // adaptive file FlowBox, and inspector never paint on top of each other.
    // Dragging separators changes the available tile grid width naturally.
    let content = gtk::Paned::new(Orientation::Horizontal);
    content.set_vexpand(true);
    content.add_css_class("files-three-pane");
    content.set_wide_handle(true);
    let sidebar = GtkBox::new(Orientation::Vertical, 8);
    sidebar.set_size_request(178, -1);
    sidebar.add_css_class("files-sidebar");
    let scroll = ScrolledWindow::new();
    scroll.set_hexpand(true);
    scroll.set_vexpand(true);
    scroll.add_css_class("files-grid");
    let tiles = FlowBox::new();
    // Start in the approved column-based list. Grid remains selectable.
    tiles.set_max_children_per_line(1);
    tiles.set_min_children_per_line(1);
    tiles.set_column_spacing(10);
    tiles.set_row_spacing(10);
    tiles.set_selection_mode(gtk::SelectionMode::None);
    tiles.set_valign(Align::Start);
    tiles.add_css_class("files-list-view");
    let list_header = GtkBox::new(Orientation::Horizontal, 9);
    list_header.add_css_class("files-list-header");
    // Column headers are functional sort controls, not decorative labels.
    // A second click reverses the selected ordering. Folder-first is retained.
    for (caption, sort_index, width, expand) in [
        ("Nome", 0, 0, true),
        ("Dimensione", 2, 84, false),
        ("Tipo", 3, 124, false),
        ("Ultima modifica", 1, 132, false),
    ] {
        let control=Button::new();
        control.add_css_class("files-list-sort");
        control.set_tooltip_text(Some("Ordina per questa colonna; premi di nuovo per invertire"));
        control.set_hexpand(expand);
        let heading = list_heading_label(caption,width,expand);
        control.set_child(Some(&heading));
        let picker = sort.clone();
        let reverse_button = reverse.clone();
        control.connect_clicked(move |_| {
            if picker.selected() == sort_index {
                reverse_button.set_active(!reverse_button.is_active());
            } else {
                reverse_button.set_active(false);
                picker.set_selected(sort_index);
            }
        });
        list_header.append(&control);
    }
    let center_panel = GtkBox::new(Orientation::Vertical, 0);
    center_panel.set_hexpand(true);
    center_panel.set_vexpand(true);
    center_panel.append(&list_header);
    scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&tiles));
    let empty_view = GtkBox::new(Orientation::Vertical, 12);
    empty_view.add_css_class("files-empty-state");
    empty_view.set_halign(Align::Center);
    empty_view.set_valign(Align::Center);
    empty_view.set_can_target(false);
    let empty_image = Image::from_icon_name("folder");
    empty_image.set_pixel_size(72);
    empty_image.add_css_class("files-empty-image");
    empty_view.append(&empty_image);
    let empty_title = Label::new(Some("Questa cartella è vuota"));
    empty_title.add_css_class("files-empty-title");
    empty_view.append(&empty_title);
    let empty_explanation = Label::new(Some("Puoi creare una cartella o trascinare qui i tuoi file."));
    empty_explanation.add_css_class("files-empty-description");
    empty_explanation.set_wrap(true);
    empty_explanation.set_justify(gtk::Justification::Center);
    empty_view.append(&empty_explanation);
    empty_view.set_visible(false);
    let files_overlay = gtk::Overlay::new();
    files_overlay.set_child(Some(&scroll));
    files_overlay.add_overlay(&empty_view);
    files_overlay.set_hexpand(true);
    files_overlay.set_vexpand(true);
    center_panel.append(&files_overlay);
    let inspector = GtkBox::new(Orientation::Vertical, 14);
    inspector.add_css_class("files-preview");
    inspector.set_size_request(220, -1);
    let inspector_scroll = ScrolledWindow::new();
    inspector_scroll.add_css_class("files-inspector-scroll");
    inspector_scroll.set_min_content_width(220);
    inspector_scroll.set_child(Some(&inspector));
    let details_area=inspector_scroll.clone();
    inspector_toggle.connect_toggled(move |toggle| {
        details_area.set_visible(toggle.is_active());
    });
    let preview_icon = Image::from_icon_name("folder");
    preview_icon.set_pixel_size(76);
    preview_icon.set_halign(Align::Center);
    preview_icon.add_css_class("files-preview-folder-icon");
    inspector.append(&preview_icon);
    let preview_heading = Label::new(Some("Home"));
    preview_heading.set_xalign(0.0);
    preview_heading.set_wrap(true);
    preview_heading.add_css_class("files-preview-heading");
    inspector.append(&preview_heading);
    let title = Label::new(Some("Dettagli"));
    title.add_css_class("files-title");
    title.set_xalign(0.0);
    inspector.append(&title);
    let preview_image=Picture::new();
    preview_image.set_size_request(190,150);
    preview_image.set_content_fit(gtk::ContentFit::Contain);
    preview_image.set_visible(false);
    preview_image.add_css_class("files-inspector-image");
    inspector.append(&preview_image);
    let preview = Label::new(Some("Seleziona un file per visualizzarne i dettagli."));
    preview.set_wrap(true);
    preview.set_xalign(0.0);
    preview.add_css_class("files-info");
    inspector.append(&preview);
    let properties_button = Button::with_label("Proprietà del file");
    inspector.append(&properties_button);
    let open = Button::with_label("Apri con l'app predefinita");
    inspector.append(&open);
    let rename_name = Entry::builder().placeholder_text("Nuovo nome del file").build();
    let rename_button = Button::with_label("Rinomina senza sovrascrivere");
    let rename_menu = gtk::MenuButton::new();
    rename_menu.add_css_class("files-command");
    rename_menu.set_tooltip_text(Some("Rinomina i file selezionati senza sovrascrivere"));
    let rename_caption = GtkBox::new(Orientation::Horizontal, 5);
    rename_caption.append(&Image::from_icon_name("edit-select-all-symbolic"));
    rename_caption.append(&Label::new(Some("Rinomina")));
    rename_menu.set_child(Some(&rename_caption));
    let rename_choices = GtkBox::new(Orientation::Vertical, 6);
    rename_choices.add_css_class("files-command-menu");
    rename_choices.append(&Label::new(Some("Nuovo nome")));
    rename_choices.append(&rename_name);
    rename_choices.append(&rename_button);
    let rename_popover = gtk::Popover::new();
    rename_popover.set_child(Some(&rename_choices));
    rename_menu.set_popover(Some(&rename_popover));
    // Transfer safety explanation belongs under "Altro", not permanently
    // occupying the preview pane. The underlying safe operations do not change.
    let transfer_label = Label::new(Some("Trasferimenti locali sicuri"));
    transfer_label.set_xalign(0.0);
    transfer_label.add_css_class("files-title");
    other_choices.append(&gtk::Separator::new(Orientation::Horizontal));
    other_choices.append(&transfer_label);
    let copy_button = action_icon_button("Copia", "edit-copy-symbolic", "Copia la selezione");
    let cut_button = action_icon_button("Taglia", "edit-cut-symbolic", "Taglia la selezione");
    let paste_button = action_icon_button("Incolla", "edit-paste-symbolic", "Incolla nella cartella");
    let trash_button = action_icon_button("Elimina", "user-trash-symbolic", "Sposta nel Cestino");
    trash_button.add_css_class("files-trash");
    trash_button.set_tooltip_text(Some("Elimina in modo recuperabile tramite il Cestino del sistema"));
    // File operations remain wired to the same handlers; surface them in the
    // adaptive actions bar rather than hiding them behind the inspector.
    for button in [&cut_button, &copy_button, &paste_button] {
        actions.append(button);
    }
    actions.append(&rename_menu);
    actions.append(&trash_button);
    actions.append(&sort_menu);
    actions.append(&view_menu);
    actions.append(&more_menu);
    let transfer_note = Label::new(Some("Copia pubblica file negli appunti GTK del sistema. Taglia resta interno a Melania e sposta solo sul filesystem locale, senza sovrascritture."));
    transfer_note.set_wrap(true);
    transfer_note.set_xalign(0.0);
    transfer_note.add_css_class("files-info");
    other_choices.append(&transfer_note);
    let middle_and_inspector = gtk::Paned::new(Orientation::Horizontal);
    middle_and_inspector.set_wide_handle(true);
    middle_and_inspector.set_start_child(Some(&center_panel));
    middle_and_inspector.set_end_child(Some(&inspector_scroll));
    middle_and_inspector.set_position(690);
    let sidebar_scroll = ScrolledWindow::new();
    sidebar_scroll.set_min_content_width(178);
    sidebar_scroll.set_child(Some(&sidebar));
    let sidebar_to_toggle = sidebar_scroll.clone();
    sidebar_toggle.connect_clicked(move |_| {
        sidebar_to_toggle.set_visible(!sidebar_to_toggle.is_visible());
    });
    content.set_start_child(Some(&sidebar_scroll));
    content.set_end_child(Some(&middle_and_inspector));
    content.set_position(190);
    root.append(&content);
    let status = Label::new(None);
    status.add_css_class("files-footer");
    status.set_xalign(0.0);
    root.append(&status);
    let progress_row=GtkBox::new(Orientation::Horizontal, 8);
    let transfer_progress=gtk::ProgressBar::new();
    transfer_progress.set_hexpand(true);
    transfer_progress.set_show_text(true);
    transfer_progress.set_visible(false);
    let transfer_cancel=Button::with_label("Annulla copia");
    transfer_cancel.set_visible(false);
    progress_row.append(&transfer_progress);
    progress_row.append(&transfer_cancel);
    root.append(&progress_row);
    let active_job: Rc<RefCell<Option<Arc<AtomicBool>>>> = Rc::new(RefCell::new(None));
    let cancel_state=active_job.clone();
    transfer_cancel.connect_clicked(move |_| {
        if let Some(job)=cancel_state.borrow().as_ref() {
            job.store(true, Ordering::Release);
        }
    });
    let browser = Browser {
        current: Rc::new(RefCell::new(start_dir)), selected: Rc::new(RefCell::new(None)),
        selected_paths: Rc::new(RefCell::new(BTreeSet::new())),
        selection_anchor: Rc::new(RefCell::new(None)),
        visible_paths: Rc::new(RefCell::new(Vec::new())),
        tile_buttons: Rc::new(RefCell::new(Vec::new())),
        tiles, empty_view, empty_icon: empty_image, empty_title, empty_explanation,
        list_mode: Rc::new(Cell::new(true)), list_header: list_header.clone(),
        preview_icon, preview_heading, location, status, preview, preview_image, search,
        back: Rc::new(RefCell::new(Vec::new())),
        forward: Rc::new(RefCell::new(Vec::new())),
        show_hidden: Rc::new(Cell::new(false)),
        only_images: Rc::new(Cell::new(false)),
        pending: shared_pending,
        sort_by: Rc::new(Cell::new(model::SortBy::Name)),
        descending: Rc::new(Cell::new(false)),
        favorites_box: GtkBox::new(Orientation::Vertical, 5),
        bookmarks: Rc::new(RefCell::new(model::bookmarks_path()
            .ok().and_then(|p| model::Bookmarks::read_from(&p).ok()).unwrap_or_default())),
        tab_title,
        highlighted: Rc::new(RefCell::new(None)),
        monitor: Rc::new(RefCell::new(None)),
        refresh_pending: Rc::new(Cell::new(false)),
        mounts_box: GtkBox::new(Orientation::Vertical, 6),
        copy_action: copy_button.clone(), cut_action: cut_button.clone(),
        trash_action: trash_button.clone(), rename_field: rename_name.clone(),
        transfer_progress, transfer_cancel, active_job,
    };
    let view_state = browser.clone();
    let view_rows = browser.tiles.clone();
    list_view_toggle.connect_toggled(move |toggle| {
        let mode = toggle.is_active();
        view_state.list_mode.set(mode);
        view_state.list_header.set_visible(mode);
        view_rows.set_max_children_per_line(if mode {1} else {6});
        if mode {
            view_rows.add_css_class("files-list-view");
        } else {
            view_rows.remove_css_class("files-list-view");
        }
        refresh(&view_state);
    });
    // User templates: copies are always create_new, so an existing document
    // cannot be replaced. Only regular files, never symlink templates.
    let template_menu = gtk::MenuButton::new();
    template_menu.set_label("Da modello");
    template_menu.set_tooltip_text(Some("Crea un nuovo file da ~/Modelli / ~/Templates"));
    let template_popover = gtk::Popover::new();
    let template_choices = GtkBox::new(Orientation::Vertical, 4);
    template_choices.append(&Label::new(Some("MODELLI")));
    let directory = xdg_user_directory("TEMPLATES", "Templates");
    let mut templates: Vec<PathBuf> = fs::read_dir(&directory).ok().into_iter()
        .flat_map(|iter| iter.filter_map(Result::ok))
        .map(|entry| entry.path())
        .filter(|path| fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file()))
        .take(40).collect();
    templates.sort();
    if templates.is_empty() {
        template_choices.append(&Label::new(Some("Nessun modello trovato")));
    }
    for source in templates {
        let name = source.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let choice = Button::with_label(&name);
        let state = browser.clone();
        choice.connect_clicked(move |_| {
            let Some(filename) = source.file_name() else { return; };
            let target = state.current.borrow().join(filename);
            match nautilus::create_from_template(&source, &target) {
                Ok(()) => { refresh(&state); state.status.set_text("File creato dal modello, senza sovrascritture."); },
                Err(e) => state.status.set_text(&format!("Modello non applicato: {e}")),
            }
        });
        template_choices.append(&choice);
    }
    template_popover.set_child(Some(&template_choices));
    template_menu.set_popover(Some(&template_popover));
    new_choices.append(&template_menu);
    for (label, dir) in [
        ("⌂  Home", home()),
        ("▤  Scrivania", xdg_user_directory("DESKTOP", "Desktop")),
        ("▤  Documenti", xdg_user_directory("DOCUMENTS", "Documents")),
        ("▣  Immagini", xdg_user_directory("PICTURES", "Pictures")),
        ("⇩  Download", xdg_user_directory("DOWNLOAD", "Downloads")),
        ("♫  Musica", xdg_user_directory("MUSIC", "Music")),
        ("▸  Video", xdg_user_directory("VIDEOS", "Videos")),
        ("◈  File system", PathBuf::from("/")),
    ] {
        sidebar.append(&navigation_button(label, dir, &browser));
    }
    let open_trash = Button::new();
    let trash_row = GtkBox::new(Orientation::Horizontal,9);
    let trash_icon = Image::from_icon_name("user-trash-symbolic");
    trash_icon.set_pixel_size(22);
    trash_row.append(&trash_icon);
    trash_row.append(&Label::new(Some("Cestino")));
    open_trash.set_child(Some(&trash_row));
    open_trash.set_tooltip_text(Some("Apri il Cestino tramite il file manager configurato"));
    open_trash.connect_clicked(|_| {
        if let Err(e) = Command::new("xdg-open").arg("trash:///").spawn() {
            eprintln!("Melania Files: impossibile aprire il Cestino: {e}");
        }
    });
    sidebar.append(&open_trash);
    sidebar.append(&browser.mounts_box);
    repaint_volumes(&browser);
    let volumes = gio::VolumeMonitor::get();
    let on_mount = browser.clone();
    volumes.connect_mount_added(move |_, _| repaint_volumes(&on_mount));
    let on_unmount = browser.clone();
    volumes.connect_mount_removed(move |_, _| repaint_volumes(&on_unmount));
    sidebar.append(&browser.favorites_box);
    repaint_favorites(&browser);
    let state=browser.clone();
    browser.location.connect_activate(move |entry| {
        let typed = entry.text().to_string();
        let typed = typed.trim();
        let target = if let Some(tail) = typed.strip_prefix("~/") { home().join(tail) }
            else if typed.starts_with('/') { PathBuf::from(typed) }
            else { state.current.borrow().join(typed) };
        if target.is_dir() { navigate(&state, target, true); }
        else { state.status.set_text("Percorso non valido o non locale: nessuna navigazione effettuata."); }
    });
    let state=browser.clone();
    create_blank.connect_clicked(move |_| {
        let name = new_file_name.text().trim().to_string();
        if name.is_empty() || matches!(name.as_str(), "."|"..") || name.contains('/') || name.contains('\0') {
            state.status.set_text("Inserisci un nome valido per il nuovo file."); return;
        }
        let target = state.current.borrow().join(&name);
        match fs::OpenOptions::new().write(true).create_new(true).open(&target) {
            Ok(_)=> { refresh(&state); state.status.set_text(&format!("Documento creato: {name}")); },
            Err(e)=>state.status.set_text(&format!("File non creato: {e}")),
        }
    });
    let state=browser.clone();
    create_folder.connect_clicked(move |_| {
        let name=new_folder_name.text().to_string();
        let name=name.trim();
        if name.is_empty() || matches!(name,"."|"..") || name.contains('/') || name.contains('\0') {
            state.status.set_text("Inserisci un nome semplice, senza separatori di percorso.");
            return;
        }
        let target=state.current.borrow().join(name);
        match fs::create_dir(&target) {
            Ok(())=>{ refresh(&state); state.status.set_text(&format!("Cartella creata: {name}")); },
            Err(err)=>state.status.set_text(&format!("Creazione annullata: {err}")),
        }
    });
    let state=browser.clone();
    recursive_search_button.connect_clicked(move |_| {
        let query = state.search.text().to_string();
        let query = query.trim();
        if query.is_empty() {
            state.status.set_text("Scrivi prima il nome da cercare nella casella di ricerca."); return;
        }
        let dir = state.current.borrow().clone();
        if dir == Path::new("/") {
            state.status.set_text("Per la ricerca ricorsiva scegli una cartella più specifica."); return;
        }
        // Intentionally bounded. A future revision must move this work into
        // a cancellable background worker for large or remote directories.
        let results = nautilus::search_tree(&dir, query, nautilus::SearchLimits {
            max_results: 150, max_directories: 120, show_hidden: state.show_hidden.get()
        });
        match results {
            Ok(result) => {
                let window = gtk::Window::builder().title("Ricerca in sottocartelle — Melania Files")
                    .default_width(560).default_height(480).build();
                let outer = GtkBox::new(Orientation::Vertical, 8);
                outer.set_margin_top(12); outer.set_margin_start(12);
                outer.set_margin_end(12); outer.set_margin_bottom(12);
                let summary = Label::new(Some(&format!("{} risultati ({} cartelle esaminate){}",
                    result.paths.len(), result.directories_scanned,
                    if result.truncated { " · Limite raggiunto" } else { "" })));
                summary.set_xalign(0.0); outer.append(&summary);
                let scroll = ScrolledWindow::new(); scroll.set_vexpand(true);
                let list = GtkBox::new(Orientation::Vertical, 3);
                for path in result.paths {
                    let item = Button::with_label(&path.display().to_string());
                    let state = state.clone(); let target = path.clone();
                    item.connect_clicked(move |_| {
                        if target.is_dir() { navigate(&state, target.clone(), true); }
                        else if let Some(parent) = target.parent() { navigate(&state, parent.to_path_buf(), true); }
                    });
                    list.append(&item);
                }
                scroll.set_child(Some(&list)); outer.append(&scroll);
                window.set_child(Some(&outer)); window.present();
            },
            Err(e) => state.status.set_text(&format!("Ricerca non riuscita: {e}")),
        }
    });
    let state=browser.clone();
    sort.connect_selected_notify(move |sort| {
        state.sort_by.set(model::SortBy::from_index(sort.selected()));
        refresh(&state);
    });
    let state=browser.clone();
    reverse.connect_toggled(move |toggle| {
        state.descending.set(toggle.is_active());
        refresh(&state);
    });
    let state=browser.clone();
    mark.connect_clicked(move |_| {
        let folder=state.current.borrow().clone();
        let result = {
            let mut bookmarks=state.bookmarks.borrow_mut();
            bookmarks.add(&folder).and_then(|_| model::bookmarks_path().and_then(|p| bookmarks.write_to(&p)))
        };
        match result {
            Ok(())=> { repaint_favorites(&state); state.status.set_text("Cartella salvata nei preferiti."); },
            Err(err)=>state.status.set_text(&format!("Impossibile salvare preferito: {err}")),
        }
    });
    let state=browser.clone();
    unmark.connect_clicked(move |_| {
        let folder=state.current.borrow().canonicalize();
        let result=folder.and_then(|folder| {
            let mut bookmarks=state.bookmarks.borrow_mut();
            bookmarks.remove(&folder);
            model::bookmarks_path().and_then(|p| bookmarks.write_to(&p))
        });
        match result {
            Ok(())=> { repaint_favorites(&state); state.status.set_text("Preferiti aggiornati."); },
            Err(err)=>state.status.set_text(&format!("Impossibile aggiornare preferiti: {err}")),
        }
    });
    let state=browser.clone();
    home_button.connect_clicked(move |_| navigate(&state,home(),true));
    let state=browser.clone();
    refresh_button.connect_clicked(move |_| refresh(&state));
    let state = browser.clone();
    up.connect_clicked(move |_| {
        let parent = state.current.borrow().parent().map(Path::to_path_buf);
        if let Some(parent) = parent {
            navigate(&state, parent, true);
        }
    });
    let state = browser.clone();
    back.connect_clicked(move |_| {
        let previous = state.back.borrow_mut().pop();
        if let Some(previous) = previous {
            state.forward.borrow_mut().push(state.current.borrow().clone());
            navigate(&state, previous, false);
        }
    });
    let state = browser.clone();
    forward.connect_clicked(move |_| {
        let next = state.forward.borrow_mut().pop();
        if let Some(next) = next {
            state.back.borrow_mut().push(state.current.borrow().clone());
            navigate(&state, next, false);
        }
    });
    let state = browser.clone();
    hidden_toggle.connect_toggled(move |toggle| {
        state.show_hidden.set(toggle.is_active());
        refresh(&state);
    });
    let state = browser.clone();
    images_toggle.connect_toggled(move |toggle| {
        state.only_images.set(toggle.is_active());
        refresh(&state);
    });
    let state = browser.clone();
    browser.search.connect_changed(move |_| refresh(&state));
    let state=browser.clone();
    properties_button.connect_clicked(move |_| {
        if let Some(path) = state.selected.borrow().as_ref() { show_properties(path); }
        else { state.status.set_text("Seleziona un elemento per visualizzarne le proprietà."); }
    });
    let state=browser.clone();
    let rename_name_for_click = rename_name.clone();
    rename_button.connect_clicked(move |_| {
        let Some(old)=state.selected.borrow().clone() else {
            state.status.set_text("Seleziona un file prima di rinominarlo."); return;
        };
        match transfer::rename_local(&old, &rename_name_for_click.text()) {
            Ok(updated)=> {
                *state.selected.borrow_mut()=Some(updated);
                refresh(&state);
                state.status.set_text("Rinomina completata senza sovrascritture.");
            },
            Err(err)=>state.status.set_text(&format!("Rinomina annullata: {err}")),
        }
    });
    let selected = browser.selected.clone();
    open.connect_clicked(move |_| {
        if let Some(path) = selected.borrow().as_ref() {
            if let Err(err) = Command::new("xdg-open").arg(path).spawn() {
                eprintln!("Melania Files: impossibile aprire {}: {err}", path.display());
            }
        }
    });
    // Use the display clipboard for interoperable COPY. Internal CUT is deliberately
    // separate: we never advertise a MOVE to an external application until the
    // Wayland/XWayland negotiation and source confirmation are tested end-to-end.
    let state=browser.clone();
    copy_button.connect_clicked(move |_| {
        let paths: Vec<PathBuf> = state.selected_paths.borrow().iter().cloned().collect();
        if paths.is_empty() {
            state.status.set_text("Seleziona uno o più file prima di copiarli.");return;
        }
        let Some(display)=gdk::Display::default() else {
            state.status.set_text("Display grafico non disponibile.");return;
        };
        let files: Vec<gio::File> = paths.iter().map(gio::File::for_path).collect();
        let list=gdk::FileList::from_array(&files);
        let provider=gdk::ContentProvider::for_value(&list.to_value());
        if display.clipboard().set_content(Some(&provider)).is_err() {
            state.status.set_text("Impossibile aggiornare gli appunti del sistema.");return;
        }
        *state.pending.borrow_mut()=Some(PendingTransfer{paths:paths.clone(),is_move:false});
        state.status.set_text(&format!("{} elementi copiati negli appunti del sistema.",paths.len()));
    });
    let state=browser.clone();
    cut_button.connect_clicked(move |_| {
        let paths: Vec<PathBuf> = state.selected_paths.borrow().iter().cloned().collect();
        if !paths.is_empty() {
            // Clear any previously published COPY to avoid exposing an outdated file
            // when the user's intent has changed to a Melania-only CUT.
            if let Some(display)=gdk::Display::default() { let _ = display.clipboard().set_content(None::<&gdk::ContentProvider>); }
            *state.pending.borrow_mut()=Some(PendingTransfer{paths,is_move:true});
            state.status.set_text("Taglia pronto: usa Incolla in Melania Files; nessun altro programma può cancellare l'origine.");
        } else {state.status.set_text("Seleziona un elemento prima di tagliarlo.");}
    });
    let state=browser.clone();
    paste_button.connect_clicked(move |_| {
        let target=state.current.borrow().clone();
        // Local cut remains atomic and no-overwrite. Copy reads the *current*
        // system clipboard, preventing stale internal selections from winning
        // over a file list copied by another application in the meantime.
        let pending=state.pending.borrow().clone();
        if let Some(pending)=pending.filter(|p|p.is_move) {
            // Atomic Linux no-replace per entry. On failure completed moves
            // remain at the destination; pending retains only unmoved paths.
            let mut moved=0;
            let mut remaining=Vec::new();
            let mut failure=None;
            let mut queue=pending.paths.into_iter();
            while let Some(path)=queue.next() {
                if let Err(err)=transfer::move_local(&path,&target) {
                    failure=Some(err.to_string());
                    remaining.push(path);
                    remaining.extend(queue);
                    break;
                }
                moved+=1;
            }
            *state.pending.borrow_mut()=if remaining.is_empty() {None} else {Some(PendingTransfer{paths:remaining,is_move:true})};
            refresh(&state);
            state.status.set_text(&match failure {
                Some(err)=>format!("Spostati {moved} elementi; il resto non è stato mosso: {err}"),
                None=>format!("Spostati {moved} elementi senza sovrascritture."),
            });
            return;
        }
        let Some(display)=gdk::Display::default() else {
            state.status.set_text("Display grafico non disponibile.");return;
        };
        let state_done=state.clone();
        display.clipboard().read_value_async(
            gdk::FileList::static_type(), glib::Priority::DEFAULT,
            None::<&gio::Cancellable>, move |result| {
                let Ok(value)=result else {
                    state_done.status.set_text("Gli appunti non contengono un elenco di file compatibile.");return;
                };
                let Ok(list)=value.get::<gdk::FileList>() else {
                    state_done.status.set_text("Formato file degli appunti non supportato.");return;
                };
                let mut sources=Vec::new();
                for file in list.files() {
                    let Some(path)=file.path() else {
                        state_done.status.set_text("Copia da URI remoti non ancora disponibile.");return;
                    };
                    sources.push(path);
                }
                if sources.is_empty() {
                    state_done.status.set_text("Nessun file negli appunti.");return;
                }
                start_background_copy(&state_done, sources, target);
            }
        );
    });
    let state=browser.clone();
    trash_button.connect_clicked(move |_| {
        let paths: Vec<PathBuf> = state.selected_paths.borrow().iter().cloned().collect();
        if paths.is_empty() {
            state.status.set_text("Seleziona uno o più file da spostare nel Cestino.");return;
        }
        // Recoverable trash only. Never fallback to permanent deletion.
        let mut moved=0;
        let mut failed=0;
        for path in paths {
            match gio::File::for_path(&path).trash(None::<&gio::Cancellable>) {
                Ok(()) => {state.selected_paths.borrow_mut().remove(&path); moved+=1;},
                Err(_) => failed+=1,
            }
        }
        *state.selected.borrow_mut()=None;
        state.preview.set_text("Cestino: gli elementi spostati possono essere ripristinati.");
        state.preview_image.set_visible(false);
        refresh(&state);
        state.status.set_text(&format!("{moved} nel Cestino; {failed} non spostati (nessuna eliminazione permanente)."));
    });
    // Per-page keyboard navigation works even when other tabs are open.
    // GTK routes the event to the focused page; back/forward histories stay isolated.
    let key_nav = gtk::EventControllerKey::new();
    let go_back = back.clone();
    let go_forward = forward.clone();
    let go_up = up.clone();
    let do_refresh = refresh_button.clone();
    let do_sidebar = sidebar_toggle.clone();
    let do_search = browser.search.clone();
    let do_location = browser.location.clone();
    let do_hidden = hidden_toggle.clone();
    let do_copy = copy_button.clone();
    let do_cut = cut_button.clone();
    let do_paste = paste_button.clone();
    let do_properties = properties_button.clone();
    let do_rename = rename_name.clone();
    let do_trash = trash_button.clone();
    let selected_for_all=browser.selected_paths.clone();
    let visible_for_all=browser.visible_paths.clone();
    let tile_buttons_for_all=browser.tile_buttons.clone();
    key_nav.connect_key_pressed(move |_, key, _, mods| {
        if mods.contains(gdk::ModifierType::ALT_MASK) {
            if key == gdk::Key::Left { go_back.emit_clicked(); return glib::Propagation::Stop; }
            if key == gdk::Key::Right { go_forward.emit_clicked(); return glib::Propagation::Stop; }
            if key == gdk::Key::Up { go_up.emit_clicked(); return glib::Propagation::Stop; }
        }
        if key == gdk::Key::F5 { do_refresh.emit_clicked(); return glib::Propagation::Stop; }
        if mods.contains(gdk::ModifierType::ALT_MASK) && key == gdk::Key::Return {
            do_properties.emit_clicked(); return glib::Propagation::Stop;
        }
        if key == gdk::Key::F2 { do_rename.grab_focus(); return glib::Propagation::Stop; }
        // Do not steal text editing shortcuts from the search, location or
        // rename entry. Ctrl+C/X/V there must still work on selected text.
        let typing = do_search.has_focus() || do_location.has_focus() || do_rename.has_focus();
        if mods.contains(gdk::ModifierType::CONTROL_MASK) {
            if key == gdk::Key::f { do_search.grab_focus(); return glib::Propagation::Stop; }
            if key == gdk::Key::l { do_location.grab_focus(); do_location.select_region(0,-1); return glib::Propagation::Stop; }
            if key == gdk::Key::h { do_hidden.set_active(!do_hidden.is_active()); return glib::Propagation::Stop; }
            if key == gdk::Key::b { do_sidebar.emit_clicked(); return glib::Propagation::Stop; }
            if !typing {
                if key == gdk::Key::a {
                    let mut selected=selected_for_all.borrow_mut();
                    selected.clear();
                    selected.extend(visible_for_all.borrow().iter().cloned());
                    for (path, button) in tile_buttons_for_all.borrow().iter() {
                        if selected.contains(path) { button.add_css_class("files-tile-selected"); }
                    }
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::c { do_copy.emit_clicked(); return glib::Propagation::Stop; }
                if key == gdk::Key::x { do_cut.emit_clicked(); return glib::Propagation::Stop; }
                if key == gdk::Key::v { do_paste.emit_clicked(); return glib::Propagation::Stop; }
            }
        }
        if key == gdk::Key::Delete && !typing { do_trash.emit_clicked(); return glib::Propagation::Stop; }
        glib::Propagation::Proceed
    });
    root.add_controller(key_nav);
    // Drops from GTK/Qt file lists currently negotiate COPY only.
    attach_drop(&scroll, None, browser.clone());
    refresh(&browser);
    watch_directory(&browser);
    root
}

/// The visible tabs live in the native GTK HeaderBar. The Notebook only
/// manages the pages, so there is no duplicated title or second tab bar.
fn sync_tab_highlights(
    notebook: &gtk::Notebook,
    headers: &Rc<RefCell<Vec<(GtkBox, GtkBox, Label)>>>,
    window: &ApplicationWindow,
) {
    let active = notebook.current_page();
    for (page, tab, title) in headers.borrow().iter() {
        if notebook.page_num(page) == active {
            tab.add_css_class("files-tab-active");
            // Compositor IPC and taskbar hover chooser can identify the
            // folder shown in each *window*, not just "Melania Files".
            window.set_title(Some(&format!("{} — Melania Files", title.label())));
        } else {
            tab.remove_css_class("files-tab-active");
        }
    }
}

/// Append the new tab AFTER the existing ones. "+" remains the first item.
fn append_browser_tab(
    notebook: &gtk::Notebook,
    tab_strip: &GtkBox,
    headers: &Rc<RefCell<Vec<(GtkBox, GtkBox, Label)>>>,
    window: &ApplicationWindow,
    path: PathBuf,
    pending: Rc<RefCell<Option<PendingTransfer>>>,
    title: &str,
) {
    let text = Label::new(Some(title));
    let child = file_tab(path, pending, text.clone());
    let tab = GtkBox::new(Orientation::Horizontal, 3);
    tab.add_css_class("files-top-tab");
    let tab_button = Button::new();
    tab_button.add_css_class("files-top-tab-select");
    let tab_contents = GtkBox::new(Orientation::Horizontal, 6);
    let icon = Image::from_icon_name("folder-symbolic");
    icon.set_pixel_size(16);
    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.set_max_width_chars(16);
    tab_contents.append(&icon);
    tab_contents.append(&text);
    tab_button.set_child(Some(&tab_contents));
    let close = Button::from_icon_name("window-close-symbolic");
    close.add_css_class("files-tab-close");
    close.set_tooltip_text(Some("Chiudi scheda"));
    tab.append(&tab_button);
    tab.append(&close);
    let index = notebook.append_page(&child, None::<&gtk::Widget>);
    headers.borrow_mut().push((child.clone(), tab.clone(), text.clone()));
    tab_strip.append(&tab);
    notebook.set_current_page(Some(index));
    sync_tab_highlights(notebook, headers, window);
    let tabs_for_title = notebook.clone();
    let page_for_title = child.clone();
    let window_for_title = window.clone();
    text.connect_label_notify(move |label| {
        if tabs_for_title.page_num(&page_for_title) == tabs_for_title.current_page() {
            window_for_title.set_title(Some(&format!("{} — Melania Files", label.label())));
        }
    });

    let pages = notebook.clone();
    let selected_page = child.clone();
    tab_button.connect_clicked(move |_| {
        if let Some(number) = pages.page_num(&selected_page) {
            pages.set_current_page(Some(number));
        }
    });

    let pages = notebook.clone();
    let heading_list = headers.clone();
    let strip = tab_strip.clone();
    let window_after_close = window.clone();
    close.connect_clicked(move |_| {
        // Never silently close the last browser. Ctrl+W follows the same rule.
        if pages.n_pages() <= 1 { return; }
        if let Some(index) = pages.page_num(&child) {
            if let Some(position) = heading_list.borrow().iter()
                .position(|(page, _, _)| page == &child) {
                heading_list.borrow_mut().remove(position);
            }
            strip.remove(&tab);
            pages.remove_page(Some(index));
            sync_tab_highlights(&pages, &heading_list, &window_after_close);
        }
    });
}

/// The nested Wayland compositor does not yet expose a formal panel work area.
/// Size the *initial* window to leave room for the 52 px Aero taskbar and a
/// titlebar/placement margin. It remains freely resizable/maximizable.
fn files_initial_size(screen_width: i32, screen_height: i32) -> (i32, i32) {
    let width = if screen_width > 0 {(screen_width - 56).clamp(560, 1160)} else {1100};
    let height = if screen_height > 0 {(screen_height - 145).clamp(450, 720)} else {620};
    (width,height)
}

fn build_ui(app: &Application) {
    setup_melania_icon_theme();
    let css = gtk::CssProvider::new();
    #[allow(deprecated)]
    css.load_from_data(include_str!("../assets/style.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
    let chrome_css = gtk::CssProvider::new();
    chrome_css.load_from_data(include_str!("../../branding/window-chrome-win7.css"));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &chrome_css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 2);
    }
    let screen = gdk::Display::default()
        .and_then(|display| display.monitors().item(0))
        .and_then(|object| object.downcast::<gdk::Monitor>().ok())
        .map(|monitor| monitor.geometry());
    let (initial_width, initial_height) = screen.map(|rect| {
        files_initial_size(rect.width(), rect.height())
    }).unwrap_or_else(|| files_initial_size(1280, 800));
    let window = ApplicationWindow::builder().application(app)
        .title("Melania Files")
        .default_width(initial_width).default_height(initial_height).build();
    window.add_css_class("melania-files-window");
    // One integrated native header: [+] [tabs...] [minimize/maximize/close].
    // GTK keeps the real draggable window chrome and accessible controls.
    let aero_header = gtk::HeaderBar::new();
    aero_header.add_css_class("files-aero-header");
    aero_header.set_show_title_buttons(true);
    aero_header.set_decoration_layout(Some(":minimize,maximize,close"));
    // Override GTK HeaderBar's automatic centered window-title widget:
    // the tabs ARE the titlebar, never add a duplicate app title.
    let no_title = GtkBox::new(Orientation::Horizontal, 0);
    aero_header.set_title_widget(Some(&no_title));
    let add_button = Button::from_icon_name("list-add-symbolic");
    add_button.add_css_class("files-new-tab");
    add_button.set_tooltip_text(Some("Nuova scheda (Ctrl+T)"));
    aero_header.pack_start(&add_button);
    let tab_strip = GtkBox::new(Orientation::Horizontal, 4);
    tab_strip.add_css_class("files-top-tabs");
    let tab_scroll = ScrolledWindow::new();
    tab_scroll.add_css_class("files-tab-scroll");
    tab_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    tab_scroll.set_min_content_width(160);
    tab_scroll.set_max_content_width(650);
    tab_scroll.set_child(Some(&tab_strip));
    aero_header.pack_start(&tab_scroll);
    window.set_titlebar(Some(&aero_header));
    let frame = GtkBox::new(Orientation::Vertical, 0);
    let tabs = gtk::Notebook::new();
    tabs.set_show_tabs(false);
    tabs.set_vexpand(true);
    tabs.set_show_border(false);
    tabs.add_css_class("files-notebook");
    let tab_headers: Rc<RefCell<Vec<(GtkBox, GtkBox, Label)>>> = Rc::new(RefCell::new(Vec::new()));
    let headers_for_switch = tab_headers.clone();
    let window_for_switch = window.clone();
    tabs.connect_switch_page(move |notebook, _, _| {
        sync_tab_highlights(notebook, &headers_for_switch, &window_for_switch);
    });

    let starting = std::env::args_os().nth(1).map(PathBuf::from)
        .filter(|p| p.is_dir()).unwrap_or_else(home);
    let shared_pending: Rc<RefCell<Option<PendingTransfer>>> = Rc::new(RefCell::new(None));
    let initial_title = starting.file_name().map(|s|s.to_string_lossy().into_owned())
        .filter(|s|!s.is_empty()).unwrap_or_else(||"Home".to_string());
    append_browser_tab(&tabs, &tab_strip, &tab_headers, &window, starting, shared_pending.clone(), &initial_title);
    let notebook = tabs.clone();
    let pending_for_add = shared_pending.clone();
    let header_for_add = tab_headers.clone();
    let rail_for_add = tab_strip.clone();
    let window_for_add = window.clone();
    add_button.connect_clicked(move |_| {
        append_browser_tab(&notebook, &rail_for_add, &header_for_add, &window_for_add,
            home(), pending_for_add.clone(), "Home");
    });
    frame.append(&tabs);

    // Browser keyboard conventions: Ctrl+T, Ctrl+W, Ctrl+Tab, Ctrl+Shift+Tab.
    let keys = gtk::EventControllerKey::new();
    let keys_notebook = tabs.clone();
    let keys_add = add_button.clone();
    let tab_headers_for_keys = tab_headers.clone();
    let strip_for_keys = tab_strip.clone();
    let window_for_keys = window.clone();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if !modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
            return glib::Propagation::Proceed;
        }
        if key == gdk::Key::t || key == gdk::Key::T {
            keys_add.emit_clicked();
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::w || key == gdk::Key::W {
            if keys_notebook.n_pages() > 1 {
                if let Some(index) = keys_notebook.current_page() {
                    if let Some((_, heading, _)) = tab_headers_for_keys.borrow().iter()
                        .find(|(page, _, _)| keys_notebook.page_num(page) == Some(index)).cloned() {
                        strip_for_keys.remove(&heading);
                    }
                    tab_headers_for_keys.borrow_mut()
                        .retain(|(page, _, _)| keys_notebook.page_num(page) != Some(index));
                    keys_notebook.remove_page(Some(index));
                    sync_tab_highlights(&keys_notebook, &tab_headers_for_keys, &window_for_keys);
                }
            }
            return glib::Propagation::Stop;
        }
        if key == gdk::Key::Tab || key == gdk::Key::ISO_Left_Tab {
            let count = keys_notebook.n_pages();
            if count > 1 {
                let index = keys_notebook.current_page().unwrap_or(0);
                let backwards = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
                let next = if backwards { (index + count - 1) % count } else { (index + 1) % count };
                keys_notebook.set_current_page(Some(next));
            }
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);
    window.set_child(Some(&frame));
    window.present();
}



/// Prefer Melania's freedesktop icon theme inside Melania applications.
/// During development, assets can be loaded directly from the source tree;
/// packaged installations use XDG_DATA_HOME/icons/Melania instead.
/// Does not change the icon theme of the user's other desktop sessions.
fn setup_melania_icon_theme() {
    let prefs=melania_config::load().unwrap_or_default();
    if let Some(display)=gtk::gdk::Display::default() {
        let icons=gtk::IconTheme::for_display(&display);
        for location in melania_config::bundled_icon_paths() {
            if location.is_dir() {icons.add_search_path(&location);}
        }
    }
    if let Some(settings)=gtk::Settings::default() {
        settings.set_property("gtk-icon-theme-name",
            melania_config::resolved_icon_theme(&prefs));
    }
}

#[cfg(test)]
mod empty_folder_tests {
    use super::empty_folder_message;

    #[test]
    fn distinguishes_empty_folder_from_empty_search() {
        let folder = empty_folder_message("", false, false);
        assert_eq!(folder.1, "Questa cartella è vuota");
        assert_eq!(folder.0, "folder");
        let search = empty_folder_message("rapporti", false, false);
        assert_eq!(search.1, "Nessun risultato");
        assert_eq!(search.0, "system-search");
    }

    #[test]
    fn only_pictures_filter_has_its_own_explanation() {
        let filtered = empty_folder_message("", true, true);
        assert_eq!(filtered.1, "Nessuna immagine visibile");
        assert!(filtered.2.contains("Foto"));
        let empty = empty_folder_message("", false, true);
        assert!(!empty.2.contains("nascosti"));
    }

    #[test]
    fn search_reason_takes_precedence_over_photo_filter() {
        assert_eq!(empty_folder_message("foto", true, false).1, "Nessun risultato");
    }
}

#[cfg(test)]
mod window_size_tests {
    use super::files_initial_size;
    #[test]
    fn respects_the_bottom_panel_on_small_desktops() {
        assert_eq!(files_initial_size(1280,800), (1160,655));
        assert_eq!(files_initial_size(1024,768), (968,623));
        assert_eq!(files_initial_size(1920,1080), (1160,720));
        assert_eq!(files_initial_size(0,0), (1100,620));
    }
}

fn main() -> gtk::glib::ExitCode {
    let app = Application::builder().application_id("org.melaniadesktop.Files").build();
    app.connect_activate(build_ui);
    app.run()
}
