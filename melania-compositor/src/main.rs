//! Melania's initial nested Wayland compositor.
//!
//! Milestone M1: listen on a dedicated Wayland socket, manage xdg-toplevels,
//! route keyboard and pointer focus, advertise wl_data_device (including DnD),
//! and render client surface trees into a host window using Smithay/winit.
//!
//! This is deliberately *not* a display manager / DRM session / full desktop.
//! The compositing and protocol APIs follow Smithay 0.7.0. Run nested only.

use std::{error::Error, ffi::OsString, fs, io::Write, os::{fd::OwnedFd, unix::{fs::PermissionsExt, net::UnixListener}}, path::PathBuf, process::Command, sync::Arc, time::{Duration, Instant}};

use smithay::{
    backend::{
        input::{AbsolutePositionEvent, ButtonState, Event, InputEvent, KeyboardKeyEvent, KeyState, PointerButtonEvent},
        renderer::{
            element::{surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement}, Kind},
            gles::GlesRenderer,
            utils::{draw_render_elements, on_commit_buffer_handler},
            Color32F, Frame, Renderer,
        },
        winit::{self, WinitEvent},
    },
    delegate_compositor, delegate_data_device, delegate_layer_shell, delegate_output, delegate_seat, delegate_shm, delegate_xdg_shell,
    desktop::{layer_map_for_output, LayerSurface as DesktopLayerSurface, Space, Window, WindowSurfaceType},
    output::{Mode, Output, PhysicalProperties, Subpixel},
    input::{
        keyboard::{keysyms, FilterResult},
        pointer::{ButtonEvent, CursorImageStatus, MotionEvent},
        Seat, SeatHandler, SeatState,
    },
    reexports::wayland_server::{
        backend::{ClientData, ClientId, DisconnectReason},
        protocol::{wl_buffer, wl_output::WlOutput, wl_seat, wl_surface::{self, WlSurface}},
        Client, Display, DisplayHandle, ListeningSocket, Resource,
    },
    utils::{Logical, Point, Rectangle, Serial, Transform, SERIAL_COUNTER},
    wayland::{
        buffer::BufferHandler,
        compositor::{with_surface_tree_downward, CompositorClientState, CompositorHandler,
                     CompositorState, SurfaceAttributes, TraversalAction, with_states},
        selection::{data_device::{set_data_device_focus, ClientDndGrabHandler,
                      DataDeviceHandler, DataDeviceState, ServerDndGrabHandler}, SelectionHandler},
        output::{OutputHandler, OutputManagerState},
        shell::{xdg::{PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState, XdgToplevelSurfaceData},
                wlr_layer::{WlrLayerShellHandler, WlrLayerShellState, LayerSurface as WlrLayerSurface, Layer as WlrLayer}},
        shm::{ShmHandler, ShmState},
    },
};
use ::winit::platform::pump_events::PumpStatus;
use wayland_protocols::xdg::shell::server::xdg_toplevel;
use melania_core::{Rect as CoreRect, interaction::{Edges, Interaction, Operation}};
use melania_shell_model::{detect_snap, snap_geometry, Geometry as ShellRect, SnapZone};
use melania_shell_model::keyboard_navigation::{adjacent_workspace, cycle_window_id};
use melania_control::{Command as ControlCommand, WindowInfo, ACK, respond_error, respond_list, read_command, respond_workspace, SOCKET_ENV};
use melania_workspaces::Workspaces;

/// Valid for exactly one pointer press. Requests using old or fabricated serials
/// cannot start a compositor-controlled move or resize.
struct PressRecord { serial: Serial, surface: WlSurface }
struct ActiveInteraction { surface: WlSurface, model: Interaction }

/// Each window gets a compositor-local, monotonically increasing identifier.
/// Wayland resource IDs alone are not globally unique across clients.
struct ManagedWindow {
    id: u64,
    window: Window,
    surface: WlSurface,
    minimized: bool,
    saved_position: (i32, i32),
    /// Geometry before maximizing; necessary to restore rather than guess.
    restore_geometry: Option<CoreRect>,
}

/// Removes a private control socket if Melania terminates normally.
struct SocketCleanup(PathBuf);
impl Drop for SocketCleanup {
    fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
}



/// A socket separate from the host compositor prevents replacing its session.
#[derive(Default)]
struct ClientState {
    compositor: CompositorClientState,
}
impl ClientData for ClientState {
    fn initialized(&self, _: ClientId) {}
    fn disconnected(&self, _: ClientId, _: DisconnectReason) {}
}

