//! Melania desktop layer (Wayland / GTK 4, independent of the old X11 UX demo).
//! Runs *under* normal windows through zwlr_layer_shell_v1.
//! Drops always COPY: do not delete a source based on a Wayland DnD success signal.
use gtk::{self, gdk, gio, prelude::*};
use gtk::{Align, Application, ApplicationWindow, Box as GtkBox, Button, FlowBox, Image, Label, Orientation, Overlay, Picture};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::{cell::RefCell, path::{Path, PathBuf}, process::Command, rc::Rc, time::Duration};
use melania_desktop_core::{Selection, list_desktop, create_unique_folder};

const APP_ID: &str = "org.melaniadesktop.Desktop";
const WALLPAPER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/wallpaper.png");

/// User-defined wallpaper takes effect only in Melania. The CI/one-shot
/// MELANIA_WALLPAPER override retains precedence for isolated screenshots.
fn wallpaper_path(prefs:&melania_config::Preferences)->PathBuf {
    std::env::var_os("MELANIA_WALLPAPER")
        .map(PathBuf::from).filter(|path|path.is_file())
        .or_else(||{
            if prefs.desktop_wallpaper.is_empty(){None}
            else {
                let path=PathBuf::from(&prefs.desktop_wallpaper);
                path.is_file().then_some(path)
            }
        })
        .unwrap_or_else(||PathBuf::from(WALLPAPER))
}
fn wallpaper_fit(value:&str)->gtk::ContentFit {
    match value {
        "contain"=>gtk::ContentFit::Contain,
        "fill"=>gtk::ContentFit::Fill,
        _=>gtk::ContentFit::Cover,
    }
}

fn desktop_directory() -> PathBuf {
    if let Ok(output) = Command::new("xdg-user-dir").arg("DESKTOP").output() {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !value.is_empty() {
                let path = PathBuf::from(value);
                if path.is_dir() { return path; }
            }
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
    let local = home.join("Scrivania");
    if local.is_dir() { local } else { home.join("Desktop") }
}

fn icon_name(path: &Path) -> &'static str {
    if path.is_dir() {
        let folder = path.file_name().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        return match folder.as_str() {
            "documenti"|"documents" => "folder-documents",
            "immagini"|"pictures"|"images" => "folder-pictures",
            "musica"|"music" => "folder-music",
            "video"|"videos" => "folder-videos",
            "scaricati"|"download"|"downloads" => "folder-download",
            "progetti"|"projects" => "folder-projects",
            _ => "folder",
        };
    } else {
        match path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
            "png"|"jpg"|"jpeg"|"webp"|"svg"|"gif" => "image-x-generic",
            "pdf" => "application-pdf",
            "mp3"|"ogg"|"wav"|"flac" => "audio-x-generic",
            "mp4"|"webm"|"mkv" => "video-x-generic",
            "zip"|"7z"|"tar" => "package-x-generic",
            _ => "text-x-generic",
        }
    }
}

type IconRegistry = Rc<RefCell<Vec<(PathBuf, Button)>>>;
type DesktopSelection = Rc<RefCell<Selection>>;

