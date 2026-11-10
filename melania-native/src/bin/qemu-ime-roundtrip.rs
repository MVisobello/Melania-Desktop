//! Two actual separate Wayland client processes against Smithay's IME-v2 and
//! text-input-v3. DISPOSABLE QEMU ONLY. This is a wire roundtrip, not a
//! graphical input method or a release-ready desktop compositor.
use std::{
    error::Error, fs::OpenOptions, io::Write, os::fd::AsFd,
    thread, time::{Duration, Instant},
};
use wayland_client::{
    protocol::{
        wl_buffer, wl_compositor, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
    },
    Connection, Dispatch, QueueHandle,
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3, zwp_text_input_v3,
};
use wayland_protocols::xdg::shell::client::{
    xdg_surface, xdg_toplevel, xdg_wm_base,
};
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_manager_v2, zwp_input_method_v2,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const TEXT: &str = "Procida è già connessa";
// Non-ASCII preedit: offsets in the Wayland protocol are UTF-8 bytes.
const PREEDIT: &str = "città";
const W: usize = 360;
const H: usize = 224;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Mode {
    #[default]
    Editor,
    Ime,
    Intruder,
}
#[derive(Default)]
struct State {
    mode: Mode,
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    seat: Option<wl_seat::WlSeat>,
    wm: Option<xdg_wm_base::XdgWmBase>,
    text_manager: Option<zwp_text_input_manager_v3::ZwpTextInputManagerV3>,
    ime_manager: Option<zwp_input_method_manager_v2::ZwpInputMethodManagerV2>,
    unexpected_privileged_ime_global: bool,
    configured: bool,
    editor_surface: Option<wl_surface::WlSurface>,
    unexpected_text_focus: bool,
    text_entered: bool,
    ime_activated: bool,
    ime_deactivated: bool,
    ime_done: u32,
    text_received: Option<String>,
    preedit_received: Option<(String, i32, i32)>,
    deletion_received: Option<(u32, u32)>,
    surrounding_received: Option<(String, u32, u32)>,
    text_done: u32,
    payload_done: bool,
}
impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self, registry: &wl_registry::WlRegistry,
        event: wl_registry::Event, _: &(), _: &Connection, qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_compositor" => state.compositor =
                    Some(registry.bind(name, version.min(4), qh, ())),
                "wl_shm" => state.shm =
                    Some(registry.bind(name, version.min(1), qh, ())),
                "wl_seat" => state.seat =
                    Some(registry.bind(name, version.min(7), qh, ())),
                "xdg_wm_base" => state.wm =
                    Some(registry.bind(name, version.min(3), qh, ())),
                "zwp_text_input_manager_v3" => state.text_manager =
                    Some(registry.bind(name, version.min(1), qh, ())),
                "zwp_input_method_manager_v2" if state.mode == Mode::Ime => {
                    state.ime_manager = Some(registry.bind(name, version.min(1), qh, ()));
                }
                "zwp_input_method_manager_v2" => {
                    state.unexpected_privileged_ime_global = true;
                }
                _ => {}
            }
        }
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(_: &mut Self, _: &wl_seat::WlSeat, _: wl_seat::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_compositor::WlCompositor, ()> for State {
    fn event(_: &mut Self, _: &wl_compositor::WlCompositor,
             _: wl_compositor::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_shm::WlShm, ()> for State {
    fn event(_: &mut Self, _: &wl_shm::WlShm, _: wl_shm::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_shm_pool::WlShmPool, ()> for State {
    fn event(_: &mut Self, _: &wl_shm_pool::WlShmPool, _: wl_shm_pool::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_buffer::WlBuffer, ()> for State {
    fn event(_: &mut Self, _: &wl_buffer::WlBuffer, _: wl_buffer::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<wl_surface::WlSurface, ()> for State {
    fn event(_: &mut Self, _: &wl_surface::WlSurface, _: wl_surface::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
    fn event(_: &mut Self, wm: &xdg_wm_base::XdgWmBase, event: xdg_wm_base::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm.pong(serial);
        }
    }
}
impl Dispatch<xdg_surface::XdgSurface, ()> for State {
    fn event(state: &mut Self, role: &xdg_surface::XdgSurface, event: xdg_surface::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {
        if let xdg_surface::Event::Configure { serial } = event {
            role.ack_configure(serial);
            state.configured = true;
        }
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel, ()> for State {
    fn event(_: &mut Self, _: &xdg_toplevel::XdgToplevel, _: xdg_toplevel::Event,
             _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<zwp_text_input_manager_v3::ZwpTextInputManagerV3, ()> for State {
    fn event(_: &mut Self, _: &zwp_text_input_manager_v3::ZwpTextInputManagerV3,
             _: zwp_text_input_manager_v3::Event, _: &(), _: &Connection,
             _: &QueueHandle<Self>) {}
}
impl Dispatch<zwp_text_input_v3::ZwpTextInputV3, ()> for State {
    fn event(state: &mut Self, _: &zwp_text_input_v3::ZwpTextInputV3,
             event: zwp_text_input_v3::Event, _: &(), _: &Connection,
             _: &QueueHandle<Self>) {
        match event {
            zwp_text_input_v3::Event::Enter { surface } => {
                // The compositor must route focus to THIS genuine XDG editor,
                // not just to some same-client or roleless Wayland object.
                if state.editor_surface.as_ref() != Some(&surface) {
                    state.unexpected_text_focus = true;
                    return;
                }
                state.text_entered = true;
                println!("MELANIA_QEMU_REAL_TEXT_INPUT_ENTERED=true");
                let _ = std::io::stdout().flush();
            }
            zwp_text_input_v3::Event::Leave { surface } => {
                if state.editor_surface.as_ref() == Some(&surface) {
                    state.text_entered = false;
                }
            }
            zwp_text_input_v3::Event::PreeditString {
                text, cursor_begin, cursor_end,
            } => {
                if state.text_entered && !state.unexpected_text_focus {
                    if let Some(text) = text {
                        state.preedit_received = Some((text, cursor_begin, cursor_end));
                    }
                } else {
                    state.unexpected_text_focus = true;
                }
            }
            zwp_text_input_v3::Event::DeleteSurroundingText {
                before_length, after_length,
            } => {
                if state.text_entered && !state.unexpected_text_focus {
                    state.deletion_received = Some((before_length, after_length));
                } else {
                    state.unexpected_text_focus = true;
                }
            }
            zwp_text_input_v3::Event::CommitString { text } => {
                // text-input-v3 carries a nullable UTF-8 commit string.
                // None is not a real committed character sequence.
                if state.text_entered && !state.unexpected_text_focus {
                    state.text_received = text;
                } else {
                    state.unexpected_text_focus = true;
                }
            }
            zwp_text_input_v3::Event::Done { .. } => {
                state.text_done = state.text_done.saturating_add(1);
                // Wayland text-input-v3 events are double-buffered. Only an
                // actual server Done AFTER all three payload events proves
                // that this complete transaction was dispatched atomically.
                if state.text_received.is_some()
                    && state.preedit_received.is_some()
                    && state.deletion_received.is_some()
                    && state.text_entered && !state.unexpected_text_focus {
                    state.payload_done = true;
                }
            }
            _ => {}
        }
    }
}
impl Dispatch<zwp_input_method_manager_v2::ZwpInputMethodManagerV2, ()> for State {
    fn event(_: &mut Self, _: &zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
             _: zwp_input_method_manager_v2::Event, _: &(), _: &Connection,
             _: &QueueHandle<Self>) {}
}
impl Dispatch<zwp_input_method_v2::ZwpInputMethodV2, ()> for State {
    fn event(state: &mut Self, _: &zwp_input_method_v2::ZwpInputMethodV2,
             event: zwp_input_method_v2::Event, _: &(), _: &Connection,
             _: &QueueHandle<Self>) {
        match event {
            zwp_input_method_v2::Event::Activate => {
                state.ime_activated = true;
                println!("MELANIA_QEMU_REAL_IME_ACTIVATED=true");
                let _ = std::io::stdout().flush();
            }
            zwp_input_method_v2::Event::Deactivate => {
                state.ime_deactivated = true;
                println!("MELANIA_QEMU_REAL_IME_DEACTIVATED=true");
                let _ = std::io::stdout().flush();
            }
            zwp_input_method_v2::Event::SurroundingText { text, cursor, anchor } => {
                state.surrounding_received = Some((text, cursor, anchor));
            }
            zwp_input_method_v2::Event::Done => {
                state.ime_done = state.ime_done.saturating_add(1);
            }
            zwp_input_method_v2::Event::Unavailable => {
                state.ime_activated = false;
                eprintln!("MELANIA_QEMU_IME_ROLE_ALREADY_OWNED=true");
            }
            _ => {}
        }
    }
}
fn roundtrip_until<F>(
    queue: &mut wayland_client::EventQueue<State>, state: &mut State,
    seconds: u64, mut condition: F,
) -> Result<()>
where
    F: FnMut(&State) -> bool,
{
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while !condition(state) {
        if Instant::now() >= deadline {
            return Err("QEMU real IME Wayland protocol event timed out".into());
        }
        queue.roundtrip(state)?;
        thread::sleep(Duration::from_millis(12));
    }
    Ok(())
}
fn run_ime(mut queue: wayland_client::EventQueue<State>, mut state: State) -> Result<()> {
    let qh = queue.handle();
    let mgr = state.ime_manager.take().ok_or("kernel PID-gated input-method global absent")?;
    let seat = state.seat.clone().ok_or("real wl_seat missing for input method")?;
    let method = mgr.get_input_method(&seat, &qh, ());
    queue.roundtrip(&mut state)?;
    println!("MELANIA_QEMU_TRUSTED_IME_MANAGER_BOUND=true");
    std::io::stdout().flush()?;
    // Send deletion only after genuine surrounding text arrives over Wayland.
    roundtrip_until(&mut queue, &mut state, 20, |s|
        s.ime_activated && s.ime_done > 0
        && s.surrounding_received.as_ref().is_some_and(|(t,c,a)|
            t == "prova" && *c == 5 && *a == 5))?;
    println!("MELANIA_QEMU_REAL_IME_SURROUNDING_TEXT_RECEIVED=true");
    // Smithay emits a Done event for activation and again for each editor
    // state commit. Use the latest REAL serial, never a fabricated constant.
    let serial = state.ime_done;
    // One atomic input-method-v2 commit, not an in-process injection.
    method.delete_surrounding_text(5, 0);
    method.commit_string(TEXT.into());
    method.set_preedit_string(PREEDIT.into(), PREEDIT.len() as i32, PREEDIT.len() as i32);
    method.commit(serial);
    queue.roundtrip(&mut state)?;
    println!("MELANIA_QEMU_REAL_IME_UTF8_COMMIT_SENT=true");
    std::io::stdout().flush()?;
    roundtrip_until(&mut queue, &mut state, 12, |s| s.ime_deactivated)?;
    method.destroy();
    mgr.destroy();
    Ok(())
}
fn run_editor(mut queue: wayland_client::EventQueue<State>, mut state: State) -> Result<()> {
    if state.unexpected_privileged_ime_global {
        return Err("untrusted editor was offered privileged input-method manager".into());
    }
    let qh = queue.handle();
    let mgr = state.text_manager.take().ok_or("text-input-v3 Wayland global missing")?;
    let compositor = state.compositor.take().ok_or("wl_compositor missing")?;
    let seat = state.seat.clone().ok_or("wl_seat missing")?;
    let wm = state.wm.take().ok_or("xdg_wm_base missing")?;
    let shm = state.shm.take().ok_or("wl_shm missing")?;
    let text = mgr.get_text_input(&seat, &qh, ());
    let surface = compositor.create_surface(&qh, ());
    state.editor_surface = Some(surface.clone());
    let role = wm.get_xdg_surface(&surface, &qh, ());
    let top = role.get_toplevel(&qh, ());
    top.set_title("Melania real IME UTF8 editor".into());
    surface.commit();
    roundtrip_until(&mut queue, &mut state, 7, |s| s.configured)?;
    // Real Wayland wl_shm-backed XDG client surface, not a model-only edit.
    let path = format!("/run/melania-native-qemu/ime-editor-{}.bgra",
                       std::process::id());
    let pixels: Vec<u8> = (0..W * H).flat_map(|_| [58u8, 112, 181, 0]).collect();
    let mut file = OpenOptions::new().create_new(true).read(true).write(true).open(path)?;
    file.write_all(&pixels)?;
    file.sync_all()?;
    let pool = shm.create_pool(file.as_fd(), pixels.len() as i32, &qh, ());
    let buffer = pool.create_buffer(0, W as i32, H as i32, (W * 4) as i32,
                                    wl_shm::Format::Xrgb8888, &qh, ());
    surface.attach(Some(&buffer), 0, 0);
    surface.damage_buffer(0, 0, W as i32, H as i32);
    surface.commit();
    roundtrip_until(&mut queue, &mut state, 8, |s| s.text_entered)?;
    if state.unexpected_text_focus {
        return Err("text-input-v3 entered a different Wayland XDG surface".into());
    }
    println!("MELANIA_QEMU_REAL_EDITOR_NO_PRIVILEGED_IME_GLOBAL=true");
    text.enable();
    text.set_surrounding_text("prova".into(), 5, 5);
    text.commit();
    queue.roundtrip(&mut state)?;
    // A genuine second client must have sent the UTF-8 string and
    // Smithay must have committed it into this focused text-input-v3.
    roundtrip_until(&mut queue, &mut state, 12, |s|
        s.text_received.is_some()
        && s.preedit_received.is_some()
        && s.deletion_received.is_some()
        && s.payload_done)?;
    if !state.text_entered || state.unexpected_text_focus {
        return Err("IME text delivered without active actual editor focus".into());
    }
    if state.text_received.as_deref() != Some(TEXT) {
        return Err(format!("IME sent wrong actual UTF8: {:?}",state.text_received).into());
    }
    // Cursor 6 is a byte offset: the final non-ASCII character is two bytes.
    if state.preedit_received.as_ref().is_none_or(|(t,c,a)|
        t != PREEDIT || *c != PREEDIT.len() as i32 || *a != PREEDIT.len() as i32) {
        return Err(format!("bad real preedit UTF8 byte cursor: {:?}",state.preedit_received).into());
    }
    if state.deletion_received != Some((5,0)) {
        return Err(format!("bad real surrounding-text deletion: {:?}",state.deletion_received).into());
    }
    println!("MELANIA_QEMU_REAL_EDITOR_IME_UTF8_ROUNDTRIP=true");
    println!("MELANIA_QEMU_REAL_EDITOR_IME_PREEDIT_DELETE_ROUNDTRIP=true");
    println!("MELANIA_QEMU_REAL_EDITOR_IME_ATOMIC_DONE=true");
    std::io::stdout().flush()?;
    text.disable();
    text.commit();
    queue.roundtrip(&mut state)?;
    text.destroy();
    top.destroy();
    role.destroy();
    surface.destroy();
    buffer.destroy();
    pool.destroy();
    Ok(())
}
fn run_intruder(state: State) -> Result<()> {
    if state.ime_manager.is_some() || state.unexpected_privileged_ime_global {
        return Err("untrusted Wayland PID could see input-method-v2 manager".into());
    }
    if state.text_manager.is_none() {
        return Err("untrusted regular editor lacked text-input-v3 manager".into());
    }
    println!("MELANIA_QEMU_UNTRUSTED_IME_GLOBAL_REJECTED=true");
    std::io::stdout().flush()?;
    Ok(())
}
fn main() -> Result<()> {
    if std::env::var("MELANIA_QEMU_IME_ROUNDTRIP").as_deref() != Ok("YES_QEMU_VM") {
        return Err("real IME roundtrip only in disposable QEMU guest".into());
    }
    let mode = match std::env::args().nth(1).as_deref() {
        Some("ime") => Mode::Ime,
        Some("editor") => Mode::Editor,
        Some("intruder") => Mode::Intruder,
        _ => return Err("usage: qemu-ime-roundtrip ime|editor|intruder [socket-marker]".into()),
    };
    if mode == Mode::Ime {
        let marker = std::env::args().nth(2)
            .ok_or("IME needs a private server-created WAYLAND_DISPLAY marker path")?;
        let deadline = Instant::now() + Duration::from_secs(15);
        while !std::path::Path::new(&marker).exists() {
            if Instant::now() >= deadline {
                return Err("trusted IME never received real compositor socket name".into());
            }
            thread::sleep(Duration::from_millis(20));
        }
        let socket = std::fs::read_to_string(marker)?;
        let socket = socket.trim();
        if !socket.starts_with("melania-native-seat-") || socket.contains('/') {
            return Err("unexpected socket marker in isolated QEMU IME test".into());
        }
        std::env::set_var("WAYLAND_DISPLAY", socket);
    }
    let connection = Connection::connect_to_env()?;
    let mut queue = connection.new_event_queue::<State>();
    let qh = queue.handle();
    let _registry = connection.display().get_registry(&qh, ());
    let mut state = State { mode, ..Default::default() };
    queue.roundtrip(&mut state)?;
    match mode {
        Mode::Ime => run_ime(queue, state),
        Mode::Editor => run_editor(queue, state),
        Mode::Intruder => run_intruder(state),
    }
}