struct Melania {
    compositor: CompositorState,
    shell: XdgShellState,
    layer_shell: WlrLayerShellState,
    output: Output,
    shm: ShmState,
    input: SeatState<Self>,
    devices: DataDeviceState,
    outputs: OutputManagerState,
    seat: Seat<Self>,
    /// Stacking, geometry, and hit testing of native Wayland windows.
    space: Space<Window>,
    display: DisplayHandle,
    last_press: Option<PressRecord>,
    interaction: Option<ActiveInteraction>,
    managed: Vec<ManagedWindow>,
    workspaces: Workspaces,
    /// Applies the persisted Aero Snap preference on compositor startup.
    snap_windows: bool,
    next_window_id: u64,
    active_window: Option<u64>,
    exit_requested: bool,
}

impl BufferHandler for Melania {
    fn buffer_destroyed(&mut self, _: &wl_buffer::WlBuffer) {}
}
impl CompositorHandler for Melania {
    fn compositor_state(&mut self) -> &mut CompositorState { &mut self.compositor }
    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().expect("known client").compositor
    }
    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        if std::env::var_os("MELANIA_DEBUG_RENDER").is_some() &&
           self.managed.iter().any(|w| w.surface == *surface) {
            eprintln!("MELANIA_WL_COMMIT toplevel {:?}", surface.id());
        }
        for window in self.space.elements() {
            if window.toplevel().is_some_and(|top| top.wl_surface() == surface) {
                window.on_commit();
            }
        }
        // Recompute panel anchors/exclusive zones on layer commits, not only
        // when the host window is resized. The taskbar's reserved area changes
        // when a layer surface is configured or hidden.
        layer_map_for_output(&self.output).arrange();
    }
}
impl ShmHandler for Melania {
    fn shm_state(&self) -> &ShmState { &self.shm }
}
impl XdgShellHandler for Melania {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState { &mut self.shell }
    fn new_toplevel(&mut self, top: ToplevelSurface) {
        let step = (self.space.elements().count() as i32 % 7) * 34;
        let window = Window::new_wayland_window(top.clone());
        self.space.map_element(window.clone(), (45 + step, 45 + step), true);
        let id = self.next_window_id;
        self.next_window_id = self.next_window_id.saturating_add(1);
        self.workspaces.register(id).expect("monotonic window id");
        self.managed.push(ManagedWindow {
            id, window, surface: top.wl_surface().clone(),
            minimized: false, saved_position: (45 + step, 45 + step),
            restore_geometry: None,
        });
        // Clients only map after the initial xdg configure round-trip.
        top.send_configure();
    }
    fn new_popup(&mut self, popup: PopupSurface, _: PositionerState) {
        let _ = popup.send_configure();
    }
    fn move_request(&mut self, top: ToplevelSurface, _: wl_seat::WlSeat, serial: Serial) {
        self.begin_interaction(&top, serial, Operation::Move);
    }
    fn resize_request(&mut self, top: ToplevelSurface, _: wl_seat::WlSeat,
                      serial: Serial, edges: xdg_toplevel::ResizeEdge) {
        if let Some(edges) = Edges::from_xdg(edges as u32) {
            self.begin_interaction(&top, serial, Operation::Resize(edges));
        }
    }
    fn maximize_request(&mut self, top: ToplevelSurface) {
        self.set_maximized(&top, true);
    }
    fn unmaximize_request(&mut self, top: ToplevelSurface) {
        self.set_maximized(&top, false);
    }
    fn minimize_request(&mut self, top: ToplevelSurface) {
        if let Some(id)=self.managed.iter()
            .find(|w|w.surface==*top.wl_surface()).map(|w|w.id) {
            let _ = self.control(ControlCommand::Minimize(id));
        }
    }
    fn toplevel_destroyed(&mut self, top: ToplevelSurface) {
        if self.interaction.as_ref().is_some_and(|g| g.surface == *top.wl_surface()) {
            self.interaction = None;
        }
        let window = self.space.elements().find(|w| w.toplevel() == Some(&top)).cloned();
        if let Some(window) = window { self.space.unmap_elem(&window); }
        if let Some(index) = self.managed.iter().position(|w| w.surface == *top.wl_surface()) {
            let id = self.managed.remove(index).id;
            self.workspaces.forget(id);
            if self.active_window == Some(id) { self.active_window = None; }
        }
    }
    fn grab(&mut self, _: PopupSurface, _: wl_seat::WlSeat, _: Serial) {}
    fn reposition_request(&mut self, popup: PopupSurface, _: PositionerState, token: u32) {
        popup.send_repositioned(token);
    }
}
// A GTK 4 taskbar is not an xdg_toplevel: it needs wlr-layer-shell.
// Layers are managed separately from ordinary windows. A newly mapped panel
// must receive its initial configure before it can attach a buffer.
impl WlrLayerShellHandler for Melania {
    fn shell_state(&mut self) -> &mut WlrLayerShellState { &mut self.layer_shell }
    fn new_layer_surface(&mut self, surface: WlrLayerSurface,
        _: Option<WlOutput>, _: WlrLayer, namespace: String) {
        let layer = DesktopLayerSurface::new(surface.clone(), namespace);
        if let Err(error) = layer_map_for_output(&self.output).map_layer(&layer) {
            eprintln!("Melania layer map: {error:?}");
            surface.send_close();
            return;
        }
        layer_map_for_output(&self.output).arrange();
        surface.send_configure();
    }
    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        let mapped = layer_map_for_output(&self.output).layers()
            .find(|l| l.wl_surface() == surface.wl_surface()).cloned();
        if let Some(mapped) = mapped { layer_map_for_output(&self.output).unmap_layer(&mapped); }
        layer_map_for_output(&self.output).arrange();
    }
}
impl SeatHandler for Melania {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;
    fn seat_state(&mut self) -> &mut SeatState<Self> { &mut self.input }
    fn focus_changed(&mut self, seat: &Seat<Self>, focus: Option<&WlSurface>) {
        // Necessary for clipboard and wl_data_device DnD focus negotiation.
        let client = focus.and_then(|surface| self.seat_client(surface));
        set_data_device_focus(&self.display, seat, client);
    }
    fn cursor_image(&mut self, _: &Seat<Self>, _: CursorImageStatus) {}
}
impl Melania {
    /// Honour xdg_toplevel maximize/unmaximize requests. This is independent
    /// of the taskbar IPC and respects the exclusive zone occupied by panels.
    fn set_maximized(&mut self, top:&ToplevelSurface, enabled:bool) {
        let Some(w)=self.managed.iter_mut().find(|w|w.surface==*top.wl_surface()) else {
            top.send_configure(); return;
        };
        if enabled {
            let usable=layer_map_for_output(&self.output).non_exclusive_zone();
            if w.restore_geometry.is_none() {
                if let Some(origin)=self.space.element_location(&w.window) {
                    let size=w.window.geometry().size;
                    w.restore_geometry=Some(CoreRect {
                        x:origin.x,y:origin.y,
                        width:size.w.max(1) as u32,height:size.h.max(1) as u32,
                    });
                }
            }
            self.space.map_element(w.window.clone(), (usable.loc.x,usable.loc.y), false);
            top.with_pending_state(|state| {
                state.size=Some((usable.size.w.max(1),usable.size.h.max(1)).into());
                state.states.set(xdg_toplevel::State::Maximized);
            });
        } else {
            if let Some(rect)=w.restore_geometry.take() {
                self.space.map_element(w.window.clone(), (rect.x,rect.y), false);
                top.with_pending_state(|state| {
                    state.size=Some((rect.width as i32,rect.height as i32).into());
                });
            }
            top.with_pending_state(|state|state.states.unset(xdg_toplevel::State::Maximized));
        }
        top.send_pending_configure();
    }

