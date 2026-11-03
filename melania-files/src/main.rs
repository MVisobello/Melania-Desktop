//! Early working filesystem browser, not a replacement for a mature file manager yet.
#[path = "../../branding/window_controls.rs"]
mod window_controls;
mod gio_backend;
mod gio_remote;
mod gio_mount;
use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use melania_transfer as transfer;
use melania_files_core as model;
use model::nautilus_features as nautilus;
use gtk::{Align, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, Entry, FlowBox, Image, Label, Orientation, Picture, ScrolledWindow};
use std::{cell::{Cell, RefCell}, collections::BTreeSet, fs, path::{Path, PathBuf}, process::Command, rc::Rc, sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}}, time::{Duration, Instant}};

static NEXT_FILES_TAB_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
struct PendingTransfer { paths: Vec<PathBuf>, is_move: bool }

/// GTK widgets stay on the UI thread; only paths and bounded RGBA cross the
/// background thumbnail channel. A tab has one active and one queued request.
struct LocalThumbnailRequest {
    generation: u64,
    targets: Vec<(PathBuf, GtkBox)>,
    cancelled: Arc<AtomicBool>,
}

const MAX_LOCAL_THUMBNAILS: usize = 48;

/// Software-rendered previews spend CPU time drawing GTK as well as decoding
/// thumbnails. Keep the normal 48-image limit on accelerated sessions; use a
/// smaller, bounded first batch with Cairo/llvmpipe (VirtualBox preview).
fn thumbnail_budget_for(renderer:Option<&str>,software:bool) -> usize {
    if renderer==Some("cairo") || software {16} else {MAX_LOCAL_THUMBNAILS}
}
fn thumbnail_budget() -> usize {
    thumbnail_budget_for(
        std::env::var("GSK_RENDERER").ok().as_deref(),
        std::env::var("LIBGL_ALWAYS_SOFTWARE").ok().as_deref()==Some("1"))
}

fn files_profile_enabled() -> bool {
    std::env::var("MELANIA_PROFILE_FILES").ok().as_deref()==Some("1")
}

/// Opt-in frame/event-loop diagnostic; reports sustained UI stalls rather than
/// making ungrounded claims about the renderer or filesystem provider.
fn install_files_ui_stall_probe() {
    if !files_profile_enabled() {return;}
    let previous=Rc::new(Cell::new(Instant::now()));
    glib::timeout_add_local(Duration::from_millis(100), move || {
        let now=Instant::now();
        let elapsed=now.duration_since(previous.replace(now));
        if elapsed>=Duration::from_millis(250) {
            eprintln!("MELANIA_FILES_UI_STALL_MS={}",elapsed.as_millis());
        }
        glib::ControlFlow::Continue
    });
}

#[derive(Clone)]
struct Browser {
    current: Rc<RefCell<PathBuf>>,
    selected: Rc<RefCell<Option<PathBuf>>>,
    selected_paths: Rc<RefCell<BTreeSet<PathBuf>>>,
    selection_anchor: Rc<RefCell<Option<PathBuf>>>,
    visible_paths: Rc<RefCell<Vec<PathBuf>>>,
    tile_buttons: Rc<RefCell<Vec<(PathBuf, Button)>>>,
    tiles: FlowBox,
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
    // Reject delayed monitor events and timers from folders already left.
    monitor_epoch: Rc<Cell<u64>>,
    // A navigation invalidates pending search-as-you-type debounce callbacks.
    search_revision: Rc<Cell<u64>>,
    suppress_search_refresh: Rc<Cell<bool>>,
    // Single coalescing filesystem worker per tab, with stale-result fencing.
    listing_generation: Rc<Cell<u64>>,
    listing_cancel: Rc<RefCell<Option<Arc<AtomicBool>>>>,
    listing_running: Rc<Cell<bool>>,
    listing_pending: Rc<RefCell<Option<ListingRequest>>>,
    thumbnail_cancel: Rc<RefCell<Option<Arc<AtomicBool>>>>,
    thumbnail_running: Rc<Cell<bool>>,
    thumbnail_pending: Rc<RefCell<Option<LocalThumbnailRequest>>>,
    mounts_box: GtkBox,
    open_remote: Rc<dyn Fn(String)>,
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
fn local_thumbnail_candidate(path:&Path)->bool{
    matches!(path.extension().and_then(|x|x.to_str())
        .unwrap_or("").to_ascii_lowercase().as_str(),"png"|"jpg"|"jpeg"|"webp")
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
        "png"|"jpg"|"jpeg"|"webp"|"gif"|"bmp"|"svg" => "image-x-generic",
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


#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LocalMutationKind { Move, Trash }

struct LocalMutationResult {
    completed: usize,
    failed: usize,
    remaining: Vec<PathBuf>,
    error: Option<String>,
    cancelled: bool,
}

/// Run filesystem mutations on a worker, never on the GTK event loop.
/// A move uses the existing no-replace primitive; trash never permanently
/// deletes, and cancellation takes effect between individual items.
fn run_local_mutation(kind: LocalMutationKind, sources: Vec<PathBuf>,
                      destination: Option<PathBuf>, cancel: &Arc<AtomicBool>) -> LocalMutationResult {
    let mut result=LocalMutationResult {
        completed:0, failed:0, remaining:Vec::new(), error:None, cancelled:false,
    };
    for (index,path) in sources.iter().enumerate() {
        if cancel.load(Ordering::Acquire) {
            result.cancelled=true;
            result.remaining.extend(sources[index..].iter().cloned());
            break;
        }
        let operation: Result<(),String> = match kind {
            LocalMutationKind::Move => transfer::move_local(path,
                destination.as_ref().expect("move requires a destination"))
                .map(|_|()).map_err(|e|e.to_string()),
            LocalMutationKind::Trash => {
                let uri=gio::File::for_path(path).uri();
                gio_backend::move_to_trash(&uri)
            }
        };
        match operation {
            Ok(())=>result.completed+=1,
            Err(error)=>{
                result.failed+=1;
                if kind==LocalMutationKind::Move {
                    result.error=Some(error);
                    result.remaining.extend(sources[index..].iter().cloned());
                    break;
                }
            }
        }
    }
    result
}

fn start_local_mutation(browser:&Browser, kind:LocalMutationKind,
                        sources:Vec<PathBuf>,destination:Option<PathBuf>) {
    if browser.active_job.borrow().is_some() {
        browser.status.set_text("Attendi o annulla l'operazione già in corso.");
        return;
    }
    if sources.is_empty(){return;}
    let snapshot=if kind==LocalMutationKind::Move{Some(sources.clone())}else{None};
    let cancel=Arc::new(AtomicBool::new(false));
    *browser.active_job.borrow_mut()=Some(cancel.clone());
    browser.transfer_progress.set_visible(true);
    browser.transfer_progress.pulse();
    browser.transfer_cancel.set_label("Annulla operazione");
    browser.transfer_cancel.set_visible(true);
    browser.status.set_text(match kind {
        LocalMutationKind::Move => "Spostamento in corso…",
        LocalMutationKind::Trash => "Spostamento nel Cestino in corso…",
    });
    let (tx,rx)=std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move ||{
        let outcome=run_local_mutation(kind,sources,destination,&cancel);
        let _=tx.send(outcome);
    });
    let state=browser.clone();
    glib::timeout_add_local(Duration::from_millis(70),move ||match rx.try_recv(){
        Ok(report)=>{
            state.active_job.borrow_mut().take();
            state.transfer_progress.set_visible(false);
            state.transfer_cancel.set_visible(false);
            state.transfer_cancel.set_label("Annulla copia");
            if let Some(original)=snapshot.as_ref(){
                // Another tab may have replaced the cut clipboard meanwhile.
                // Never overwrite that newer selection with an old job result.
                let mut pending=state.pending.borrow_mut();
                if pending.as_ref().is_some_and(|p|p.is_move && &p.paths==original){
                    *pending=if report.remaining.is_empty(){None}else{
                        Some(PendingTransfer{paths:report.remaining.clone(),is_move:true})
                    };
                }
            }
            refresh(&state);
            let message=match kind {
                LocalMutationKind::Move=>{
                    let suffix=if report.cancelled{" · Annullato dopo l'elemento in corso".to_string()}
                        else if let Some(error)=report.error{format!(" · Non completato: {error}")}
                        else {String::new()};
                    format!("Spostati {} elementi; {} restanti{}.",
                        report.completed,report.remaining.len(),suffix)
                },
                LocalMutationKind::Trash=>format!(
                    "{} nel Cestino; {} non spostati; {} non elaborati{} (nessuna eliminazione permanente).",
                    report.completed,report.failed,report.remaining.len(),
                    if report.cancelled {" · Annullato"}else{""}),
            };
            state.status.set_text(&message);
            glib::ControlFlow::Break
        },
        Err(std::sync::mpsc::TryRecvError::Empty)=>{
            state.transfer_progress.pulse();
            glib::ControlFlow::Continue
        },
        Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
            state.active_job.borrow_mut().take();
            state.transfer_progress.set_visible(false);
            state.transfer_cancel.set_visible(false);
            state.transfer_cancel.set_label("Annulla copia");
            // Do not clear the pending CUT list when a worker fails unexpectedly:
            // never assume that all sources have moved or been deleted.
            state.status.set_text("Operazione interrotta: verifica i file prima di riprovare.");
            glib::ControlFlow::Break
        },
    });
}