fn icon_button(path: PathBuf, status: &Label, selection: &DesktopSelection, registry: &IconRegistry) -> Button {
    let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let button = Button::new();
    button.add_css_class("melania-desktop-icon");
    button.set_tooltip_text(Some(&path.display().to_string()));
    let content = GtkBox::new(Orientation::Vertical, 5);
    content.set_halign(Align::Center);
    let icon = Image::from_icon_name(icon_name(&path));
    icon.set_pixel_size(46);
    content.append(&icon);
    let text = Label::new(Some(&name));
    text.set_max_width_chars(15);
    text.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.set_wrap(true);
    text.set_justify(gtk::Justification::Center);
    text.add_css_class("melania-desktop-caption");
    content.append(&text);
    button.set_child(Some(&content));

    // A single click selects. CTRL-click toggles a selection, and only a
    // double-click opens a file: no accidental program launch while dragging.
    let click = gtk::GestureClick::new();
    click.set_button(1);
    let open_path = path.clone();
    let selected = selection.clone();
    let icons = registry.clone();
    let feedback = status.clone();
    click.connect_pressed(move |gesture, count, _, _| {
        let additive = gesture.current_event_state().contains(gdk::ModifierType::CONTROL_MASK);
        selected.borrow_mut().select(open_path.clone(), additive);
        let snapshot = selected.borrow();
        for (path, icon) in icons.borrow().iter() {
            if snapshot.is_selected(path) { icon.add_css_class("melania-icon-selected"); }
            else { icon.remove_css_class("melania-icon-selected"); }
        }
        if count == 2 {
            if let Err(error) = Command::new("xdg-open").arg(&open_path).spawn() {
                feedback.set_text(&format!("Impossibile aprire: {error}"));
            }
        }
    });
    button.add_controller(click);

    let source = gtk::DragSource::new();
    source.set_actions(gdk::DragAction::COPY);
    let dragged = path.clone();
    let selection_for_drag = selection.clone();
    source.connect_prepare(move |_, _, _| {
        let selection = selection_for_drag.borrow();
        // Dragging a selected icon transfers the whole selection, as long as
        // the selection is bounded. Dragging an unselected icon transfers only it.
        let paths = if selection.is_selected(&dragged) {
            selection.paths().take(256).cloned().collect::<Vec<_>>()
        } else { vec![dragged.clone()] };
        let files = paths.iter().map(gio::File::for_path).collect::<Vec<_>>();
        let list = gdk::FileList::from_array(&files);
        Some(gdk::ContentProvider::for_value(&list.to_value()))
    });
    button.add_controller(source);

    // The folder icon itself is a valid drop destination, just like in a
    // classic desktop. All drops are COPY-only until MOVE is verified.
    if path.is_dir() {
        let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        let highlight = button.clone();
        target.connect_enter(move |_, _, _| {
            highlight.add_css_class("melania-folder-drop-ready");
            gdk::DragAction::COPY
        });
        let highlight = button.clone();
        target.connect_leave(move |_| highlight.remove_css_class("melania-folder-drop-ready"));
        let folder = path.clone();
        let feedback = status.clone();
        target.connect_drop(move |_, value, _, _| {
            let Ok(list) = value.get::<gdk::FileList>() else { return false; };
            let mut sources = Vec::new();
            for file in list.files() {
                let Some(local) = file.path() else {
                    feedback.set_text("File remoti: utilizza Melania Files.");
                    return false;
                };
                sources.push(local);
            }
            match melania_transfer::copy_sources(&sources, &folder) {
                Ok(created) if !created.is_empty() => {
                    feedback.set_text(&format!("Copiati {} elementi in {}", created.len(),folder.display()));
                    true
                }
                Ok(_) => false,
                Err(error) => { feedback.set_text(&format!("Copia annullata: {error}")); false }
            }
        });
        button.add_controller(target);
    }
    button
}

fn reload_icons(grid: &FlowBox, directory: &Path, status: &Label,
                selection: &DesktopSelection, registry: &IconRegistry) {
    while let Some(child) = grid.first_child() {
        if let Ok(row) = child.downcast::<gtk::FlowBoxChild>() { grid.remove(&row); }
        else { break; }
    }
    registry.borrow_mut().clear();
    selection.borrow_mut().retain_existing();
    let items = match list_desktop(directory, 73, false) {
        Ok(items) => items,
        Err(_) => {
            status.set_text("Cartella Scrivania assente: configurala nelle impostazioni XDG.");
            return;
        }
    };
    for item in items.iter().take(72) {
        let button = icon_button(item.path.clone(), status, selection, registry);
        if selection.borrow().is_selected(&item.path) {button.add_css_class("melania-icon-selected");}
        registry.borrow_mut().push((item.path.clone(),button.clone()));
        grid.insert(&button, -1);
    }
    if items.len() > 72 { status.set_text("Altri elementi disponibili in Melania Files."); }
}