    fn begin_interaction(&mut self, top: &ToplevelSurface, serial: Serial, op: Operation) {
        // Check the implicit pointer press serial and the owner of the surface.
        let Some(press) = self.last_press.take() else { return; };
        if press.serial != serial || press.surface != *top.wl_surface() { return; }
        let Some(window) = self.space.elements()
            .find(|w| w.toplevel() == Some(top)).cloned() else { return; };
        let Some(position) = self.space.element_location(&window) else { return; };
        let size = window.geometry().size;
        let rect = CoreRect {
            x: position.x, y: position.y,
            width: size.w.max(1) as u32, height: size.h.max(1) as u32,
        };
        let location = self.seat.get_pointer().expect("pointer").current_location();
        let model = Interaction::new(rect, (location.x, location.y), op);
        if matches!(op, Operation::Resize(_)) {
            top.with_pending_state(|s| { s.states.set(xdg_toplevel::State::Resizing); });
            top.send_pending_configure();
        }
        self.interaction = Some(ActiveInteraction { surface: top.wl_surface().clone(), model });
    }
    fn update_interaction(&mut self, location: Point<f64, Logical>) {
        let Some(active) = self.interaction.as_ref() else { return; };
        let rect = active.model.updated((location.x, location.y));
        let operation = active.model.operation;
        let surface = active.surface.clone();
        let window = self.space.elements().find(|w| w.toplevel().is_some_and(
            |top| *top.wl_surface() == surface)).cloned();
        let Some(window) = window else { self.interaction = None; return; };
        self.space.map_element(window.clone(), (rect.x, rect.y), false);
        if matches!(operation, Operation::Resize(_)) {
            if let Some(top) = window.toplevel() {
                top.with_pending_state(|s| {
                    s.size = Some((rect.width as i32, rect.height as i32).into());
                });
                top.send_pending_configure();
            }
        }
    }
    fn finish_interaction(&mut self) {
        if let Some(active) = self.interaction.take() {
            if let Some(top) = self.shell.toplevel_surfaces().into_iter().find(|t| *t.wl_surface() == active.surface) {
                // Aero Snap only applies to a completed *window move*.
                // File DnD is owned by wl_data_device and is never interpreted
                // as moving a toplevel window.
                if self.snap_windows && matches!(active.model.operation, Operation::Move) {
                    if let Some(mode) = self.output.current_mode() {
                        let usable = layer_map_for_output(&self.output).non_exclusive_zone();
                        let area = ShellRect::new(usable.loc.x, usable.loc.y,
                            usable.size.w.max(1) as u32,usable.size.h.max(1) as u32);
                        let full = ShellRect::new(0, 0,
                            mode.size.w.max(1) as u32,mode.size.h.max(1) as u32);
                        let position = self.seat.get_pointer().expect("pointer").current_location();
                        if let Some(zone) = detect_snap(position.x as i32,position.y as i32,full,18) {
                            let target = snap_geometry(zone,area);
                            let snapped_window = self.space.elements()
                                .find(|w|w.toplevel().is_some_and(|t| t.wl_surface()==top.wl_surface())).cloned();
                            if let Some(window) = snapped_window {
                                self.space.map_element(window, (target.x,target.y), false);
                                top.with_pending_state(|s| {
                                    s.size=Some((target.width as i32,target.height as i32).into());
                                    if zone == SnapZone::Top {s.states.set(xdg_toplevel::State::Maximized);}
                                    else {s.states.unset(xdg_toplevel::State::Maximized);}
                                });
                            }
                        }
                    }
                }
                top.with_pending_state(|s| { s.states.unset(xdg_toplevel::State::Resizing); });
                top.send_pending_configure();
            }
        }
        self.last_press = None;
    }
    fn seat_client(&self, surface: &WlSurface) -> Option<Client> {
        self.display.get_client(surface.id()).ok()
    }