/// GIO supports local and remote URIs; use it for mixed clipboard payloads
/// while keeping the existing optimized local-to-local background transfer.
fn start_gio_copy_into_local(browser:&Browser, sources:Vec<String>, dest:PathBuf) {
    if browser.active_job.borrow().is_some(){
        browser.status.set_text("C'è già un trasferimento attivo.");return;
    }
    let destination=gio::File::for_path(dest).uri().to_string();
    let cancel=Arc::new(AtomicBool::new(false));
    *browser.active_job.borrow_mut()=Some(cancel.clone());
    browser.transfer_progress.set_visible(true);
    browser.transfer_progress.pulse();
    browser.transfer_cancel.set_visible(true);
    browser.status.set_text("Copia da GVfs in corso…");
    let (tx,rx)=std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move ||{
        let result=gio_backend::copy_uris(&sources,&destination,&cancel);
        let _=tx.send(result);
    });
    let state=browser.clone();
    glib::timeout_add_local(Duration::from_millis(70),move ||match rx.try_recv(){
        Ok(Ok(n))=>{
            state.active_job.borrow_mut().take();
            state.transfer_progress.set_visible(false);
            state.transfer_cancel.set_visible(false);
            refresh(&state);
            state.status.set_text(&format!("{n} elementi copiati da GIO/GVfs."));
            glib::ControlFlow::Break
        },
        Ok(Err(error))=>{
            state.active_job.borrow_mut().take();
            state.transfer_progress.set_visible(false);
            state.transfer_cancel.set_visible(false);
            refresh(&state);
            state.status.set_text(&format!("Copia non completata: {error}"));
            glib::ControlFlow::Break
        },
        Err(std::sync::mpsc::TryRecvError::Empty)=>{
            state.transfer_progress.pulse();
            glib::ControlFlow::Continue
        },
        Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
            state.active_job.borrow_mut().take();
            state.transfer_progress.set_visible(false);
            state.transfer_cancel.set_visible(false);
            state.status.set_text("Operazione GVfs interrotta; controlla la destinazione.");
            glib::ControlFlow::Break
        }
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

/// Installed system handlers for this file's MIME type; never construct
/// shell commands from filenames or desktop entries.
fn open_with_dialog(path: &Path) {
    let file = gio::File::for_path(path);
    let mime = file.query_info("standard::content-type",
        gio::FileQueryInfoFlags::NONE,None::<&gio::Cancellable>)
        .ok().and_then(|info|info.content_type());
    let window = gtk::Window::builder().title("Apri con — Melania Files")
        .default_width(420).default_height(400).build();
    let outer = GtkBox::new(Orientation::Vertical, 8);
    outer.set_margin_top(14);
    outer.set_margin_bottom(14);
    outer.set_margin_start(14);
    outer.set_margin_end(14);
    let description = Label::new(Some(&format!("Applicazioni disponibili per {}",
        path.file_name().unwrap_or_default().to_string_lossy())));
    description.set_wrap(true);
    description.set_xalign(0.0);
    outer.append(&description);
    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    let apps_box = GtkBox::new(Orientation::Vertical, 4);
    let mut listed = 0;
    let feedback = Label::new(None);
    feedback.set_xalign(0.0);
    feedback.set_wrap(true);
    if let Some(mime) = mime {
        for app in gio::AppInfo::all_for_type(mime.as_str()).into_iter()
            .filter(|app|app.should_show()).take(40) {
            let row = GtkBox::new(Orientation::Horizontal, 5);
            let button = Button::with_label(&app.display_name());
            button.set_hexpand(true);
            let selected_file = file.clone();
            let selected_window = window.clone();
            let on_error = feedback.clone();
            button.connect_clicked({
                let app = app.clone();
                move |_| {
                    match app.launch(&[selected_file.clone()],None::<&gio::AppLaunchContext>) {
                        Ok(()) => selected_window.close(),
                        Err(error) => on_error.set_text(&format!("Impossibile aprire: {error}")),
                    }
                }
            });
            row.append(&button);
            let make_default = Button::with_label("Predefinita");
            make_default.set_tooltip_text(Some("Imposta questa applicazione come predefinita per il tipo di file (modifica le associazioni utente Linux)"));
            let mime_for_default = mime.to_string();
            let status_for_default = feedback.clone();
            make_default.connect_clicked(move |_| {
                match app.set_as_default_for_type(&mime_for_default) {
                    Ok(()) => status_for_default.set_text("Applicazione predefinita aggiornata."),
                    Err(error) => status_for_default.set_text(&format!("Associazione non modificata: {error}")),
                }
            });
            row.append(&make_default);
            apps_box.append(&row);
            listed += 1;
        }
    }
    if listed==0 {
        apps_box.append(&Label::new(Some("Nessuna applicazione associata a questo tipo di file.")));
    }
    scroll.set_child(Some(&apps_box));
    outer.append(&scroll);
    outer.append(&feedback);
    let dismiss = Button::with_label("Chiudi");
    let dismiss_window = window.clone();
    dismiss.connect_clicked(move |_| dismiss_window.close());
    outer.append(&dismiss);
    window.set_child(Some(&outer));
    window.present();
}

#[derive(Debug)]
struct TrashedItem {
    title: String,
    uri: String,
    original: Option<PathBuf>,
}

/// Use GIO's trash:// backend so recoverability is not restricted to
/// ~/.local/share/Trash and includes supported removable-device trash.
fn read_trash_entries() -> Result<(Vec<TrashedItem>, bool), String> {
    let trash = gio::File::for_uri("trash:///");
    let enumerator = trash.enumerate_children(
        "standard::name,standard::display-name,trash::orig-path",
        gio::FileQueryInfoFlags::NONE,None::<&gio::Cancellable>
    ).map_err(|err|format!("Cestino GIO non disponibile: {err}"))?;
    let mut entries = Vec::new();
    let mut truncated = false;
    loop {
        let info = enumerator.next_file(None::<&gio::Cancellable>)
            .map_err(|err|format!("Impossibile leggere il Cestino: {err}"))?;
        let Some(info) = info else {break;};
        if entries.len() >= 300 { truncated = true;break; }
        let child = enumerator.child(&info);
        let original = info.attribute_byte_string("trash::orig-path")
            .map(|name|PathBuf::from(name.as_str()))
            .filter(|path|path.is_absolute());
        entries.push(TrashedItem {
            title: info.display_name().to_string(),
            uri: child.uri().to_string(),
            original,
        });
    }
    entries.sort_by(|a,b|a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok((entries,truncated))
}

/// One item per explicit click. No permanent delete, no overwrite and no
/// fabrication of missing original directories. GIO itself enforces the
/// destination collision check unless the OVERWRITE flag is requested.
fn restore_trash_item(uri: &str, original: &Path) -> Result<(), String> {
    if !original.is_absolute() || !original.parent().is_some_and(|p|p.is_dir()) {
        return Err("La cartella originale non esiste: ripristino non effettuato".into());
    }
    let source=gio::File::for_uri(uri);
    let destination=gio::File::for_path(original);
    source.move_(&destination, gio::FileCopyFlags::NONE,
        None::<&gio::Cancellable>, None)
        .map_err(|err|format!("Ripristino rifiutato senza sovrascrivere: {err}"))
}

/// Native, recoverable Trash listing instead of delegating trash:/// to a
/// different file manager. GIO I/O stays off the GTK thread.
fn open_trash_window(browser: &Browser) {
    let window=gtk::Window::builder().title("Cestino — Melania Files")
        .default_width(650).default_height(510).build();
    let outer=GtkBox::new(Orientation::Vertical, 9);
    outer.set_margin_top(12);
    outer.set_margin_bottom(12);
    outer.set_margin_start(12);
    outer.set_margin_end(12);
    let status=Label::new(Some("Lettura del Cestino in corso…"));
    status.set_xalign(0.0);
    status.set_wrap(true);
    outer.append(&status);
    let scroll=ScrolledWindow::new();
    scroll.set_vexpand(true);
    let items_box=GtkBox::new(Orientation::Vertical, 4);
    scroll.set_child(Some(&items_box));
    outer.append(&scroll);
    let close=Button::with_label("Chiudi");
    let close_window=window.clone();
    close.connect_clicked(move |_| close_window.close());
    outer.append(&close);
    window.set_child(Some(&outer));
    window.present();
    let (tx,rx)=std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {let _=tx.send(read_trash_entries());});
    let state=browser.clone();
    glib::timeout_add_local(Duration::from_millis(50),move || match rx.try_recv() {
        Ok(Ok((entries,truncated))) => {
            let len=entries.len();
            status.set_text(&format!("{len} elementi nel Cestino{} · Nessuna eliminazione permanente",
                if truncated {" (primi 300)"} else {""}));
            if len==0 {items_box.append(&Label::new(Some("Il Cestino è vuoto.")));}
            for item in entries {
                let row=GtkBox::new(Orientation::Horizontal, 8);
                let subtitle=match &item.original {
                    Some(p)=>format!("{}\nOrigine: {}",item.title,p.display()),
                    None=>format!("{}\nPercorso di origine non disponibile",item.title)
                };
                let name=Label::new(Some(&subtitle));
                name.set_xalign(0.0);
                name.set_wrap(true);
                name.set_hexpand(true);
                row.append(&name);
                let restore=Button::with_label("Ripristina");
                restore.set_sensitive(item.original.is_some());
                if let Some(original)=item.original {
                    let uri=item.uri.clone();
                    let dialog_status=status.clone();
                    let source_row=row.clone();
                    let list=items_box.clone();
                    let current_state=state.clone();
                    restore.connect_clicked(move |clicked| {
                        clicked.set_sensitive(false);
                        dialog_status.set_text("Ripristino in corso…");
                        let (tx,rx)=std::sync::mpsc::sync_channel(1);
                        let uri=uri.clone();
                        let original=original.clone();
                        std::thread::spawn(move || {
                            let _=tx.send(restore_trash_item(&uri,&original));
                        });
                        let status=dialog_status.clone();
                        let row=source_row.clone();
                        let list=list.clone();
                        let clicked=clicked.clone();
                        let browser=current_state.clone();
                        glib::timeout_add_local(Duration::from_millis(50),move || match rx.try_recv() {
                            Ok(Ok(()))=>{
                                list.remove(&row);
                                status.set_text("File ripristinato nel percorso originale senza sovrascritture.");
                                refresh(&browser);
                                glib::ControlFlow::Break
                            }
                            Ok(Err(message))=>{
                                clicked.set_sensitive(true);
                                status.set_text(&message);
                                glib::ControlFlow::Break
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty)=>glib::ControlFlow::Continue,
                            Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
                                status.set_text("Ripristino interrotto: verificare i file.");
                                glib::ControlFlow::Break
                            }
                        });
                    });
                } else {
                    restore.set_tooltip_text(Some("Nessun percorso originale: non è possibile ripristinare in sicurezza"));
                }
                row.append(&restore);
                items_box.append(&row);
            }
            glib::ControlFlow::Break
        }
        Ok(Err(error))=>{
            status.set_text(&error);
            glib::ControlFlow::Break
        }
        Err(std::sync::mpsc::TryRecvError::Empty)=>glib::ControlFlow::Continue,
        Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
            status.set_text("Lettura del Cestino interrotta.");
            glib::ControlFlow::Break
        }
    });
}