fn build(app: &Application) {
    setup_melania_icon_theme();
    if !gtk4_layer_shell::is_supported() {
        eprintln!("Melania Desktop requires zwlr_layer_shell_v1; use a Wayland compositor supporting it.");
        return;
    }
    let css = gtk::CssProvider::new();
    css.load_from_data(include_str!("../assets/desktop.css"));
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
    let window = ApplicationWindow::builder()
        .application(app).title("Melania Desktop")
        .decorated(false).default_width(1280).default_height(720).build();
    window.init_layer_shell();
    window.set_namespace(Some("melania-desktop"));
    window.set_layer(Layer::Background);
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        window.set_anchor(edge, true);
    }
    window.set_exclusive_zone(0);
    let overlay = Overlay::new();
    overlay.add_css_class("melania-desktop-root");
    let initial_prefs=melania_config::load().unwrap_or_default();
    let image_path=wallpaper_path(&initial_prefs);
    let wallpaper=Picture::for_filename(&image_path);
    wallpaper.set_content_fit(wallpaper_fit(&initial_prefs.desktop_wallpaper_fit));
    wallpaper.set_can_shrink(true);
    overlay.set_child(Some(&wallpaper));
    let layer = GtkBox::new(Orientation::Vertical, 12);
    layer.set_halign(Align::Start);
    layer.set_valign(Align::Fill);
    layer.set_margin_start(18);
    layer.set_margin_top(20);
    layer.set_margin_bottom(64);
    layer.set_visible(initial_prefs.desktop_show_icons);
    let grid = FlowBox::new();
    grid.set_orientation(Orientation::Vertical);
    grid.set_max_children_per_line(9);
    grid.set_min_children_per_line(1);
    grid.set_selection_mode(gtk::SelectionMode::None);
    grid.set_row_spacing(11);
    grid.set_column_spacing(11);
    grid.add_css_class("melania-icon-grid");
    layer.append(&grid);
    overlay.add_overlay(&layer);
    let message = Label::new(None);
    message.add_css_class("melania-desktop-message");
    message.set_halign(Align::Center);
    message.set_valign(Align::End);
    message.set_margin_bottom(70);
    overlay.add_overlay(&message);
    // Real live reload: Settings writes the shared per-user XDG file, and
    // this desktop changes only its own layer (never the host wallpaper).
    // Retain all GTK updates on the main thread.
    let current_prefs=Rc::new(RefCell::new(initial_prefs));
    let live_prefs=current_prefs.clone();
    let live_picture=wallpaper.clone();
    let live_icons=layer.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(700),move ||{
        if let Ok(next)=melania_config::load() {
            let mut prior=live_prefs.borrow_mut();
            if prior.desktop_wallpaper!=next.desktop_wallpaper {
                let selected=gio::File::for_path(wallpaper_path(&next));
                live_picture.set_file(Some(&selected));
            }
            if prior.desktop_wallpaper_fit!=next.desktop_wallpaper_fit {
                live_picture.set_content_fit(wallpaper_fit(&next.desktop_wallpaper_fit));
            }
            if prior.desktop_show_icons!=next.desktop_show_icons {
                live_icons.set_visible(next.desktop_show_icons);
            }
            *prior=next;
        }
        gtk::glib::ControlFlow::Continue
    });
    let directory = desktop_directory();
    let selected: DesktopSelection = Rc::new(RefCell::new(Selection::default()));
    let icon_registry: IconRegistry = Rc::new(RefCell::new(Vec::new()));
    reload_icons(&grid, &directory, &message, &selected, &icon_registry);
    let drop = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    let grid_after_drop = grid.clone();
    let dir_after_drop = directory.clone();
    let message_after_drop = message.clone();
    let selected_after_drop = selected.clone();
    let icons_after_drop = icon_registry.clone();
    drop.connect_drop(move |_, value, _, _| {
        let Ok(list) = value.get::<gdk::FileList>() else { return false; };
        let mut sources = Vec::new();
        for item in list.files() {
            let Some(local_path) = item.path() else {
                message_after_drop.set_text("Posizioni remote: usa Melania Files.");
                return false;
            };
            sources.push(local_path);
        }
        match melania_transfer::copy_sources(&sources, &dir_after_drop) {
            Ok(created) if !created.is_empty() => {
                reload_icons(&grid_after_drop, &dir_after_drop, &message_after_drop, &selected_after_drop, &icons_after_drop);
                message_after_drop.set_text(&format!("Copiati {} elementi sulla Scrivania", created.len()));
                true
            }
            Ok(_) => false,
            Err(error) => { message_after_drop.set_text(&format!("Copia annullata: {error}")); false }
        }
    });
    overlay.add_controller(drop);
    // Classic desktop context menu: real file operations, never simulated.
    // It belongs to the background and does not replace application menus.
    let context = gtk::Popover::new();
    context.set_parent(&overlay);
    context.set_has_arrow(false);
    context.add_css_class("melania-desktop-context");
    let context_items = GtkBox::new(Orientation::Vertical, 4);
    context_items.set_margin_start(8);
    context_items.set_margin_end(8);
    context_items.set_margin_top(8);
    context_items.set_margin_bottom(8);

    let create = Button::with_label("Nuova cartella");
    let create_dir = directory.clone();
    let create_grid = grid.clone();
    let create_message = message.clone();
    let create_selected = selected.clone();
    let create_registry = icon_registry.clone();
    let create_context = context.clone();
    create.connect_clicked(move |_| {
        create_context.popdown();
        match create_unique_folder(&create_dir) {
            Ok(path) => {
                reload_icons(&create_grid, &create_dir, &create_message, &create_selected, &create_registry);
                create_message.set_text(&format!("Creata {}",path.display()));
            }
            Err(error) => create_message.set_text(&format!("Nuova cartella: {error}")),
        }
    });
    context_items.append(&create);

    let refresh = Button::with_label("Aggiorna desktop");
    let refresh_dir = directory.clone();
    let refresh_grid = grid.clone();
    let refresh_message = message.clone();
    let refresh_selected = selected.clone();
    let refresh_registry = icon_registry.clone();
    let refresh_context = context.clone();
    refresh.connect_clicked(move |_| {
        refresh_context.popdown();
        reload_icons(&refresh_grid, &refresh_dir, &refresh_message, &refresh_selected, &refresh_registry);
    });
    context_items.append(&refresh);
    let files = Button::with_label("Apri Melania Files");
    files.connect_clicked(|_| { let _=Command::new("melania-files").spawn(); });
    context_items.append(&files);
    let settings = Button::with_label("Impostazioni");
    settings.connect_clicked(|_| { let _=Command::new("melania-settings").spawn(); });
    context_items.append(&settings);
    context.set_child(Some(&context_items));
    let menu_click = gtk::GestureClick::new();
    menu_click.set_button(3);
    menu_click.connect_pressed(move |_, _, x, y| {
        context.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
        context.popup();
    });
    overlay.add_controller(menu_click);

    // Live desktop: changes made by other applications appear without restart.
    let file = gio::File::for_path(&directory);
    if let Ok(monitor) = file.monitor_directory(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>) {
        let updated = grid.clone();
        let status = message.clone();
        let base = directory.clone();
        let selected_for_monitor = selected.clone();
        let icons_for_monitor = icon_registry.clone();
        monitor.connect_changed(move |_, _, _, _| {
            reload_icons(&updated, &base, &status, &selected_for_monitor, &icons_for_monitor);
        });
        // Retain the monitor for the life of the desktop window.
        window.connect_hide(move |_| { monitor.cancel(); });
    }
    window.set_child(Some(&overlay));
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

fn main() -> gtk::glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build);
    app.run()
}