    /// True hit testing uses Smithay's scene geometry and client input regions.
    /// This is required for an accurate drag target when crossing windows.
    fn surface_at(&self, position: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        let layers = layer_map_for_output(&self.output);
        // Top and Overlay are above application windows. These must be hit-tested
        // first so taskbar, menu and quick settings can actually be clicked.
        for kind in [WlrLayer::Overlay, WlrLayer::Top] {
            if let Some(layer) = layers.layer_under(kind, position) {
                if let Some(geo) = layers.layer_geometry(layer) {
                    if let Some((surface, point)) = layer.surface_under(position - geo.loc.to_f64(), WindowSurfaceType::ALL) {
                        return Some((surface, (point + geo.loc).to_f64()));
                    }
                }
            }
        }
        let normal = self.space.element_under(position).and_then(|(window, origin)| {
            window.surface_under(position - origin.to_f64(), WindowSurfaceType::ALL)
                .map(|(surface, point)| (surface, (point + origin).to_f64()))
        });
        if normal.is_some() { return normal; }
        // Background and Bottom layers must remain below ordinary windows.
        for kind in [WlrLayer::Bottom, WlrLayer::Background] {
            if let Some(layer) = layers.layer_under(kind, position) {
                if let Some(geo) = layers.layer_geometry(layer) {
                    if let Some((surface, point)) = layer.surface_under(position - geo.loc.to_f64(), WindowSurfaceType::ALL) {
                        return Some((surface, (point + geo.loc).to_f64()));
                    }
                }
            }
        }
        None
    }
}
impl Melania {
    /// Switch only compositor window mappings; preserve minimized state and
    /// desktop surface across workspaces. Never treat a workspace switch as
    /// a client-side drag/copy/move operation.
    fn switch_workspace(&mut self, target:u8) -> Result<(), &'static str> {
        self.workspaces.switch(target).map_err(|_|"Spazio di lavoro non valido")?;
        self.interaction=None;
        self.last_press=None;
        for w in &mut self.managed {
            if self.workspaces.is_visible(w.id) && !w.minimized {
                self.space.map_element(w.window.clone(), w.saved_position, true);
            } else {
                if let Some(position)=self.space.element_location(&w.window) {
                    w.saved_position=(position.x,position.y);
                }
                self.space.unmap_elem(&w.window);
            }
        }
        self.active_window=None;
        self.seat.get_keyboard().expect("keyboard").set_focus(self,None,SERIAL_COUNTER.next_serial());
        Ok(())
    }
    fn send_to_workspace(&mut self,id:u64,destination:u8)->Result<(), &'static str> {
        let index=self.managed.iter().position(|w|w.id==id).ok_or("Finestra non trovata")?;
        self.workspaces.send(id,destination).map_err(|_|"Spazio di lavoro non valido")?;
        let w=&mut self.managed[index];
        if self.workspaces.is_visible(id) && !w.minimized {
            self.space.map_element(w.window.clone(),w.saved_position,true);
        } else {
            if let Some(position)=self.space.element_location(&w.window) {
                w.saved_position=(position.x,position.y);
            }
            self.space.unmap_elem(&w.window);
            if self.active_window==Some(id) {
                self.active_window=None;
                self.seat.get_keyboard().expect("keyboard").set_focus(self,None,SERIAL_COUNTER.next_serial());
            }
        }
        Ok(())
    }

    /// Live xdg_toplevel title and application ID, safely encoded by melania-control.
    fn window_snapshot(&self) -> Vec<WindowInfo> {
        // Bound list size: a malicious client can supply arbitrarily long titles.
        // Keep the response comfortably under melania-control::MAX_RESPONSE.
        self.managed.iter().take(96).map(|w| {
            let (title, app_id) = with_states(&w.surface, |states| {
                states.data_map.get::<XdgToplevelSurfaceData>()
                    .map(|data| {
                        let data = data.lock().unwrap();
                        (data.title.clone(), data.app_id.clone())
                    })
                    .unwrap_or((None, None))
            });
            WindowInfo {
                id: w.id,
                title: title.filter(|s| !s.is_empty()).unwrap_or_else(||format!("Finestra {}",w.id)).chars().take(96).collect(),
                app_id: app_id.unwrap_or_else(||"applicazione-wayland".into()).chars().take(96).collect(),
                active: self.active_window == Some(w.id),
                minimized: w.minimized,
            }
        }).collect()
    }

    /// Alt+Tab cycles only visible, non-minimized toplevels on this workspace.
    /// Uses the existing Activate IPC implementation: focus and stacking
    /// remain owned by the compositor, never inferred from the taskbar.
    fn cycle_visible_window(&mut self, backwards: bool) {
        let visible: Vec<u64> = self.managed.iter()
            .filter(|w| self.workspaces.is_visible(w.id) && !w.minimized)
            .map(|w| w.id).collect();
        if let Some(id) = cycle_window_id(&visible, self.active_window, backwards) {
            let _ = self.control(ControlCommand::Activate(id));
        }
    }

    /// Only the compositor may focus, minimize, or close client toplevels.
    fn control(&mut self, command: ControlCommand) -> String {
        if command == ControlCommand::List {return respond_list(&self.window_snapshot());}
        if command == ControlCommand::Status {return respond_workspace(self.workspaces.current());}
        if let ControlCommand::Workspace(n)=command {
            return match self.switch_workspace(n) {Ok(())=>ACK.into(),Err(e)=>respond_error(e)};
        }
        if let ControlCommand::Send(id,n)=command {
            return match self.send_to_workspace(id,n) {Ok(())=>ACK.into(),Err(e)=>respond_error(e)};
        }
        if command == ControlCommand::Quit {
            self.exit_requested=true;
            return ACK.into();
        }
        let id = match command {
            ControlCommand::Activate(id) | ControlCommand::Minimize(id) | ControlCommand::Close(id) => id,
            ControlCommand::List | ControlCommand::Status | ControlCommand::Workspace(_) | ControlCommand::Send(_,_) | ControlCommand::Quit => unreachable!(),
        };
        let Some(index) = self.managed.iter().position(|w| w.id == id) else {
            return respond_error("Finestra non trovata");
        };
        match command {
            ControlCommand::Activate(_) => {
                if let Some(dest)=self.workspaces.workspace_of(id) {
                    if dest!=self.workspaces.current() { let _=self.switch_workspace(dest); }
                }
                let w = &mut self.managed[index];
                if w.minimized {
                    self.space.map_element(w.window.clone(), w.saved_position, true);
                    w.minimized = false;
                }
                self.space.raise_element(&w.window, true);
                let surface = w.surface.clone();
                let serial = SERIAL_COUNTER.next_serial();
                self.seat.get_keyboard().expect("keyboard").set_focus(self, Some(surface), serial);
                self.active_window = Some(id);
            }
            ControlCommand::Minimize(_) => {
                let w = &mut self.managed[index];
                if !w.minimized {
                    if let Some(position) = self.space.element_location(&w.window) {
                        w.saved_position = (position.x, position.y);
                    }
                    self.space.unmap_elem(&w.window);
                    w.minimized = true;
                    if self.active_window == Some(id) {
                        self.active_window = None;
                        let serial = SERIAL_COUNTER.next_serial();
                        self.seat.get_keyboard().expect("keyboard").set_focus(self,None,serial);
                    }
                }
            }
            ControlCommand::Close(_) => {
                if let Some(top) = self.managed[index].window.toplevel() { top.send_close(); }
            }
            ControlCommand::List | ControlCommand::Status | ControlCommand::Workspace(_) | ControlCommand::Send(_,_) | ControlCommand::Quit => unreachable!(),
        }
        ACK.into()
    }
}