fn watch_directory(browser: &Browser) {
    // Cancel the old monitor and invalidate its pending debounced refreshes.
    // An event queued for folder A must not reload folder B after navigation.
    let epoch = browser.monitor_epoch.get().wrapping_add(1);
    browser.monitor_epoch.set(epoch);
    if let Some(previous) = browser.monitor.borrow_mut().take() { previous.cancel(); }
    browser.refresh_pending.set(false);
    let dir = browser.current.borrow().clone();
    if let Ok(monitor) = gio::File::for_path(&dir).monitor_directory(
        gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>) {
        let state = browser.clone();
        monitor.connect_changed(move |_, _, _, _| {
            if state.monitor_epoch.get() != epoch || *state.current.borrow() != dir { return; }
            if state.refresh_pending.replace(true) { return; }
            let update = state.clone();
            let watched = dir.clone();
            glib::timeout_add_local_once(Duration::from_millis(220), move || {
                if update.monitor_epoch.get() != epoch ||
                    *update.current.borrow() != watched { return; }
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
    // Mounted local paths and remote GVfs URIs share the sidebar. Never
    // offer unsafe forced unmount of the root filesystem.
    for mount in gio::VolumeMonitor::get().mounts() {
        let root=mount.root();
        let row=GtkBox::new(Orientation::Horizontal,5);
        let label=mount.name().to_string();
        if let Some(path)=root.path().filter(|p|p.is_dir()){
            let open=navigation_button_with_icon(&label,path,browser,Some("drive-harddisk-symbolic"));
            open.set_hexpand(true);
            row.append(&open);
        } else {
            let button=Button::with_label(&label);
            button.set_hexpand(true);
            button.set_tooltip_text(Some(&root.uri()));
            let uri=root.uri().to_string();
            let open=browser.open_remote.clone();
            button.connect_clicked(move |_|open(uri.clone()));
            row.append(&button);
        }
        if root.uri()!="file:///" && mount.can_unmount() {
            let button=Button::from_icon_name("media-eject-symbolic");
            button.set_tooltip_text(Some(&format!("Smonta {label} dopo conferma, senza forzare")));
            let active=browser.clone();
            let target=mount.clone();
            let mount_label=label.clone();
            button.connect_clicked(move |_|{
                let parent=active.mounts_box.root().and_then(|r|r.downcast::<gtk::Window>().ok());
                let selected=target.clone();
                let state=active.clone();
                let title=format!("Smontare {mount_label}?");
                gio_mount::confirm_mount_action(parent.as_ref(),&title,
                    "Chiudi i file aperti e completa eventuali copie prima di smontare. L'operazione non sarà forzata.",
                    "Smonta",move ||{
                        let parent=state.mounts_box.root().and_then(|r|r.downcast::<gtk::Window>().ok());
                        let state_done=state.clone();
                        match gio_mount::unmount(&selected,parent.as_ref(),move |result|{
                            state_done.status.set_text(match &result{
                                Ok(())=>"Volume smontato correttamente.",
                                Err(e)=>e.as_str()
                            });
                            repaint_volumes(&state_done);
                        }){
                            Ok(_)=>state.status.set_text("Smontaggio in corso…"),
                            Err(e)=>state.status.set_text(&e),
                        }
                    });
            });
            row.append(&button);
        }
        if root.uri()!="file:///" && mount.can_eject() {
            let button=Button::from_icon_name("media-removable-symbolic");
            button.set_tooltip_text(Some(&format!("Espelli {label} dopo conferma, senza forzare")));
            let active=browser.clone();
            let target=mount.clone();
            let mount_label=label.clone();
            button.connect_clicked(move |_|{
                let parent=active.mounts_box.root().and_then(|r|r.downcast::<gtk::Window>().ok());
                let selected=target.clone();
                let state=active.clone();
                let title=format!("Espellere {mount_label}?");
                gio_mount::confirm_mount_action(parent.as_ref(),&title,
                    "Verifica di aver salvato i file e terminato i trasferimenti. Nessuna espulsione forzata.",
                    "Espelli",move ||{
                        let parent=state.mounts_box.root().and_then(|r|r.downcast::<gtk::Window>().ok());
                        let done=state.clone();
                        match gio_mount::eject(&selected,parent.as_ref(),move |result|{
                            done.status.set_text(match &result{
                                Ok(())=>"Dispositivo espulso correttamente.",
                                Err(e)=>e.as_str()
                            });
                            repaint_volumes(&done);
                        }){
                            Ok(_)=>state.status.set_text("Espulsione in corso…"),
                            Err(e)=>state.status.set_text(&e),
                        }
                    });
            });
            row.append(&button);
        }
        browser.mounts_box.append(&row);
    }
    // Expose removable volumes not yet mounted, rather than silently hiding
    // them. The user explicitly clicks Mount and GTK handles credentials.
    for volume in gio::VolumeMonitor::get().volumes() {
        if volume.get_mount().is_some() || !volume.can_mount(){continue;}
        let row=GtkBox::new(Orientation::Horizontal,5);
        let name=volume.name().to_string();
        let description=Label::new(Some(&name));
        description.set_xalign(0.0);
        description.set_hexpand(true);
        row.append(&description);
        let button=Button::with_label("Monta");
        button.set_tooltip_text(Some("Monta questo volume tramite GIO, chiedendo autorizzazione se necessaria"));
        let active=browser.clone();
        let selected=volume.clone();
        button.connect_clicked(move |clicked|{
            clicked.set_sensitive(false);
            let parent=active.mounts_box.root().and_then(|r|r.downcast::<gtk::Window>().ok());
            let done=active.clone();
            let button=clicked.clone();
            gio_mount::mount_volume(&selected,parent.as_ref(),move |result|{
                button.set_sensitive(true);
                done.status.set_text(match &result {
                    Ok(())=>"Volume montato. Aprilo dalla barra laterale.",
                    Err(e)=>e.as_str()
                });
                repaint_volumes(&done);
            });
            active.status.set_text("Montaggio richiesto: conferma le eventuali credenziali.");
        });
        row.append(&button);
        browser.mounts_box.append(&row);
    }
}

#[derive(Clone)]
struct ListingRequest {
    generation: u64,
    directory: PathBuf,
    query: String,
    options: model::ListingOptions,
    cancelled: Arc<AtomicBool>,
}

/// Run no more than one filesystem scan per browser tab. A later filter or
/// navigation replaces the queued request instead of spawning unbounded jobs.
fn start_listing_worker(browser: &Browser) {
    if browser.listing_running.get() {return;}
    let task=browser.listing_pending.borrow_mut().take();
    let Some(request)=task else {return;};
    browser.listing_running.set(true);
    let (sender,receiver)=std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        // Only plain Rust filesystem data crosses the thread boundary.
        // GTK widgets must remain exclusively on the owning main thread.
        let started=Instant::now();
        let result=gio_backend::list_local_entries(&request.directory,&request.query,
            request.options,&request.cancelled);
        let _=sender.send((request,result,started.elapsed()));
    });
    let tab=browser.clone();
    glib::timeout_add_local(Duration::from_millis(35),move || {
        match receiver.try_recv() {
            Ok((request,result,elapsed)) => {
                if files_profile_enabled() && tab.listing_generation.get()==request.generation {
                    eprintln!("MELANIA_FILES_SCAN_MS={} path={}",
                        elapsed.as_millis(), request.directory.display());
                }
                tab.listing_running.set(false);
                if tab.listing_generation.get()==request.generation {
                    match result {
                        Ok(entries)=>render_listing(&tab,&request.directory,entries),
                        Err(error)=>{
                            tab.status.set_text(&format!("Impossibile accedere: {error}"));
                            tab.preview.set_text("La cartella non è disponibile.");
                            tab.preview_image.set_visible(false);
                            tab.visible_paths.borrow_mut().clear();
                            tab.selected_paths.borrow_mut().clear();
                        }
                    }
                }
                start_listing_worker(&tab);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty)=>glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
                tab.listing_running.set(false);
                if tab.listing_pending.borrow().is_none() {
                    tab.status.set_text("Lettura cartella interrotta.");
                }
                start_listing_worker(&tab);
                glib::ControlFlow::Break
            }
        }
    });
}

/// Directory I/O never runs on GTK's event loop.
/// One coalesced thumbnail worker per tab. Rapid F5/navigation cancels the
/// obsolete request; the current worker drains and exits before a new one is
/// started. Decoding and GIO calls NEVER run on the GTK main thread.
fn start_local_thumbnail_worker(browser:&Browser) {
    if browser.thumbnail_running.get(){return;}
    let Some(request)=browser.thumbnail_pending.borrow_mut().take() else {return;};
    browser.thumbnail_running.set(true);
    let (tx,rx)=std::sync::mpsc::sync_channel(1);
    let paths:Vec<PathBuf>=request.targets.iter().map(|(path,_)|path.clone()).collect();
    let stop=request.cancelled.clone();
    std::thread::spawn(move ||{
        let started=Instant::now();
        let mut processed=0usize;
        for (index,path) in paths.into_iter().enumerate() {
            if stop.load(Ordering::Acquire){break;}
            let uri=gio::File::for_path(&path).uri().to_string();
            let result=gio_backend::image_thumbnail(&uri,&stop);
            processed+=1;
            if tx.send((index,result)).is_err(){break;}
        }
        if files_profile_enabled() {
            eprintln!("MELANIA_FILES_THUMBNAILS_MS={} processed={} cancelled={}",
                started.elapsed().as_millis(),processed,stop.load(Ordering::Acquire));
        }
    });
    let ui=browser.clone();
    let targets=request.targets;
    let cancelled=request.cancelled;
    let generation=request.generation;
    // The old 55 ms timer applied one thumbnail per tick: 48 pictures could
    // take >2.6 seconds to appear even when the worker had already decoded
    // them. Drain a SMALL batch each frame and yield to GTK between batches.
    glib::timeout_add_local(Duration::from_millis(16),move ||{
        for _ in 0..4 {
            match rx.try_recv(){
                Ok((index,Ok(thumbnail)))=>{
                    // Discard outdated decoded buffers without touching old tiles.
                    if !cancelled.load(Ordering::Acquire) &&
                        ui.listing_generation.get()==generation{
                        if let Some((_,frame))=targets.get(index){
                            let bytes=glib::Bytes::from_owned(thumbnail.rgba);
                            let texture=gdk::MemoryTexture::new(
                                thumbnail.width as i32,thumbnail.height as i32,
                                gdk::MemoryFormat::R8g8b8a8,&bytes,
                                thumbnail.width as usize*4);
                            let picture=Picture::new();
                            picture.set_size_request(112,84);
                            picture.set_content_fit(gtk::ContentFit::Contain);
                            picture.set_paintable(Some(&texture));
                            while let Some(child)=frame.first_child(){frame.remove(&child);}
                            frame.append(&picture);
                        }
                    }
                },
                Ok((_,Err(_)))=>{},
                Err(std::sync::mpsc::TryRecvError::Empty)=>
                    return glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
                    ui.thumbnail_running.set(false);
                    start_local_thumbnail_worker(&ui);
                    return glib::ControlFlow::Break;
                },
            }
        }
        glib::ControlFlow::Continue
    });
}

fn refresh(browser: &Browser) {
    if let Some(old)=browser.thumbnail_cancel.borrow_mut().take(){
        old.store(true,Ordering::Release);
    }
    if let Some(pending)=browser.thumbnail_pending.borrow_mut().take(){
        pending.cancelled.store(true,Ordering::Release);
    }
    // Cancel obsolete GIO enumeration before scheduling a fresh request.
    if let Some(previous)=browser.listing_cancel.borrow_mut().take() {
        previous.store(true,Ordering::Release);
    }
    let cancelled=Arc::new(AtomicBool::new(false));
    *browser.listing_cancel.borrow_mut()=Some(cancelled.clone());
    let generation=browser.listing_generation.get().wrapping_add(1);
    browser.listing_generation.set(generation);
    let directory=browser.current.borrow().clone();
    let query=browser.search.text().to_lowercase();
    let options=model::ListingOptions {
        show_hidden: browser.show_hidden.get(),
        pictures_only: browser.only_images.get(),
        sort_by: browser.sort_by.get(),
        descending: browser.descending.get(),
        ..Default::default()
    };
    browser.location.set_text(&directory.display().to_string());
    let caption=directory.file_name().filter(|n|!n.is_empty())
        .map(|n|n.to_string_lossy().into_owned())
        .unwrap_or_else(||directory.display().to_string());
    browser.tab_title.set_text(&caption);
    // Hovering a tab exposes its actual location, not just its abbreviated title.
    // The new-tab action reuses this URI/path to open beside the active tab.
    browser.tab_title.set_tooltip_text(Some(&directory.display().to_string()));
    // Stale rows and paths must not remain actionable while another folder
    // or search filter is loading; never allow deleting a hidden old item.
    browser.highlighted.borrow_mut().take();
    browser.tile_buttons.borrow_mut().clear();
    browser.visible_paths.borrow_mut().clear();
    browser.selected_paths.borrow_mut().clear();
    *browser.selected.borrow_mut()=None;
    *browser.selection_anchor.borrow_mut()=None;
    while let Some(child)=browser.tiles.first_child(){
        if let Ok(tile)=child.downcast::<gtk::FlowBoxChild>() {
            browser.tiles.remove(&tile);
        } else {break;}
    }
    browser.preview.set_text("Lettura della cartella in corso…");
    browser.preview_image.set_visible(false);
    browser.status.set_text("Caricamento…");
    *browser.listing_pending.borrow_mut()=Some(ListingRequest{
        generation,directory,query,options,cancelled,
    });
    start_listing_worker(browser);
}

/// GTK rows/actions are built only from the latest worker result for this tab.
fn render_listing(browser: &Browser,dir:&Path,entries:Vec<model::Entry>) {
    let rendering_started=Instant::now();
    let entry_count=entries.len();
    let caption=dir.file_name().filter(|n|!n.is_empty())
        .map(|n|n.to_string_lossy().into_owned())
        .unwrap_or_else(||dir.display().to_string());
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
    let mut thumbnail_targets:Vec<(PathBuf,GtkBox)>=Vec::new();
    let max_thumbnails=thumbnail_budget();
    // Each GTK idle slice constructs only a bounded group of entries. This
    // keeps pointer/keyboard input responsive in directories with many files.
    // An obsolete slice must never add rows to a new location or search.
    let generation=browser.listing_generation.get();
    let browser=browser.clone();
    let dir=dir.to_path_buf();
    let mut remaining=std::collections::VecDeque::from(entries);
    glib::idle_add_local(move ||{
        if browser.listing_generation.get()!=generation {
            return glib::ControlFlow::Break;
        }
        for _ in 0..24 {
            let Some(entry)=remaining.pop_front() else {break;};
        let type_name = file_type_label(&entry);
        let file_size = if entry.is_directory {"—".to_string()} else {human_size(entry.size)};
        let changed = modified_time_label(entry.modified);
        let thumbnail_candidate=!entry.is_directory && !entry.is_symlink &&
            local_thumbnail_candidate(&entry.path);
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
            let symbol = Image::from_icon_name(entry_icon(&path, folder));
            symbol.set_pixel_size(42);
            symbol.add_css_class("files-icon");
            if thumbnail_candidate && thumbnail_targets.len()<max_thumbnails {
                let placeholder=GtkBox::new(Orientation::Vertical,0);
                placeholder.set_size_request(112,84);
                placeholder.set_halign(Align::Center);
                placeholder.set_valign(Align::Center);
                placeholder.append(&symbol);
                thumbnail_targets.push((path.clone(),placeholder.clone()));
                stack.append(&placeholder);
            }else{
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
            ("Apri", "open"), ("Apri con…", "openwith"), ("Copia", "copy"), ("Taglia", "cut"),
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
                    "openwith" => open_with_dialog(&target),
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
        if !remaining.is_empty() {
            return glib::ControlFlow::Continue;
        }
    let queued_thumbnails=thumbnail_targets.len();
    if !list_mode && !thumbnail_targets.is_empty() {
        let cancelled=Arc::new(AtomicBool::new(false));
        *browser.thumbnail_cancel.borrow_mut()=Some(cancelled.clone());
        *browser.thumbnail_pending.borrow_mut()=Some(LocalThumbnailRequest{
            generation:browser.listing_generation.get(),targets:std::mem::take(&mut thumbnail_targets),cancelled,
        });
        start_local_thumbnail_worker(&browser);
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
    // Renderer diagnostics share a redirected stderr with compositor logs.
    // Format a whole line and issue one write to prevent torn metrics.
    if files_profile_enabled() {
        use std::io::Write as _;
        let line=format!("MELANIA_FILES_RENDER_MS={} entries={} thumbnails_queued={} software_budget={}\n",
            rendering_started.elapsed().as_millis(),entry_count,queued_thumbnails,max_thumbnails);
        let _=std::io::stderr().write_all(line.as_bytes());
    }
    // CI-only proof that a real worker result reached the GTK renderer.
    if std::env::var_os("MELANIA_SMOKE_FILES_LISTING").is_some() {
        use std::io::Write as _;
        let line=format!("MELANIA_FILES_LISTING_READY count={count}\n");
        let _=std::io::stderr().write_all(line.as_bytes());
    }
    glib::ControlFlow::Break
    });
}

// Keep a bounded navigation history, independent of the directory renderer.
fn navigate(browser: &Browser, target: PathBuf, keep_history: bool) {
    // Never synchronously stat a slow FUSE/GVfs-backed path on GTK's main loop.
    // The cancellable GIO listing worker reports inaccessible/non-folder paths.
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
    // Clearing the old search must not schedule a SECOND scan 150ms later.
    browser.search_revision.set(browser.search_revision.get().wrapping_add(1));
    browser.suppress_search_refresh.set(true);
    browser.search.set_text("");
    browser.suppress_search_refresh.set(false);
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

fn navigation_button_with_icon(label: &str, target: PathBuf, browser: &Browser, forced_icon: Option<&str>) -> Button {
    let button = Button::new();
    let row = GtkBox::new(Orientation::Horizontal, 9);
    // Use the compact, purpose-built symbolic icons in Melania Files' sidebar.
    // Detailed PNGs remain in the main file grid at larger sizes.
    let icon_name = if label == "Home" { "user-home-symbolic" }
        else if label == "Scrivania" { "user-desktop-symbolic" }
        else if label.contains("Documenti") { "folder-documents-symbolic" }
        else if label.contains("Immagini") { "folder-pictures-symbolic" }
        else if label.contains("Download") { "folder-download-symbolic" }
        else if label.contains("Musica") { "folder-music-symbolic" }
        else if label.contains("Video") { "folder-videos-symbolic" }
        else if label.contains("File system") { "drive-harddisk-symbolic" }
        else { "user-bookmarks-symbolic" };
    let icon = Image::from_icon_name(forced_icon.unwrap_or(icon_name));
    icon.set_pixel_size(21);
    row.append(&icon);
    let caption = Label::new(Some(label));
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

fn navigation_button(label: &str, target: PathBuf, browser: &Browser) -> Button {
    navigation_button_with_icon(label, target, browser, None)
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
        container.append(&navigation_button_with_icon(&name, path.clone(), browser, Some("user-bookmarks-symbolic")));
    }
}

/// Explicit, non-destructive import of local GTK/Nautilus folder bookmarks.
/// Remote URIs are deliberately skipped until the browser becomes gio::File-based.
fn import_nautilus_bookmarks(browser: &Browser) {
    let config = std::env::var_os("XDG_CONFIG_HOME").filter(|s|!s.is_empty())
        .map(PathBuf::from).unwrap_or_else(||home().join(".config"));
    let mut imported=0usize;
    let mut skipped_remote=0usize;
    let mut bookmarks=browser.bookmarks.borrow_mut();
    for version in ["gtk-3.0","gtk-4.0"] {
        let source=config.join(version).join("bookmarks");
        let Ok(bytes)=fs::read(&source) else {continue;};
        if bytes.len()>128*1024 {continue;}
        let Ok(content)=String::from_utf8(bytes) else {continue;};
        for line in content.lines().take(500) {
            let Some(uri)=line.split_whitespace().next() else {continue;};
            if uri.starts_with('#') {continue;}
            if !uri.starts_with("file://") {skipped_remote+=1;continue;}
            let Some(path)=gio::File::for_uri(uri).path() else {continue;};
            if let Ok(true)=bookmarks.add(&path) {imported+=1;}
        }
    }
    let result=if imported>0 {
        model::bookmarks_path().and_then(|path|bookmarks.write_to(&path))
    } else {Ok(())};
    drop(bookmarks);
    match result {
        Ok(()) => {
            repaint_favorites(browser);
            browser.status.set_text(&format!(
                "Importati {imported} preferiti locali da Nautilus/GTK. URI remoti saltati: {skipped_remote}."
            ));
        }
        Err(error) => browser.status.set_text(&format!(
            "Importazione non salvata: {error}; ricontrollare i preferiti prima di chiudere."
        )),
    }
}

/// Numbered batch rename keeps Melania's browser UI and delegates all
/// filesystem mutations to the same Linux atomic no-replace primitive used by
/// single-file rename. A preview is shown before any operation.
fn open_batch_rename_dialog(browser: &Browser) {
    let sources: Vec<PathBuf> = browser.selected_paths.borrow().iter().cloned().collect();
    if sources.len() < 2 || sources.len() > 500 {
        browser.status.set_text("Seleziona da 2 a 500 elementi (Ctrl o Maiusc) per la rinomina multipla.");
        return;
    }
    let window = gtk::Window::builder()
        .title("Rinomina multipla — Melania Files")
        .default_width(560).default_height(440).build();
    let outer = GtkBox::new(Orientation::Vertical, 10);
    outer.set_margin_top(16);
    outer.set_margin_bottom(16);
    outer.set_margin_start(16);
    outer.set_margin_end(16);
    let caption = Label::new(Some(&format!("{} elementi selezionati · anteprima prima della conferma", sources.len())));
    caption.set_xalign(0.0);
    outer.append(&caption);
    let prefix = Entry::builder().placeholder_text("Prefisso (es. Fattura)").build();
    prefix.set_text("File");
    outer.append(&prefix);
    let start = gtk::SpinButton::with_range(1.0, 999_999.0, 1.0);
    start.set_value(1.0);
    start.set_tooltip_text(Some("Numero iniziale"));
    outer.append(&start);
    let listing = Label::new(None);
    listing.set_xalign(0.0);
    listing.set_yalign(0.0);
    listing.set_wrap(true);
    listing.set_selectable(true);
    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_child(Some(&listing));
    outer.append(&scroll);
    let feedback = Label::new(Some("Le estensioni vengono preservate; nessun file sarà sovrascritto."));
    feedback.set_wrap(true);
    feedback.set_xalign(0.0);
    outer.append(&feedback);
    let buttons = GtkBox::new(Orientation::Horizontal, 8);
    buttons.set_halign(Align::End);
    let cancel = Button::with_label("Annulla");
    let apply = Button::with_label("Rinomina");
    apply.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&apply);
    outer.append(&buttons);
    window.set_child(Some(&outer));
    let cancel_window = window.clone();
    cancel.connect_clicked(move |_| cancel_window.close());
    let update = {
        let sources = sources.clone();
        let prefix = prefix.clone();
        let start = start.clone();
        let listing = listing.clone();
        let apply = apply.clone();
        Rc::new(move || {
            let name = prefix.text().to_string();
            match nautilus::batch_rename_plan(&sources, &name, start.value_as_int() as u32) {
                Ok(plan) => {
                    let shown = plan.iter().take(25)
                        .map(|(old,new)| format!("{}  →  {}",
                            old.file_name().unwrap_or_default().to_string_lossy(),
                            new.file_name().unwrap_or_default().to_string_lossy()))
                        .collect::<Vec<_>>().join("\n");
                    listing.set_text(&format!("{shown}{}", if plan.len()>25 { "\n… altri elementi" } else { "" }));
                    apply.set_sensitive(true);
                }
                Err(error) => {
                    listing.set_text(&format!("Impossibile applicare questa rinomina: {error}"));
                    apply.set_sensitive(false);
                }
            }
        })
    };
    update();
    let on_prefix = update.clone();
    prefix.connect_changed(move |_| on_prefix());
    let on_start = update.clone();
    start.connect_value_changed(move |_| on_start());
    let done_window = window.clone();
    let done_state = browser.clone();
    apply.connect_clicked(move |button| {
        let name = prefix.text().to_string();
        let number = start.value_as_int() as u32;
        // Re-plan after the confirmation to detect deleted sources or names
        // that another process created while the preview window was open.
        if let Err(error) = nautilus::batch_rename_plan(&sources, &name, number) {
            feedback.set_text(&format!("Rinomina non avviata: {error}"));
            return;
        }
        button.set_sensitive(false);
        cancel.set_sensitive(false);
        feedback.set_text("Rinomina in corso…");
        let paths = sources.clone();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            let result = model::batch_rename::rename_numbered(&paths, &name, number,
                |source, destination| {
                    let filename = destination.file_name().and_then(|s| s.to_str())
                        .ok_or_else(||std::io::Error::new(std::io::ErrorKind::InvalidInput,"Nome destinazione non valido"))?;
                    transfer::rename_local(source, filename).map(|_| ())
                });
            let _ = tx.send(result);
        });
        let state = done_state.clone();
        let dialog = done_window.clone();
        let feedback = feedback.clone();
        let apply = button.clone();
        let cancel = cancel.clone();
        glib::timeout_add_local(Duration::from_millis(40), move || match rx.try_recv() {
            Ok(Ok(count)) => {
                refresh(&state);
                state.status.set_text(&format!("{count} elementi rinominati senza sovrascritture."));
                dialog.close();
                glib::ControlFlow::Break
            }
            Ok(Err(error)) => {
                feedback.set_text(&format!("Rinomina interrotta: {error}"));
                state.status.set_text(&format!("Controlla il risultato della rinomina multipla: {error}"));
                apply.set_sensitive(true);
                cancel.set_sensitive(true);
                refresh(&state);
                glib::ControlFlow::Break
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                feedback.set_text("La procedura di rinomina è terminata senza risposta. Verifica i file.");
                cancel.set_sensitive(true);
                glib::ControlFlow::Break
            }
        });
    });
    window.present();
}

fn file_tab(start_dir: PathBuf, shared_pending: Rc<RefCell<Option<PendingTransfer>>>,
            tab_title: Label, open_remote: Rc<dyn Fn(String)>) -> GtkBox {
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

    let import_bookmarks = Button::with_label("Importa preferiti Nautilus");
    import_bookmarks.set_tooltip_text(Some("Importa soltanto cartelle locali dai preferiti GTK; non cambia i preferiti di Nautilus"));
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
    other_choices.append(&import_bookmarks);
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
    center_panel.append(&scroll);
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
    let open_with = Button::with_label("Apri con…");
    inspector.append(&open_with);
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
    rename_choices.append(&gtk::Separator::new(Orientation::Horizontal));
    let rename_multiple = Button::with_label("Rinomina multipla…");
    rename_multiple.set_tooltip_text(Some("Rinomina i file selezionati mostrando un'anteprima; ripristina i nomi precedenti in caso di errore"));
    rename_choices.append(&rename_multiple);
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
        tiles, list_mode: Rc::new(Cell::new(true)), list_header: list_header.clone(),
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
        monitor_epoch: Rc::new(Cell::new(0)),
        search_revision: Rc::new(Cell::new(0)),
        suppress_search_refresh: Rc::new(Cell::new(false)),
        listing_generation: Rc::new(Cell::new(0)),
        listing_cancel: Rc::new(RefCell::new(None)),
        listing_running: Rc::new(Cell::new(false)),
        listing_pending: Rc::new(RefCell::new(None)),
        thumbnail_cancel: Rc::new(RefCell::new(None)),
        thumbnail_running: Rc::new(Cell::new(false)),
        thumbnail_pending: Rc::new(RefCell::new(None)),
        mounts_box: GtkBox::new(Orientation::Vertical, 6),
        open_remote: open_remote.clone(),
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
        ("Home", home()),
        ("Scrivania", xdg_user_directory("DESKTOP", "Desktop")),
        ("Documenti", xdg_user_directory("DOCUMENTS", "Documents")),
        ("Immagini", xdg_user_directory("PICTURES", "Pictures")),
        ("Download", xdg_user_directory("DOWNLOAD", "Downloads")),
        ("Musica", xdg_user_directory("MUSIC", "Music")),
        ("Video", xdg_user_directory("VIDEOS", "Videos")),
        ("File system", PathBuf::from("/")),
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
    open_trash.set_tooltip_text(Some("Visualizza e ripristina file direttamente dal Cestino GIO"));
    let trash_browser=browser.clone();
    open_trash.connect_clicked(move |_| open_trash_window(&trash_browser));
    sidebar.append(&open_trash);
    sidebar.append(&browser.mounts_box);
    repaint_volumes(&browser);
    let volumes = gio::VolumeMonitor::get();
    let on_mount = browser.clone();
    volumes.connect_mount_added(move |_, _| repaint_volumes(&on_mount));
    let on_unmount = browser.clone();
    volumes.connect_mount_removed(move |_, _| repaint_volumes(&on_unmount));
    let on_vol_added=browser.clone();
    volumes.connect_volume_added(move |_,_|repaint_volumes(&on_vol_added));
    let on_vol_removed=browser.clone();
    volumes.connect_volume_removed(move |_,_|repaint_volumes(&on_vol_removed));
    let on_vol_changed=browser.clone();
    volumes.connect_volume_changed(move |_,_|repaint_volumes(&on_vol_changed));
    sidebar.append(&browser.favorites_box);
    repaint_favorites(&browser);
    let state=browser.clone();
    browser.location.connect_activate(move |entry| {
        let typed = entry.text().to_string();
        let typed = typed.trim();
        if typed.contains("://") {
            (state.open_remote)(typed.to_string());
            return;
        }
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
        let folder_uri=gio::File::for_path(state.current.borrow().clone()).uri();
        match gio_backend::create_blank_file(&folder_uri,&name) {
            Ok(())=> { refresh(&state); state.status.set_text(&format!("Documento creato: {name}")); },
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
        let folder_uri=gio::File::for_path(state.current.borrow().clone()).uri();
        match gio_backend::make_folder(&folder_uri,name) {
            Ok(())=>{ refresh(&state); state.status.set_text(&format!("Cartella creata: {name}")); },
            Err(err)=>state.status.set_text(&format!("Creazione annullata: {err}")),
        }
    });
    let state=browser.clone();
    recursive_search_button.connect_clicked(move |_| {
        let query = state.search.text().trim().to_string();
        if query.is_empty() {
            state.status.set_text("Scrivi prima il nome da cercare nella casella di ricerca.");
            return;
        }
        let dir = state.current.borrow().clone();
        if dir == Path::new("/") {
            state.status.set_text("Per la ricerca ricorsiva scegli una cartella più specifica.");
            return;
        }
        let window = gtk::Window::builder()
            .title("Ricerca in sottocartelle — Melania Files")
            .default_width(560).default_height(480).build();
        let outer = GtkBox::new(Orientation::Vertical, 8);
        outer.set_margin_top(12);
        outer.set_margin_start(12);
        outer.set_margin_end(12);
        outer.set_margin_bottom(12);
        let summary = Label::new(Some("Ricerca in corso…"));
        summary.set_xalign(0.0);
        outer.append(&summary);
        let scroll = ScrolledWindow::new();
        scroll.set_vexpand(true);
        let list = GtkBox::new(Orientation::Vertical, 3);
        scroll.set_child(Some(&list));
        outer.append(&scroll);
        let cancel = Button::with_label("Annulla ricerca");
        outer.append(&cancel);
        window.set_child(Some(&outer));
        let cancelled = Arc::new(AtomicBool::new(false));
        let on_close=cancelled.clone();
        window.connect_close_request(move |_| {
            on_close.store(true,Ordering::Release);
            glib::Propagation::Proceed
        });
        let close_window=window.clone();
        cancel.connect_clicked(move |_| close_window.close());
        let (sender,receiver)=std::sync::mpsc::sync_channel(1);
        let worker_cancel=cancelled.clone();
        let show_hidden=state.show_hidden.get();
        std::thread::spawn(move || {
            let result=nautilus::search_tree_cancellable(&dir,&query,nautilus::SearchLimits {
                max_results: 150, max_directories: 120, show_hidden
            },||worker_cancel.load(Ordering::Acquire));
            let _=sender.send(result);
        });
        let result_state=state.clone();
        let result_cancel=cancelled.clone();
        glib::timeout_add_local(Duration::from_millis(40),move || {
            if result_cancel.load(Ordering::Acquire) {return glib::ControlFlow::Break;}
            match receiver.try_recv() {
                Ok(Ok(result)) => {
                    summary.set_text(&format!("{} risultati ({} cartelle esaminate){}",
                        result.paths.len(),result.directories_scanned,
                        if result.truncated {" · Limite raggiunto"} else {""}));
                    cancel.set_label("Chiudi");
                    for path in result.paths {
                        let button=Button::with_label(&path.display().to_string());
                        let target=path.clone();
                        let state=result_state.clone();
                        button.connect_clicked(move |_| {
                            if target.is_dir() {navigate(&state,target.clone(),true);}
                            else if let Some(parent)=target.parent() {
                                navigate(&state,parent.to_path_buf(),true);
                            }
                        });
                        list.append(&button);
                    }
                    glib::ControlFlow::Break
                }
                Ok(Err(error)) => {
                    summary.set_text(&format!("Ricerca non riuscita: {error}"));
                    result_state.status.set_text(&format!("Ricerca non riuscita: {error}"));
                    cancel.set_label("Chiudi");
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty)=>glib::ControlFlow::Continue,
                Err(std::sync::mpsc::TryRecvError::Disconnected)=>{
                    summary.set_text("Ricerca interrotta.");
                    cancel.set_label("Chiudi");
                    glib::ControlFlow::Break
                }
            }
        });
        window.present();
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
    let state_for_import=browser.clone();
    import_bookmarks.connect_clicked(move |_| import_nautilus_bookmarks(&state_for_import));
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
    // Search-as-you-type must not tear down and rebuild the entire FlowBox
    // on every single keystroke. Only the latest edit runs after a short pause.
    browser.search.connect_changed(move |_| {
        let ticket=state.search_revision.get().wrapping_add(1);
        state.search_revision.set(ticket);
        if state.suppress_search_refresh.get() {return;}
        let current_revision=state.search_revision.clone();
        let tab=state.clone();
        glib::timeout_add_local_once(Duration::from_millis(150),move || {
            if current_revision.get()==ticket {refresh(&tab);}
        });
    });
    let state=browser.clone();
    properties_button.connect_clicked(move |_| {
        if let Some(path) = state.selected.borrow().as_ref() { show_properties(path); }
        else { state.status.set_text("Seleziona un elemento per visualizzarne le proprietà."); }
    });
    let batch_state = browser.clone();
    rename_multiple.connect_clicked(move |_| open_batch_rename_dialog(&batch_state));
    let state=browser.clone();
    let rename_name_for_click = rename_name.clone();
    rename_button.connect_clicked(move |_| {
        let Some(old)=state.selected.borrow().clone() else {
            state.status.set_text("Seleziona un file prima di rinominarlo."); return;
        };
        let source_uri=gio::File::for_path(&old).uri();
        match gio_backend::rename_item(&source_uri,&rename_name_for_click.text()) {
            Ok(updated_uri)=> {
                *state.selected.borrow_mut()=gio::File::for_uri(&updated_uri).path();
                refresh(&state);
                state.status.set_text("Rinomina GIO completata senza sovrascritture.");
            },
            Err(err)=>state.status.set_text(&format!("Rinomina annullata: {err}")),
        }
    });
    let selected_for_open_with=browser.selected.clone();
    open_with.connect_clicked(move |_| {
        if let Some(path)=selected_for_open_with.borrow().as_ref() {
            if path.is_file() { open_with_dialog(path); }
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
            // Preserve no-replace CUT semantics, but never block GTK on disk I/O.
            start_local_mutation(&state,LocalMutationKind::Move,pending.paths,Some(target));
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
                let files=list.files();
                if files.is_empty() {
                    state_done.status.set_text("Nessun file negli appunti.");return;
                }
                if files.iter().all(|f|f.path().is_some()){
                    let sources=files.iter().filter_map(|f|f.path()).collect();
                    start_background_copy(&state_done,sources,target);
                } else {
                    let sources=files.iter().map(|f|f.uri().to_string()).collect();
                    start_gio_copy_into_local(&state_done,sources,target);
                }
            }
        );
    });
    let state=browser.clone();
    trash_button.connect_clicked(move |_| {
        let paths: Vec<PathBuf> = state.selected_paths.borrow().iter().cloned().collect();
        if paths.is_empty() {
            state.status.set_text("Seleziona uno o più file da spostare nel Cestino.");return;
        }
        // Only recoverable GIO trash, on a worker. No permanent-delete fallback.
        start_local_mutation(&state,LocalMutationKind::Trash,paths,None);
    });
    // Per-page keyboard navigation works even when other tabs are open.
    // GTK routes the event to the focused page; back/forward histories stay isolated.
    let key_nav = gtk::EventControllerKey::new();
    let go_back = back.clone();
    let go_forward = forward.clone();
    let go_up = up.clone();
    let do_refresh = refresh_button.clone();
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
    // Releasing a notebook page must stop outstanding monitor callbacks and
    // background reads; stale completions must not repaint a closed tab.
    let closing=browser.clone();
    root.connect_unrealize(move |_| {
        closing.monitor_epoch.set(closing.monitor_epoch.get().wrapping_add(1));
        if let Some(monitor)=closing.monitor.borrow_mut().take(){monitor.cancel();}
        if let Some(cancel)=closing.listing_cancel.borrow_mut().take(){
            cancel.store(true,Ordering::Release);
        }
        if let Some(cancel)=closing.thumbnail_cancel.borrow_mut().take(){
            cancel.store(true,Ordering::Release);
        }
        if let Some(pending)=closing.listing_pending.borrow_mut().take(){
            pending.cancelled.store(true,Ordering::Release);
        }
        if let Some(pending)=closing.thumbnail_pending.borrow_mut().take(){
            pending.cancelled.store(true,Ordering::Release);
        }
    });
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

/// Close a tab by its page identity rather than a saved numeric index.
fn close_files_tab(notebook:&gtk::Notebook,strip:&GtkBox,
                   headers:&Rc<RefCell<Vec<(GtkBox,GtkBox,Label)>>>,
                   window:&ApplicationWindow,page:&GtkBox) {
    // Melania retains the final open tab instead of unexpectedly closing the
    // whole file manager. Both the X and Ctrl+W use this exact same rule.
    if notebook.n_pages()<=1{return;}
    let Some(index)=notebook.page_num(page) else{return;};
    let heading=headers.borrow().iter()
        .find(|(candidate,_,_)|candidate==page)
        .map(|(_,heading,_)|heading.clone());
    if let Some(heading)=heading{strip.remove(&heading);}
    headers.borrow_mut().retain(|(candidate,_,_)|candidate!=page);
    notebook.remove_page(Some(index));
    sync_tab_highlights(notebook,headers,window);
    if std::env::var_os("MELANIA_SMOKE_FILES_TABS").is_some() {
        eprintln!("MELANIA_FILES_TAB_COUNT={}",headers.borrow().len());
    }
}

/// One source of truth for keyboard, context-menu and pointer tab reorder.
/// The hidden Notebook and the visible titlebar rail must never disagree.
fn reorder_files_tab(notebook:&gtk::Notebook,strip:&GtkBox,
                     headers:&Rc<RefCell<Vec<(GtkBox,GtkBox,Label)>>>,
                     window:&ApplicationWindow,page:&GtkBox,destination:u32) -> bool {
    let Some(origin)=notebook.page_num(page) else{return false;};
    let destination=destination.min(notebook.n_pages().saturating_sub(1));
    if origin==destination{return false;}
    notebook.reorder_child(page,Some(destination));
    let (heading,previous)={
        let registered=headers.borrow();
        let heading=registered.iter().find(|(p,_,_)|p==page).map(|(_,h,_)|h.clone());
        let previous=if destination==0{None}else{
            registered.iter()
                .find(|(p,_,_)|notebook.page_num(p)==Some(destination-1))
                .map(|(_,h,_)|h.clone())
        };
        (heading,previous)
    };
    if let Some(heading)=heading {
        strip.reorder_child_after(&heading,previous.as_ref());
    }
    sync_tab_highlights(notebook,headers,window);
    true
}

/// A drop on the left/right half of a tab selects the adjacent insertion
/// slot. Removing a source from before that slot shifts the index by one.
fn tab_drop_destination(source:u32,target:u32,after:bool)->u32 {
    let insertion=target.saturating_add(u32::from(after));
    if source<insertion{insertion-1}else{insertion}
}

fn close_other_files_tabs(notebook:&gtk::Notebook,strip:&GtkBox,
                          headers:&Rc<RefCell<Vec<(GtkBox,GtkBox,Label)>>>,
                          window:&ApplicationWindow,keep:&GtkBox,right_only:bool){
    let keep_index=notebook.page_num(keep).unwrap_or(0);
    let to_close:Vec<GtkBox>=headers.borrow().iter()
        .filter(|(page,_,_)|page!=keep &&
            (!right_only || notebook.page_num(page).is_some_and(|index|index>keep_index)))
        .map(|(page,_,_)|page.clone()).collect();
    for page in to_close{close_files_tab(notebook,strip,headers,window,&page);}
}

/// Reveal the active tab when the rail is narrower than the total set.
fn reveal_active_files_tab(notebook:&gtk::Notebook,strip:&GtkBox,
                           headers:&Rc<RefCell<Vec<(GtkBox,GtkBox,Label)>>>,
                           scroll:&ScrolledWindow) {
    let active=notebook.current_page();
    let target=headers.borrow().iter().find(|(page,_,_)|notebook.page_num(page)==active)
        .map(|(_,tab,_)|tab.clone());
    let Some(tab)=target else{return;};
    let scroller=scroll.clone();
    let rail=strip.clone();
    glib::idle_add_local_once(move ||{
        let Some((x,_))=tab.translate_coordinates(&rail,0.0,0.0) else{return;};
        let adjustment=scroller.hadjustment();
        let left=adjustment.value();
        let visible=adjustment.page_size();
        if visible<=0.0{return;}
        if x<left {adjustment.set_value(x);}
        else if x+f64::from(tab.width())>left+visible{
            adjustment.set_value(x+f64::from(tab.width())-visible);
        }
    });
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
    open_remote: Rc<dyn Fn(String)>,
    new_button: &Button,
) {
    let text = Label::new(Some(title));
    let child = file_tab(path, pending, text.clone(), open_remote);
    append_file_page(notebook,tab_strip,headers,window,text,child,"folder-symbolic",new_button);
}
fn append_remote_tab(
    notebook: &gtk::Notebook,
    tab_strip: &GtkBox,
    headers: &Rc<RefCell<Vec<(GtkBox, GtkBox, Label)>>>,
    window: &ApplicationWindow,
    uri: &str,
    new_button: &Button,
) {
    let title=Label::new(Some("Rete"));
    let child=gio_remote::create(uri,title.clone());
    append_file_page(notebook,tab_strip,headers,window,title,child,"network-server-symbolic",new_button);
}
/// Shared Nautilus-style tab bar for local and GVfs pages. The GTK Notebook
/// still owns actual pages, while the compact headerbar renders their handles.
#[derive(Clone,Copy)]
enum FilesTabAction { Select, New, Close, CloseOthers, CloseRight, MoveLeft, MoveRight }

fn append_file_page(
    notebook: &gtk::Notebook,
    tab_strip: &GtkBox,
    headers: &Rc<RefCell<Vec<(GtkBox, GtkBox, Label)>>>,
    window: &ApplicationWindow,
    text: Label,
    child: GtkBox,
    icon_name: &str,
    new_button: &Button,
) {
    let tab = GtkBox::new(Orientation::Horizontal, 2);
    tab.add_css_class("files-top-tab");
    tab.set_valign(Align::End);
    tab.set_widget_name(&format!("melania-tab-{}",NEXT_FILES_TAB_ID.fetch_add(1,Ordering::Relaxed)));
    let tab_button = Button::new();
    tab_button.add_css_class("files-top-tab-select");
    tab_button.set_hexpand(true);
    let tab_contents = GtkBox::new(Orientation::Horizontal, 7);
    let icon = Image::from_icon_name(icon_name);
    icon.set_pixel_size(16);
    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.set_max_width_chars(17);
    text.set_hexpand(true);
    text.set_xalign(0.0);
    tab_contents.append(&icon);
    tab_contents.append(&text);
    tab_button.set_child(Some(&tab_contents));
    let close = Button::from_icon_name("window-close-symbolic");
    close.add_css_class("files-tab-close");
    close.set_tooltip_text(Some("Chiudi scheda (Ctrl+W)"));
    tab.append(&tab_button);
    tab.append(&close);

    // Nautilus opens a new tab immediately to the right of the active page.
    // The plus button itself stays at the far end of the scrollable tab rail.
    let previous=notebook.current_page();
    let index=notebook.append_page(&child,None::<&gtk::Widget>);
    headers.borrow_mut().push((child.clone(),tab.clone(),text.clone()));
    tab_strip.append(&tab);
    if let Some(active)=previous {
        let after=(active+1).min(index);
        if after<index {
            reorder_files_tab(notebook,tab_strip,headers,window,&child,after);
        }
    }
    notebook.set_current_page(notebook.page_num(&child));
    sync_tab_highlights(notebook,headers,window);
    if std::env::var_os("MELANIA_SMOKE_FILES_TABS").is_some() {
        eprintln!("MELANIA_FILES_TAB_COUNT={}",headers.borrow().len());
    }

    let tabs_for_title=notebook.clone();
    let page_for_title=child.clone();
    let window_for_title=window.clone();
    text.connect_label_notify(move |label| {
        if tabs_for_title.page_num(&page_for_title)==tabs_for_title.current_page() {
            window_for_title.set_title(Some(&format!("{} — Melania Files",label.label())));
        }
    });

    // Only weak references are captured by tab controls, so closing a tab
    // does not keep its file monitors and GTK widgets alive indefinitely.
    let weak_pages=notebook.downgrade();
    let weak_strip=tab_strip.downgrade();
    let weak_window=window.downgrade();
    let weak_page=child.downgrade();
    let weak_new=new_button.downgrade();
    let weak_headers=Rc::downgrade(headers);
    let dispatch:Rc<dyn Fn(FilesTabAction)>=Rc::new(move |action|{
        let (Some(pages),Some(strip),Some(window),Some(page),Some(headers))=(
            weak_pages.upgrade(),weak_strip.upgrade(),weak_window.upgrade(),
            weak_page.upgrade(),weak_headers.upgrade()) else{return;};
        match action {
            FilesTabAction::Select=>{
                if let Some(index)=pages.page_num(&page){pages.set_current_page(Some(index));}
            },
            FilesTabAction::New=>{
                if let Some(index)=pages.page_num(&page){pages.set_current_page(Some(index));}
                if let Some(button)=weak_new.upgrade(){button.emit_clicked();}
            },
            FilesTabAction::Close=>close_files_tab(&pages,&strip,&headers,&window,&page),
            FilesTabAction::CloseOthers=>{
                close_other_files_tabs(&pages,&strip,&headers,&window,&page,false)
            },
            FilesTabAction::CloseRight=>{
                close_other_files_tabs(&pages,&strip,&headers,&window,&page,true)
            },
            FilesTabAction::MoveLeft|FilesTabAction::MoveRight=>{
                if let Some(index)=pages.page_num(&page){
                    let target=if matches!(action,FilesTabAction::MoveLeft) {
                        index.saturating_sub(1)
                    }else{(index+1).min(pages.n_pages().saturating_sub(1))};
                    reorder_files_tab(&pages,&strip,&headers,&window,&page,target);
                }
            }
        }
    });
    let select=dispatch.clone();
    tab_button.connect_clicked(move |_|select(FilesTabAction::Select));
    let close_action=dispatch.clone();
    close.connect_clicked(move |_|close_action(FilesTabAction::Close));

    let middle=gtk::GestureClick::new();
    middle.set_button(2);
    let close_middle=dispatch.clone();
    middle.connect_released(move |_,_,_,_|close_middle(FilesTabAction::Close));
    tab.add_controller(middle);

    // A native GTK popover presents the familiar Nautilus tab operations.
    let menu=gtk::Popover::new();
    menu.add_css_class("files-tab-menu");
    menu.set_has_arrow(false);
    menu.set_autohide(true);
    menu.set_parent(&tab);
    let actions=GtkBox::new(Orientation::Vertical,3);
    actions.add_css_class("files-tab-menu-items");
    for (caption,command) in [
        ("Nuova scheda",FilesTabAction::New),
        ("Sposta scheda a sinistra",FilesTabAction::MoveLeft),
        ("Sposta scheda a destra",FilesTabAction::MoveRight),
        ("Chiudi scheda",FilesTabAction::Close),
        ("Chiudi altre schede",FilesTabAction::CloseOthers),
        ("Chiudi schede a destra",FilesTabAction::CloseRight),
    ] {
        let button=Button::with_label(caption);
        button.add_css_class("files-tab-menu-item");
        button.set_halign(Align::Fill);
        let dispatch=dispatch.clone();
        let weak_menu=menu.downgrade();
        button.connect_clicked(move |_| {
            if let Some(popover)=weak_menu.upgrade(){popover.popdown();}
            dispatch(command);
        });
        actions.append(&button);
    }
    menu.set_child(Some(&actions));
    let right_click=gtk::GestureClick::new();
    right_click.set_button(3);
    let weak_menu=menu.downgrade();
    right_click.connect_pressed(move |_,_,x,y|{
        if let Some(popover)=weak_menu.upgrade(){
            popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32,y as i32,1,1)));
            popover.popup();
        }
    });
    tab.add_controller(right_click);

    // Drag a tab onto either half of another tab to reorder it. This
    // represents TAB handles only; no file DnD/clipboard payload is changed.
    let drag_source=gtk::DragSource::new();
    drag_source.set_actions(gdk::DragAction::MOVE);
    let drag_token=tab.widget_name().to_string();
    drag_source.connect_prepare(move |_,_,_|{
        Some(gdk::ContentProvider::for_value(&drag_token.to_value()))
    });
    tab.add_controller(drag_source);
    let drop=gtk::DropTarget::new(String::static_type(),gdk::DragAction::MOVE);
    let weak_pages=notebook.downgrade();
    let weak_strip=tab_strip.downgrade();
    let weak_window=window.downgrade();
    let weak_page=child.downgrade();
    let weak_tab=tab.downgrade();
    let weak_headers=Rc::downgrade(headers);
    drop.connect_drop(move |_,value,x,_|{
        let Ok(token)=value.get::<String>() else{return false;};
        if !token.starts_with("melania-tab-"){return false;}
        let (Some(pages),Some(strip),Some(window),Some(page),Some(tab),Some(headers))=(
            weak_pages.upgrade(),weak_strip.upgrade(),weak_window.upgrade(),
            weak_page.upgrade(),weak_tab.upgrade(),weak_headers.upgrade()) else{return false;};
        let source=headers.borrow().iter()
            .find(|(_,heading,_)|heading.widget_name().as_str()==token)
            .map(|(source,_,_)|source.clone());
        let Some(source)=source else{return false;};
        let (Some(from),Some(target))=(pages.page_num(&source),pages.page_num(&page))
            else{return false;};
        let right_half=x>f64::from(tab.width())/2.0;
        let destination=tab_drop_destination(from,target,right_half);
        reorder_files_tab(&pages,&strip,&headers,&window,&source,destination)
    });
    tab.add_controller(drop);
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
    let startup_clock=Instant::now();
    install_files_ui_stall_probe();
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
    let appearance = melania_config::load().unwrap_or_default();
    window.add_css_class(&format!("melania-theme-{}", appearance.appearance_theme()));
    if let Some(gtk_settings) = gtk::Settings::default() {
        gtk_settings.set_property("gtk-application-prefer-dark-theme", appearance.dark_mode);
    }
    // One integrated header: [+] [tabs...] [shared Win7 controls].
    // Same accessible, functional titlebar buttons as Melania Settings.
    let aero_header = gtk::HeaderBar::new();
    aero_header.add_css_class("files-aero-header");
    aero_header.add_css_class(&format!("melania-theme-{}", appearance.appearance_theme()));
    window_controls::install(&window, &aero_header);
    // Override GTK HeaderBar's automatic centered window-title widget:
    // the tabs ARE the titlebar, never add a duplicate app title.
    let no_title = GtkBox::new(Orientation::Horizontal, 0);
    aero_header.set_title_widget(Some(&no_title));
    let add_button = Button::from_icon_name("list-add-symbolic");
    add_button.add_css_class("files-new-tab");
    add_button.set_tooltip_text(Some("Nuova scheda (Ctrl+T)"));
    let tab_strip = GtkBox::new(Orientation::Horizontal, 0);
    tab_strip.add_css_class("files-top-tabs");
    let tab_scroll = ScrolledWindow::new();
    tab_scroll.add_css_class("files-tab-scroll");
    tab_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
    tab_scroll.set_min_content_width(160);
    tab_scroll.set_max_content_width(650);
    tab_scroll.set_child(Some(&tab_strip));
    // One tab rail: open tabs followed by [+], never the reverse.
    let tab_rail = GtkBox::new(Orientation::Horizontal, 3);
    tab_rail.add_css_class("files-tab-rail");
    tab_rail.append(&tab_scroll);
    tab_rail.append(&add_button);
    let network_button=Button::from_icon_name("network-server-symbolic");
    network_button.add_css_class("files-new-tab");
    network_button.set_tooltip_text(Some("Rete e server SMB/SFTP/WebDAV (GVfs)"));
    tab_rail.append(&network_button);
    aero_header.pack_start(&tab_rail);
    window.set_titlebar(Some(&aero_header));
    let frame = GtkBox::new(Orientation::Vertical, 0);
    let tabs = gtk::Notebook::new();
    tabs.set_show_tabs(false);
    tabs.set_vexpand(true);
    tabs.set_show_border(false);
    tabs.add_css_class("files-notebook");
    // Present a lightweight real Files window before constructing the full
    // GIO browser tab; GTK can paint its chrome while the first folder loads.
    let loading=Label::new(Some("Caricamento cartella…"));
    loading.add_css_class("files-loading");
    tabs.append_page(&loading,None::<&gtk::Widget>);
    let tab_headers: Rc<RefCell<Vec<(GtkBox, GtkBox, Label)>>> = Rc::new(RefCell::new(Vec::new()));
    let headers_for_switch = tab_headers.clone();
    let window_for_switch = window.clone();
    let scroll_for_switch = tab_scroll.clone();
    let strip_for_switch = tab_strip.clone();
    tabs.connect_switch_page(move |notebook, _, _| {
        sync_tab_highlights(notebook, &headers_for_switch, &window_for_switch);
        reveal_active_files_tab(notebook,&strip_for_switch,&headers_for_switch,&scroll_for_switch);
    });

    let weak_tabs=tabs.downgrade();
    let weak_strip=tab_strip.downgrade();
    let weak_window=window.downgrade();
    let weak_add=add_button.downgrade();
    let headers_for_remote=tab_headers.clone();
    let open_remote:Rc<dyn Fn(String)>=Rc::new(move |uri:String| {
        if let (Some(tabs),Some(strip),Some(window),Some(add))=(
            weak_tabs.upgrade(),weak_strip.upgrade(),weak_window.upgrade(),weak_add.upgrade()) {
            append_remote_tab(&tabs,&strip,&headers_for_remote,&window,&uri,&add);
        }
    });
    let open_network=open_remote.clone();
    network_button.connect_clicked(move |_|open_network("network:///".to_owned()));
    let shared_pending: Rc<RefCell<Option<PendingTransfer>>> = Rc::new(RefCell::new(None));
    let notebook = tabs.clone();
    let pending_for_add = shared_pending.clone();
    let header_for_add = tab_headers.clone();
    let rail_for_add = tab_strip.clone();
    let window_for_add = window.clone();
    let remote_for_add=open_remote.clone();
    let new_button_for_tab=add_button.clone();
    add_button.connect_clicked(move |_| {
        // A Nautilus new tab starts in the currently visible folder/URI,
        // rather than unexpectedly resetting to Home.
        let current=notebook.current_page();
        let place=header_for_add.borrow().iter()
            .find(|(page,_,_)|notebook.page_num(page)==current)
            .and_then(|(_,_,title)|title.tooltip_text())
            .map(|uri|uri.to_string());
        if let Some(uri)=place.as_deref().filter(|uri|uri.contains("://")){
            remote_for_add(uri.to_string());
        } else {
            let path=place.map(PathBuf::from).unwrap_or_else(home);
            let title=path.file_name().map(|name|name.to_string_lossy().into_owned())
                .filter(|name|!name.is_empty()).unwrap_or_else(||"Home".to_owned());
            append_browser_tab(&notebook, &rail_for_add, &header_for_add, &window_for_add,
                path, pending_for_add.clone(), &title, remote_for_add.clone(),&new_button_for_tab);
        }
    });
    frame.append(&tabs);

    // GNOME/Nautilus tab shortcuts. Within-file shortcuts remain attached
    // to their own page; these only act on the window's tab view.
    let keys = gtk::EventControllerKey::new();
    let keys_notebook = tabs.clone();
    let keys_add = add_button.clone();
    let tab_headers_for_keys = tab_headers.clone();
    let strip_for_keys = tab_strip.clone();
    let window_for_keys = window.clone();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        let ctrl=modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        let shift=modifiers.contains(gdk::ModifierType::SHIFT_MASK);
        if modifiers.contains(gdk::ModifierType::ALT_MASK) && !ctrl {
            if let Some(digit)=key.to_unicode().and_then(|c|c.to_digit(10)){
                let count=keys_notebook.n_pages();
                if count>0{
                    let index=if digit==0{count-1}else{digit-1};
                    if index<count {keys_notebook.set_current_page(Some(index));}
                    return glib::Propagation::Stop;
                }
            }
        }
        if !ctrl{return glib::Propagation::Proceed;}
        if key == gdk::Key::t || key == gdk::Key::T {
            if !shift{keys_add.emit_clicked();}
            return if shift{glib::Propagation::Proceed}else{glib::Propagation::Stop};
        }
        if key == gdk::Key::w || key == gdk::Key::W {
            if let Some(index)=keys_notebook.current_page(){
                if let Some(page)=keys_notebook.nth_page(Some(index))
                    .and_then(|page|page.downcast::<GtkBox>().ok()){
                    close_files_tab(&keys_notebook,&strip_for_keys,&tab_headers_for_keys,
                                    &window_for_keys,&page);
                }
            }
            return glib::Propagation::Stop;
        }
        let count=keys_notebook.n_pages();
        if count<=1{return glib::Propagation::Proceed;}
        let current=keys_notebook.current_page().unwrap_or(0);
        if (key==gdk::Key::Page_Up || key==gdk::Key::Page_Down) && shift{
            let target=if key==gdk::Key::Page_Up{
                current.saturating_sub(1)
            }else{(current+1).min(count-1)};
            if let Some(page)=keys_notebook.nth_page(Some(current))
                .and_then(|page|page.downcast::<GtkBox>().ok()){
                reorder_files_tab(&keys_notebook,&strip_for_keys,&tab_headers_for_keys,
                                  &window_for_keys,&page,target);
            }
            return glib::Propagation::Stop;
        }
        if key==gdk::Key::Home {
            keys_notebook.set_current_page(Some(0));
            return glib::Propagation::Stop;
        }
        if key==gdk::Key::End {
            keys_notebook.set_current_page(Some(count-1));
            return glib::Propagation::Stop;
        }
        if key==gdk::Key::Tab || key==gdk::Key::ISO_Left_Tab ||
            key==gdk::Key::Page_Up || key==gdk::Key::Page_Down {
            let backwards=(key==gdk::Key::Tab && shift) ||
                key==gdk::Key::ISO_Left_Tab || key==gdk::Key::Page_Up;
            let next=if backwards{(current+count-1)%count}else{(current+1)%count};
            keys_notebook.set_current_page(Some(next));
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);
    window.set_child(Some(&frame));
    window.present();
    if files_profile_enabled() {
        eprintln!("MELANIA_FILES_FIRST_WINDOW_MS={}",startup_clock.elapsed().as_millis());
    }
    let first_tabs=tabs.clone();
    let first_strip=tab_strip.clone();
    let first_headers=tab_headers.clone();
    let first_window=window.clone();
    let first_pending=shared_pending.clone();
    let first_remote=open_remote.clone();
    let first_add=add_button.clone();
    glib::timeout_add_local_once(Duration::from_millis(55),move || {
        // Only now perform path probes and instantiate the complete browser
        // widget tree. Initial async listing and thumbnails remain separate.
        let requested_uri=std::env::args_os().nth(1)
            .and_then(|s|s.into_string().ok())
            .filter(|s|s.contains("://"));
        let starting=std::env::args_os().nth(1).map(PathBuf::from)
            .filter(|p|p.is_dir()).unwrap_or_else(home);
        let title=starting.file_name().map(|s|s.to_string_lossy().into_owned())
            .filter(|s|!s.is_empty()).unwrap_or_else(||"Home".to_owned());
        append_browser_tab(&first_tabs,&first_strip,&first_headers,&first_window,
            starting,first_pending,&title,first_remote.clone(),&first_add);
        // Drop only the temporary loading page; all genuine tabs remain.
        first_tabs.remove_page(Some(0));
        if let Some(uri)=requested_uri {first_remote(uri);}
        if let Ok(uri)=std::env::var("MELANIA_SMOKE_GIO_FILE_URI") {
            if uri.starts_with("file:///") {first_remote(uri);}
        }
        if files_profile_enabled() {
            eprintln!("MELANIA_FILES_FIRST_TAB_MS={}",startup_clock.elapsed().as_millis());
        }
    });
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
mod software_preview_performance_tests {
    use super::{thumbnail_budget_for,LocalMutationKind,run_local_mutation};
    use std::sync::{Arc,atomic::AtomicBool};


    #[test]
    fn cancelled_cut_does_not_touch_source_or_destination() {
        let root=std::env::temp_dir().join(format!("melania-mutation-cancel-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&root);
        let from=root.join("from");let to=root.join("to");
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(from.join("safe.txt"),b"original").unwrap();
        let stop=Arc::new(AtomicBool::new(true));
        let outcome=run_local_mutation(LocalMutationKind::Move,
            vec![from.join("safe.txt")],Some(to.clone()),&stop);
        assert!(outcome.cancelled);
        assert_eq!(outcome.completed,0);
        assert_eq!(outcome.remaining,vec![from.join("safe.txt")]);
        assert_eq!(std::fs::read(from.join("safe.txt")).unwrap(),b"original");
        assert!(!to.join("safe.txt").exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_cut_never_overwrites_destination() {
        let root=std::env::temp_dir().join(format!("melania-mutation-conflict-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&root);
        let from=root.join("from");let to=root.join("to");
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        std::fs::write(from.join("existing.txt"),b"source").unwrap();
        std::fs::write(to.join("existing.txt"),b"destination").unwrap();
        let stop=Arc::new(AtomicBool::new(false));
        let outcome=run_local_mutation(LocalMutationKind::Move,
            vec![from.join("existing.txt")],Some(to.clone()),&stop);
        assert_eq!(outcome.completed,0);
        assert_eq!(outcome.failed,1);
        assert_eq!(outcome.remaining.len(),1);
        assert_eq!(std::fs::read(from.join("existing.txt")).unwrap(),b"source");
        assert_eq!(std::fs::read(to.join("existing.txt")).unwrap(),b"destination");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn preserve_full_thumbnails_on_accelerated_sessions() {
        assert_eq!(thumbnail_budget_for(None,false),48);
        assert_eq!(thumbnail_budget_for(Some("ngl"),false),48);
    }

    #[test]
    fn bound_thumbnail_decoding_on_software_renderers() {
        assert_eq!(thumbnail_budget_for(Some("cairo"),false),16);
        assert_eq!(thumbnail_budget_for(None,true),16);
    }
}

#[cfg(test)]
mod nautilus_tab_reorder_tests {
    use super::tab_drop_destination;

    #[test]
    fn dropping_before_and_after_uses_the_correct_notebook_index() {
        assert_eq!(tab_drop_destination(0,2,false),1);
        assert_eq!(tab_drop_destination(0,2,true),2);
        assert_eq!(tab_drop_destination(3,1,false),1);
        assert_eq!(tab_drop_destination(3,1,true),2);
        assert_eq!(tab_drop_destination(1,1,false),1);
        assert_eq!(tab_drop_destination(1,1,true),1);
    }

    #[test]
    fn repeated_drag_does_not_reorder_into_an_invalid_negative_slot() {
        assert_eq!(tab_drop_destination(0,0,false),0);
        assert_eq!(tab_drop_destination(0,0,true),0);
        assert_eq!(tab_drop_destination(9,0,false),0);
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