impl OutputHandler for Melania {}
delegate_output!(Melania);

impl SelectionHandler for Melania {
    type SelectionUserData = ();
}
impl DataDeviceHandler for Melania {
    fn data_device_state(&self) -> &DataDeviceState { &self.devices }
}
impl ClientDndGrabHandler for Melania {}
impl ServerDndGrabHandler for Melania {
    fn send(&mut self, _: String, _: OwnedFd, _: Seat<Self>) {}
}

delegate_compositor!(Melania);
delegate_layer_shell!(Melania);
delegate_shm!(Melania);
delegate_xdg_shell!(Melania);
delegate_seat!(Melania);
delegate_data_device!(Melania);

/// Frame callbacks allow clients to animate without busy-looping themselves.
fn frame_done(surface: &WlSurface, elapsed_ms: u32) {
    with_surface_tree_downward(
        surface, (),
        |_, _, &()| TraversalAction::DoChildren(()),
        |_, attributes, &()| {
            for callback in attributes.cached_state.get::<SurfaceAttributes>()
                .current().frame_callbacks.drain(..) {
                callback.done(elapsed_ms);
            }
        },
        |_, _, &()| true,
    );
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut display = Display::<Melania>::new()?;
    let mut handle = display.handle();
    let mut seats = SeatState::new();
    let mut seat = seats.new_wl_seat(&handle, "melania-seat0");
    seat.add_keyboard(Default::default(), 250, 40)?;
    seat.add_pointer();
    let (mut backend, mut host) = winit::init::<GlesRenderer>()?;
    let output = Output::new("Melania Nested Output".into(), PhysicalProperties {
        size: (0, 0).into(), subpixel: Subpixel::Unknown,
        make: "Melania".into(), model: "Nested Display".into(),
    });
    let _output_global = output.create_global::<Melania>(&handle);
    let initial_mode = Mode { size: backend.window_size(), refresh: 60_000 };
    output.change_current_state(Some(initial_mode), Some(Transform::Flipped180), None,
                                Some((0, 0).into()));
    output.set_preferred(initial_mode);
    // Preference changes take effect after restarting the nested session.
    // This avoids silently remapping a user's currently open windows.
    let preferences = melania_config::load().unwrap_or_default();
    let mut state = Melania {
        compositor: CompositorState::new::<Melania>(&handle),
        shell: XdgShellState::new::<Melania>(&handle),
        layer_shell: WlrLayerShellState::new::<Melania>(&handle),
        output: output.clone(),
        shm: ShmState::new::<Melania>(&handle, vec![]),
        input: seats,
        devices: DataDeviceState::new::<Melania>(&handle),
        outputs: OutputManagerState::new_with_xdg_output::<Melania>(&handle),
        seat,
        space: Space::default(),
        display: handle.clone(),
        last_press: None,
        interaction: None,
        managed: Vec::new(),
        workspaces: Workspaces::new(preferences.workspace_count),
        snap_windows: preferences.snap_windows,
        next_window_id: 1,
        active_window: None,
        exit_requested: false,
    };
    state.space.map_output(&output, (0, 0));
    let listener = ListeningSocket::bind_auto("melania", 1..=32)?;
    let socket: OsString = listener.socket_name().expect("named socket").to_owned();
    // This socket is separate from the Wayland protocol socket. It serves
    // trusted user-session clients (panel/CLI) and is not exposed system-wide.
    let runtime = PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or("XDG_RUNTIME_DIR is required to create the private control socket")?);
    let control_path = runtime.join(format!("melania-{}-control.sock", std::process::id()));
    let control_listener = UnixListener::bind(&control_path)?;
    fs::set_permissions(&control_path, fs::Permissions::from_mode(0o600))?;
    control_listener.set_nonblocking(true)?;
    let _control_cleanup = SocketCleanup(control_path.clone());
    println!("Melania nested Wayland ready: WAYLAND_DISPLAY={}", socket.to_string_lossy());
    println!("Melania taskbar control socket: {}", control_path.display());
    let begin = Instant::now();
    // Opt-in instrumentation for nested software-rendering CI diagnostics.
    let debug_render = std::env::var_os("MELANIA_DEBUG_RENDER").is_some();
    let mut last_render_report = Instant::now();

    // Optional clients. Each --command PROGRAM starts inside our nested
    // compositor, with no changes to the parent session's environment.
    // These are test clients; the preview is not yet a true layer-shell.
    let mut children = Vec::new();
    let mut arguments = std::env::args_os().skip(1);
    while let Some(flag) = arguments.next() {
        if flag != "--command" {
            return Err(format!("Unknown option {:?}; use --command /path/to/program", flag).into());
        }
        let program = arguments.next().ok_or("--command requires a program")?;
        children.push(Command::new(program).env("WAYLAND_DISPLAY", &socket)
            .env(SOCKET_ENV, &control_path)
            .env("XDG_SESSION_TYPE", "wayland")
            .env("XDG_CURRENT_DESKTOP", "Melania")
            .env("GDK_BACKEND", "wayland").spawn()?);
    }
    loop {
        let status = host.dispatch_new_events(|event| match event {
            WinitEvent::Resized { size, .. } => {
                output.change_current_state(Some(Mode { size, refresh: 60_000 }), None, None, None);
                layer_map_for_output(&output).arrange();
            }
            WinitEvent::Input(input) => match input {
                InputEvent::Keyboard { event, .. } => {
                    let serial = SERIAL_COUNTER.next_serial();
                    let keyboard = state.seat.get_keyboard().expect("keyboard");
                    let key_state = event.state();
                    let mut open_files = false;
                    keyboard.input::<(), _>(&mut state, event.key_code(), key_state, serial,
                        event.time_msec(), |data, modifiers, keysym| {
                            let symbol = keysym.modified_sym();
                            let pressed = key_state == KeyState::Pressed;
                            // Avoid overriding a client's Ctrl+Tab or AltGr
                            // input; only explicitly reserved combinations.
                            if modifiers.alt && !modifiers.ctrl && !modifiers.logo &&
                                (symbol == keysyms::KEY_Tab.into() || symbol == keysyms::KEY_ISO_Left_Tab.into()) {
                                if pressed {
                                    data.cycle_visible_window(modifiers.shift ||
                                        symbol == keysyms::KEY_ISO_Left_Tab.into());
                                }
                                return FilterResult::Intercept(());
                            }
                            if modifiers.ctrl && modifiers.alt && !modifiers.logo &&
                                (symbol == keysyms::KEY_Left.into() || symbol == keysyms::KEY_Right.into()) {
                                if pressed {
                                    if let Some(next) = adjacent_workspace(
                                        data.workspaces.current(), data.workspaces.count(),
                                        symbol == keysyms::KEY_Left.into()) {
                                        let _ = data.switch_workspace(next);
                                    }
                                }
                                return FilterResult::Intercept(());
                            }
                            if modifiers.logo && !modifiers.ctrl && !modifiers.alt &&
                                (symbol == keysyms::KEY_e.into() || symbol == keysyms::KEY_E.into()) {
                                if pressed {open_files = true;}
                                return FilterResult::Intercept(());
                            }
                            FilterResult::Forward
                        });
                    if open_files {
                        // A fixed, known program; no shell or interpolated input.
                        // Launch only inside the nested Melania Wayland session.
                        if let Err(e)=Command::new("melania-files")
                            .env("WAYLAND_DISPLAY", &socket)
                            .env(SOCKET_ENV, &control_path)
                            .env("XDG_SESSION_TYPE", "wayland")
                            .env("XDG_CURRENT_DESKTOP", "Melania")
                            .env("GDK_BACKEND", "wayland").spawn() {
                            eprintln!("Melania shortcut Super+E: {e}");
                        }
                    }
                }
                InputEvent::PointerMotionAbsolute { event, .. } => {
                    let point = event.position_transformed((backend.window_size().w, backend.window_size().h).into());
                    state.update_interaction(point);
                    let under = if state.interaction.is_some() { None } else { state.surface_at(point) };
                    let pointer = state.seat.get_pointer().expect("pointer");
                    pointer.motion(&mut state, under, &MotionEvent {
                        location: point,
                        serial: SERIAL_COUNTER.next_serial(), time: event.time_msec(),
                    });
                    pointer.frame(&mut state);
                }
                InputEvent::PointerButton { event, .. } => {
                    let pointer = state.seat.get_pointer().expect("pointer");
                    let serial = SERIAL_COUNTER.next_serial();
                    if event.state() == ButtonState::Pressed && !pointer.is_grabbed() {
                        // The serial must originate from the same surface the
                        // client subsequently requests to move/resize.
                        let target=state.surface_at(pointer.current_location());
                        // A click on a popup, panel or desktop background must
                        // NEVER authorize an xdg_toplevel move below that layer.
                        state.last_press = target.as_ref().and_then(|(surface, _)| {
                            state.managed.iter().find(|w|w.surface==*surface)
                                .map(|w| PressRecord {serial,surface:surface.clone()})
                        });
                        if let Some((surface, _)) = target {
                            if let Some(w) = state.managed.iter().find(|w| w.surface == surface) {
                                state.active_window = Some(w.id);
                            }
                            state.seat.get_keyboard().expect("keyboard")
                                .set_focus(&mut state, Some(surface), serial);
                        }
                        let layer_intercepts = {
                            let map=layer_map_for_output(&output);
                            map.layer_under(WlrLayer::Overlay,pointer.current_location()).is_some()
                                || map.layer_under(WlrLayer::Top,pointer.current_location()).is_some()
                        };
                        if !layer_intercepts {
                            if let Some((window, _)) = state.space.element_under(pointer.current_location()) {
                                let window = window.clone();
                                state.space.raise_element(&window, true);
                            }
                        }
                    }
                    pointer.button(&mut state, &ButtonEvent {
                        button: event.button_code(), state: event.state(), serial, time: event.time_msec(),
                    });
                    pointer.frame(&mut state);
                    if event.state() == ButtonState::Released {
                        state.finish_interaction();
                    }
                }
                _ => {}
            },
            _ => {}
        });
        if matches!(status, PumpStatus::Exit(_)) { break; }
        let size = backend.window_size();
        let damage = Rectangle::from_size(size);
        {
            let (renderer, mut framebuffer) = backend.bind()?;
            let mut elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = Vec::new();
            // Smithay draw_render_elements consumes FRONT-TO-BACK elements:
            // top/overlay first, then toplevels, then bottom/background.
            // The previous reversed stack allowed the full-screen wallpaper
            // to occlude every application, despite valid Wayland windows.
            {
                let map = layer_map_for_output(&output);
                for kind in [WlrLayer::Overlay,WlrLayer::Top] {
                    for layer in map.layers_on(kind) {
                        if let Some(geo)=map.layer_geometry(layer) {
                            elements.extend(render_elements_from_surface_tree(renderer,layer.wl_surface(),
                                (geo.loc.x,geo.loc.y),1.0,1.0,Kind::Unspecified));
                        }
                    }
                }
            }
            let top_render_elements = elements.len();
            elements.extend(state.space.elements().rev().filter_map(|window| {
                let origin=state.space.element_location(window)?;
                Some((window.toplevel()?.wl_surface().clone(),origin))
            }).flat_map(|(surface,origin)| {
                render_elements_from_surface_tree(renderer,&surface,(origin.x,origin.y),1.0,1.0,Kind::Unspecified)
            }));
            let xdg_render_elements = elements.len() - top_render_elements;
            {
                let map=layer_map_for_output(&output);
                for kind in [WlrLayer::Bottom,WlrLayer::Background] {
                    for layer in map.layers_on(kind) {
                        if let Some(geo)=map.layer_geometry(layer) {
                            elements.extend(render_elements_from_surface_tree(renderer,layer.wl_surface(),
                                (geo.loc.x,geo.loc.y),1.0,1.0,Kind::Unspecified));
                        }
                    }
                }
            }
            if debug_render && last_render_report.elapsed() >= Duration::from_secs(1) {
                eprintln!("MELANIA_RENDER windows={} layers={} elements={} xdg={} top={} size={}x{}",
                          state.space.elements().count(),
                          layer_map_for_output(&output).layers().count(),
                          elements.len(), xdg_render_elements, top_render_elements, size.w, size.h);
                last_render_report = Instant::now();
            }
            let mut frame = renderer.render(&mut framebuffer, size, Transform::Flipped180)?;
            frame.clear(Color32F::new(0.035, 0.08, 0.18, 1.0), &[damage])?;
            draw_render_elements(&mut frame, 1.0, &elements, &[damage])?;
            frame.finish()?;
            for top in state.shell.toplevel_surfaces() {
                frame_done(top.wl_surface(), begin.elapsed().as_millis() as u32);
            }
            for layer in layer_map_for_output(&output).layers() {
                frame_done(layer.wl_surface(), begin.elapsed().as_millis() as u32);
            }
        }
        while let Some(stream) = listener.accept()? {
            handle.insert_client(stream, Arc::new(ClientState::default()))?;
        }
        // Process bounded, single-line local taskbar requests without threads.
        // Timeouts prevent a stalled client from freezing the rendering loop.
        for _ in 0..8 {
            match control_listener.accept() {
                Ok((mut client, _)) => {
                    let _ = client.set_read_timeout(Some(Duration::from_millis(40)));
                    let _ = client.set_write_timeout(Some(Duration::from_millis(40)));
                    // Read up to EOF, with a strict length bound, before acting.
                    // A fragmented or truncated CLOSE must never execute.
                    let answer = match read_command(&mut client) {
                        Ok(Some(cmd)) => state.control(cmd),
                        Ok(None) => respond_error("Comando IPC incompleto o non valido"),
                        Err(e) => respond_error(&format!("Lettura IPC fallita: {e}")),
                    };
                    let _ = client.write_all(answer.as_bytes());
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {eprintln!("Melania control socket: {e}");break;}
            }
        }
        if state.exit_requested { break; }
        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;
        backend.submit(Some(&[damage]))?;
        children.retain_mut(|c| c.try_wait().ok().flatten().is_none());
        state.space.refresh();
    }
    // Child clients belong to the nested compositor, not the host session.
    // Do not leave stray panel/files/settings processes after QUIT.
    for mut child in children {
        let _=child.kill();
        let _=child.wait();
    }
    Ok(())
}
