//! QEMU-only native Wayland seat/keyboard/pointer bridge.
 //!
 //! This is a real Smithay wl_compositor + wl_seat endpoint receiving libinput
 //! from libseat in a dedicated VM console, NOT a full graphical compositor.
 //! A client can create a wl_surface and receive keyboard/pointer events, but
 //! KMS scanout of client buffers and native login remain OFF. A separate
//! QEMU-only shell mode checks real xdg_toplevel and layer-surface configure
//! roundtrips without claiming these clients are displayed.
use std::{
    error::Error,
    fs,
    io::{Read,Write},
    os::unix::{fs::PermissionsExt,net::{UnixDatagram,UnixListener,UnixStream}},
    path::{Path,PathBuf},
    sync::{Arc,atomic::{AtomicBool,Ordering}},
    time::{Duration,Instant},
};
use smithay::{
    backend::{
        input::{InputEvent, ButtonState, KeyboardKeyEvent, PointerButtonEvent, PointerMotionEvent,
                AbsolutePositionEvent, PointerAxisEvent, Axis, AxisSource, Event, KeyState},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Session, Event as SessionEvent},
        udev::{UdevBackend,UdevEvent},
    },
    delegate_compositor, delegate_seat, delegate_xdg_shell, delegate_layer_shell, delegate_shm, delegate_output,
    delegate_data_device,
    input::{keyboard::FilterResult, pointer::{AxisFrame, ButtonEvent, CursorImageStatus,
           MotionEvent}, Seat, SeatHandler, SeatState},
    reexports::{
        input::{self as libinput, DeviceCapability, Libinput},
        wayland_server::{
            backend::{ClientData,ClientId,DisconnectReason},
            protocol::{wl_buffer,wl_surface::WlSurface,wl_seat,wl_output::WlOutput},
            Client, Display, ListeningSocket,
        },
    },
    utils::{Logical, Point, Serial, Transform, SERIAL_COUNTER},
    output::{Mode, Output, PhysicalProperties, Subpixel},
    wayland::{
        buffer::BufferHandler,
        compositor::{with_states,with_surface_tree_downward,TraversalAction,BufferAssignment,SurfaceAttributes,CompositorClientState,CompositorHandler,CompositorState},
        shm::{ShmState,ShmHandler,with_buffer_contents},
        selection::{SelectionHandler, data_device::{DataDeviceHandler,DataDeviceState,
            ClientDndGrabHandler,ServerDndGrabHandler}},
        output::{OutputHandler,OutputManagerState},
        shell::{xdg::{XdgShellState,XdgShellHandler,ToplevelSurface,PopupSurface,PositionerState,XdgToplevelSurfaceData,XdgPopupSurfaceData},
                wlr_layer::{WlrLayerShellState,WlrLayerShellHandler,
                           LayerSurface as WlrLayerSurface,Layer as WlrLayer,
                           Anchor as WlrAnchor,LayerSurfaceCachedState}},
    },
};
use melania_native::layer_surface_layout::{self,Anchors,Insets,LayerRequest,ShellLayer};
use melania_native::gtk_layer_geometry;
use melania_native::scene::Layer as SceneLayer;
use melania_native::{NativeLifecycle,DeviceChange,native_gpu_watch::NativeGpuWatch,render_scheduler::{FrameTicket,RenderScheduler},continuous_redraw::{ContinuousRedraw,PlannedSceneFrame},continuous_kms_stream::{ContinuousKmsWorker,ContinuousKmsJob},client_commit_ledger::{ClientCommitLedger,MAX_PENDING_CLIENTS},multi_window_scene::MultiWindowScene,dynamic_window_scene::{DynamicWindowScene,WindowPixels},window_slots::WindowSlots,async_kms_stream::{AsyncKmsWorker,FrameJob},window_interaction::{WindowInteraction,WindowRect,InteractionKind,ResizeDirection},managed_window::{ManagedWindow,WindowDisplayState}};
use smithay::backend::input::Device as InputDevice;
use wayland_protocols::xdg::shell::server::xdg_toplevel;
use crate::{native_client_frame,qemu_continuous_receiver};
use smithay::reexports::rustix::fs::OFlags;
use std::os::fd::OwnedFd;
use melania_control::{start_toggle_socket_path,SOCKET_ENV,
    Command as ControlCommand,WindowInfo,ACK,respond_list,respond_workspace,respond_error};
use melania_native::control_io::NativeControlPoller;
use melania_native::native_presentation_guard::{NativePresentationGuard,PresentationToken};
use melania_native::native_frame_budget;
use melania_native::native_popup_layout::PopupPlacement;
use smithay::utils::Rectangle;
use melania_native::keyboard_window_cycle::{KeyboardWindowCycle,ShortcutResult};
use signal_hook::consts::signal::{SIGINT,SIGTERM};

struct NativeControlSocket(PathBuf);
impl Drop for NativeControlSocket {
    fn drop(&mut self){let _=fs::remove_file(&self.0);}
}

#[derive(Default)]
struct BridgeClientState {
    compositor: CompositorClientState,
}
impl ClientData for BridgeClientState {
    fn initialized(&self,_:ClientId) {}
    fn disconnected(&self,_:ClientId,_:DisconnectReason) {}
}

struct BridgeState {
    compositor: CompositorState,
    shm:ShmState,
    // GTK4's GDK Wayland backend requires wl_data_device_manager even
    // before it creates its first layer-shell window. This is a protocol
    // global, NOT proof of native clipboard/DnD integration or login.
    data_device:DataDeviceState,
    output:Output,
    output_manager:OutputManagerState,
    pixel_probe:bool,
    pixel_ready:bool,
    live_stream:bool,
    frame_callbacks_probe:bool,
    frame_callback_count:u32,
    render_scheduler:RenderScheduler,
    continuous_redraw:ContinuousRedraw,
    continuous_pending:Option<PlannedSceneFrame>,
    // Opt-in, disposable QEMU continuous renderer; the legacy proof is intact.
    continuous_kms_mode:bool,
    // Strictly disposable QEMU mode: continue dispatching damage until a
    // real SIGINT/SIGTERM, then drain ACKs and restore the owned CRTC.
    persistent_kms_mode:bool,
    persistent_shutdown_requested:bool,
    continuous_worker:Option<ContinuousKmsWorker>,
    // Keep one pending wl_shm buffer per real client surface, not globally.
    continuous_inflight:Option<(PlannedSceneFrame,Vec<(WlSurface,wl_buffer::WlBuffer)>)>,
    // Stage89: the token is minted at worker enqueue time. A frame ACK
    // after libseat/VT revocation must never release clients or callbacks.
    continuous_presentation_token:Option<PresentationToken>,
    native_presentation_guard:NativePresentationGuard,
    continuous_client_ledger:ClientCommitLedger<WlSurface,wl_buffer::WlBuffer>,
    continuous_frame_count:u64,
    continuous_frame_target:u64,
    continuous_reopen_mode:bool,
    continuous_close_frame_presented:bool,
    continuous_reopen_frame_presented:bool,
    continuous_two_window_presented:bool,
    multi_window_mode:bool,
    async_kms_mode:bool,
    drag_redraw_mode:bool,
    close_redraw_mode:bool,
    close_frame_presented:bool,
    reopen_redraw_mode:bool,
    reopen_frame_presented:bool,
    presented_layout_generation:u64,
    // Store proof of a successful KMS presentation separately from the
    // current scene: a client may destroy its toplevel after the frame ACK.
    drag_frame_presented:bool,
    two_window_frame_presented:bool,
    async_stream:Option<AsyncKmsWorker>,
    async_pending:Option<PendingAsyncFrame>,
    multi_scene:MultiWindowScene,
    // Stage53 is opt-in within disposable QEMU. The original two fixed
    // window scene and its verified KMS experiments remain unchanged.
    dynamic_window_mode:bool,
    dynamic_scene:DynamicWindowScene,
    // Only the KMS-confirmed owned CPU image is eligible for damage-based
    // repaint. Ownership returns from the transport worker after the real
    // CRTC/FB_ID ACK; no duplicate full-frame staging Vec is retained.
    dynamic_confirmed_frame:Option<Vec<u8>>,
    // Explicit, VM-only xdg_toplevel configure/ack resize protocol.
    // Unrelated nested Wayland applications use their existing policy.
    dynamic_xdg_resize_mode:bool,
    // Real QMP tablet drag AFTER xdg resize; needs one compositor-only KMS frame.
    dynamic_pointer_proof_mode:bool,
    dynamic_toplevels:Vec<ToplevelSurface>,
    // Opt-in VM-only bridge of real xdg_toplevel requests to Stage77.
    managed_xdg_mode:bool,
    managed_windows:Vec<(WlSurface,ManagedWindow)>,
    // A configure alone is not a displayed resize: wait for the matching
    // wl_shm client image before changing the retained KMS scene.
    managed_pending_sizes:Vec<(WlSurface,(usize,usize))>,
    // A top/left edge moves only with the matching new client buffer.
    managed_pending_resize:Vec<(WlSurface,WindowRect)>,
    // Configured xdg_popup children; only ACKed client pixels enter KMS.
    // Parent identity is retained to follow real parent drags/destroy.
    native_popup_roles:Vec<(PopupSurface,WlSurface)>,
    window_interaction:WindowInteraction,
    keyboard_window_cycle:KeyboardWindowCycle,
    // One-shot move authorization minted from a real libinput left click,
    // scoped to its Wayland surface and the exact wl_pointer button serial.
    last_xdg_press:Option<(WlSurface,Serial)>,
    known_window_surfaces:WindowSlots<WlSurface>,
    stream:Option<UnixStream>,
    pixel_frame_count:u8,
    pixel_frame_target:u8,
    xdg_shell: XdgShellState,
    layer_shell: WlrLayerShellState,
    // Real wlr-layer wl_shm is opt-in in a disposable QEMU guest only.
    layer_pixels_mode:bool,
    // Five named GTK surfaces are accepted only in the explicit QEMU opt-in.
    gtk_layer_mode:bool,
    // Stage84 is a separate, strictly QEMU-only real GTK -> wl_shm -> DRM proof.
    real_gtk_pixel_kms_mode:bool,
    layer_surfaces:Vec<(String,WlrLayerSurface)>,
    xdg_toplevel_count: u32,
    layer_surface_count: u32,
    seats: SeatState<Self>,
    seat: Seat<Self>,
    focused: Option<WlSurface>,
    pointer_position: Point<f64,Logical>,
    lifecycle: NativeLifecycle,
    // Real libseat/udev seat monitor. Any DRM topology change permanently
    // revokes this QEMU resource generation until a fresh acquisition.
    gpu_watch:NativeGpuWatch,
    delivered_keys: u32,
    // Only the disposable QEMU Stage87 experiment tracks one Super key.
    // Any intervening key press cancels the standalone-press shortcut.
    qemu_start_super_down:Option<u32>,
    delivered_buttons: u32,
    delivered_motions: u32,
    delivered_scroll_frames: u32,
}

fn xdg_resize_direction(edge:xdg_toplevel::ResizeEdge)->Option<ResizeDirection>{
    use xdg_toplevel::ResizeEdge as Edge;
    Some(match edge {
        Edge::Top=>ResizeDirection::Top,
        Edge::Bottom=>ResizeDirection::Bottom,
        Edge::Left=>ResizeDirection::Left,
        Edge::Right=>ResizeDirection::Right,
        Edge::TopLeft=>ResizeDirection::TopLeft,
        Edge::TopRight=>ResizeDirection::TopRight,
        Edge::BottomLeft=>ResizeDirection::BottomLeft,
        Edge::BottomRight=>ResizeDirection::BottomRight,
        _=>return None,
    })
}

// A client-provided serial alone is never permission to move another window.
// Consume even an incorrect/replayed token; no request may reuse a click.
fn take_xdg_pointer_press<T:PartialEq>(
    token:&mut Option<(T,Serial)>, surface:&T, serial:Serial,
)->bool {
    token.take().is_some_and(|(pressed,issued)|&pressed==surface && issued==serial)
}

struct PendingAsyncFrame {
    client:Option<(WlSurface,wl_buffer::WlBuffer)>,
    layout_generation:u64,
    ticket:FrameTicket,
    ordinal:u8,
}

/// Send wl_surface.frame completions only after the KMS presenter has
/// acknowledged that client-owned pixels actually reached the VM CRTC.
/// Traverse subsurfaces so a client does not stall an animated surface tree.
// A toplevel role request cannot present old pixels at a new geometry.
// The actual imported wl_shm buffer must satisfy the last configure size.
fn accepts_managed_role_buffer(expected:Option<(usize,usize)>,actual:(usize,usize))->bool {
    expected.is_none_or(|size|size==actual)
}

fn complete_presented_frame(surface:&WlSurface,elapsed_ms:u32)->u32 {
    let mut delivered=0u32;
    with_surface_tree_downward(
        surface,(),
        |_,_,&()|TraversalAction::DoChildren(()),
        |_,attributes,&()|{
            for callback in attributes.cached_state.get::<SurfaceAttributes>()
                .current().frame_callbacks.drain(..) {
                callback.done(elapsed_ms);
                delivered=delivered.saturating_add(1);
            }
        },
        |_,_,&()|true,
    );
    delivered
}

impl BridgeState {
    // Stage43: observe the *actual* scene changes in the QEMU Wayland
    // event loop. The stable ordinal-based KMS presenter remains unchanged;
    // this second, generation-based ledger audits its real ACKs before any
    // wl_surface.frame callback or buffer.release is emitted.
    fn note_scene_change(&mut self) {
        if self.async_kms_mode || self.continuous_kms_mode {
            let generation=if self.dynamic_window_mode {
                self.dynamic_scene.generation()
            } else {
                self.multi_scene.layout_generation()
            };
            self.continuous_redraw.scene_changed(generation);
        }
    }

    // Stage88: a real keyboard focus may only follow a committed GTK4 Start
    // wl_shm buffer, not a mere layer-shell configure or a QMP host marker.
    // This remains opt-in for the disposable native QEMU compositor.
    fn refresh_qemu_gtk_start_focus(&mut self,surface:&WlSurface,on_screen:bool) {
        if !self.real_gtk_pixel_kms_mode ||
           std::env::var("MELANIA_NATIVE_GTK_START_FOCUS")
               .as_deref()!=Ok("YES_QEMU_VM") {
            return;
        }
        if on_screen {
            if self.focused.as_ref()==Some(surface) {return;}
            let keyboard=self.seat.get_keyboard().expect("keyboard");
            keyboard.set_focus(self,Some(surface.clone()),SERIAL_COUNTER.next_serial());
            self.focused=Some(surface.clone());
            println!("MELANIA_NATIVE_GTK_START_REAL_KEYBOARD_FOCUS=true");
        } else if self.focused.as_ref()==Some(surface) {
            let keyboard=self.seat.get_keyboard().expect("keyboard");
            keyboard.set_focus(self,None,SERIAL_COUNTER.next_serial());
            self.focused=None;
            println!("MELANIA_NATIVE_GTK_START_KEYBOARD_FOCUS_RELEASED=true");
        }
        let _=std::io::stdout().flush();
    }

    // Stage46: in this isolated mode ContinuousRedraw is authoritative.
    // All DRM socket writes and ACK reads happen off the Smithay thread.
    fn submit_continuous_frame(&mut self)->Result<(),Box<dyn Error>> {
        if !self.continuous_kms_mode || self.continuous_inflight.is_some()
            || self.persistent_shutdown_requested {
            return Ok(());
        }
        // A fresh frame must originate from the currently active libseat
        // generation. An automatic seat resume never re-arms old GPU state.
        let presentation_token=self.native_presentation_guard
            .ticket(self.lifecycle.seat_active())
            .ok_or("native KMS frame blocked: libseat presentation revoked")?;
        // Stage86 is a bounded *three-frame* VM proof. GTK may repaint the
        // taskbar more than once after frame two, before its Start layer is
        // mapped. Do not consume the third KMS slot with another taskbar-only
        // frame: wait until Smithay actually owns the Start wl_shm surface.
        // Once the third CRTC ACK has arrived, never submit a fourth frame
        // into the deliberately terminating DRM receiver.
        if self.real_gtk_pixel_kms_mode {
            let start_proof=std::env::var("MELANIA_NATIVE_REAL_GTK_START_KMS")
                .as_deref()==Ok("YES_QEMU_VM");
            if self.continuous_frame_count>=self.continuous_frame_target ||
                (start_proof && self.continuous_frame_count>=2 &&
                 !self.layer_surfaces.iter().any(|(name,role)|{
                     name=="melania-start" &&
                     self.known_window_surfaces.get(2)==Some(role.wl_surface())
                 })) {
                return Ok(());
            }
        }
        let Some(planned)=self.continuous_redraw.next_frame() else {return Ok(());};
        let generation=if self.dynamic_window_mode {
            self.dynamic_scene.generation()
        } else {
            self.multi_scene.layout_generation()
        };
        if planned.generation!=generation {
            self.continuous_redraw.failed(planned);
            return Err("continuous KMS scene changed during planning".into());
        }
        let composed=if self.dynamic_window_mode {
            if let Some(mut previous)=self.dynamic_confirmed_frame.take() {
                // The presenter owns the old baseline exclusively until
                // the verified ACK. A failed flip aborts safely; a newer
                // client commit cannot accidentally reuse unconfirmed pixels.
                self.dynamic_scene.composite_damaged_into(&mut previous)
                    .map(|_|previous).map_err(|_|"dynamic partial composition failed")
            } else {
                self.dynamic_scene.composite().map_err(|_|"dynamic scene failed")
            }
        } else {
            self.multi_scene.composite().map_err(|_|"fixed scene failed")
        };
        let pixels=match composed {
            Ok(pixels)=>pixels,
            Err(_)=>{
                self.continuous_redraw.failed(planned);
                return Err("continuous native scene composition failed".into());
            }
        };
        let Some(worker)=self.continuous_worker.as_ref() else {
            self.continuous_redraw.failed(planned);
            return Err("continuous QEMU KMS transport unavailable".into());
        };
        // Move, never clone, the complete scene into the bounded transport.
        // The exact allocation returns with its presentation receipt.
        if worker.submit(ContinuousKmsJob{planned,pixels}).is_err() {
            self.continuous_redraw.failed(planned);
            return Err("continuous QEMU KMS worker rejected frame".into());
        }
        self.continuous_presentation_token=Some(presentation_token);
        self.continuous_inflight=Some((
            planned,self.continuous_client_ledger.take_for_presentation()
        ));
        println!("MELANIA_NATIVE_CONTINUOUS_FRAME_SUBMITTED={} generation={}",
                 planned.ticket.sequence,planned.generation);
        let _=std::io::stdout().flush();
        Ok(())
    }

    fn poll_continuous_kms(&mut self)->Result<(),Box<dyn Error>> {
        if !self.continuous_kms_mode {return Ok(());}
        if self.native_presentation_guard
            .ticket(self.lifecycle.seat_active()).is_none() {
            return Err("native KMS ACK blocked after libseat/VT revocation".into());
        }
        let Some(worker)=self.continuous_worker.as_ref() else {return Ok(());};
        let reply=match worker.try_result() {
            Ok(Some(reply))=>reply,
            Ok(None)=>return Ok(()),
            Err(error)=>return Err(format!("continuous KMS worker disconnected: {error}").into()),
        };
        let (planned,clients)=self.continuous_inflight.take()
            .ok_or("continuous KMS reply with no pending frame")?;
        let ticket=self.continuous_presentation_token.take()
            .ok_or("continuous KMS reply without a seat presentation token")?;
        if !self.native_presentation_guard.accepts(ticket,self.lifecycle.seat_active()) {
             self.continuous_redraw.failed(planned);
            return Err("stale native KMS ACK from revoked libseat generation".into());
        }
        if reply.planned!=planned {
             self.continuous_redraw.failed(planned);
            return Err("continuous QEMU KMS ACK ticket mismatch".into());
        }
        if let Err(error)=reply.status {
             self.continuous_redraw.failed(planned);
            return Err(format!("continuous KMS presentation rejected: {error}").into());
        }
        // The DRM receiver verifies the actual CRTC FB_ID before MLA1.
        if !self.continuous_redraw.presented(planned) {
             self.dynamic_confirmed_frame=None;
            return Err("continuous KMS stale ACK after seat/output transition".into());
        }
        if self.dynamic_window_mode {
            // The worker returned the exact submitted Vec. Only this
            // generation's successful real DRM receipt authorizes it
            // as the next damage-based scene baseline.
            self.dynamic_confirmed_frame=Some(reply.pixels);
        }
        // Stage59: a damage snapshot is retired only after the real KMS
        // CRTC FB_ID receiver verified this exact frame. A newer client
        // commit or pointer motion keeps its regions dirty for the successor.
        if self.dynamic_window_mode {
            self.dynamic_scene.confirm_presented_damage(planned.generation);
        }
        self.continuous_frame_count=self.continuous_frame_count.saturating_add(1);
        self.presented_layout_generation=planned.generation;
        // Stage99: after a VERIFIED CRTC/connector ACK only, expose a
        // guest-local synchronization marker for the real Wayland client.
        // Unlike a protocol-only role ACK, frame 13/14 must have reached
        // the VM's physical display before the next IPC action is allowed.
        if std::env::var("MELANIA_NATIVE_MANAGED_MINIMIZE_QMP")
            .as_deref()==Ok("YES_QEMU_VM") &&
           matches!(self.continuous_frame_count,13|14) {
            let visible=self.continuous_frame_count==14;
            let first=self.known_window_surfaces.get(1)
                .ok_or("managed minimize lost the first real Wayland surface")?;
            let model=self.managed_windows.iter()
                .find(|(surface,_)|surface==first)
                .ok_or("managed minimize lost the first xdg role")?;
            if self.dynamic_scene.window_count()!=2 ||
               self.dynamic_scene.is_visible(1)!=Some(visible) ||
               self.dynamic_scene.is_visible(2)!=Some(true) ||
               (model.1.state()==WindowDisplayState::Minimized)==visible {
                return Err("managed minimize CRTC ACK has inconsistent retained window state".into());
            }
            let marker=format!("/run/melania-native-qemu/managed-minimize-frame{}-acked",
                self.continuous_frame_count);
            std::fs::write(marker,b"verified CRTC route and framebuffer ACK\n")?;
            println!("MELANIA_NATIVE_MANAGED_MINIMIZE_KMS_ACK={} visible={}",
                     self.continuous_frame_count,visible);
            let _=std::io::stdout().flush();
        }
        if self.dynamic_pointer_proof_mode && self.continuous_frame_count==12 {
            // Unique *post-ACK* signal: prior scenarios in this same QEMU
            // guest also present frame 12, so the host must not use a
            // generic ordinal to inject input into the wrong test.
            println!("MELANIA_NATIVE_DYNAMIC_POINTER_READY_AFTER_KMS12=true");
            std::io::stdout().flush()?;
        }
        if self.dynamic_pointer_proof_mode && self.continuous_frame_count==13 {
            let (x,y,width,height)=self.dynamic_scene.geometry(2)
                .ok_or("QMP pointer drag lost resized client B")?;
            if (x-740).abs()>3 || (y-270).abs()>3 ||
               width!=220 || height!=180 || self.dynamic_scene.window_count()!=2 {
                return Err("QMP pointer drag did not preserve resized B geometry".into());
            }
            // Signal only after the real CRTC FB_ID success ACK. Keep the
            // client alive until this proof, otherwise disconnect clears B.
            std::fs::write("/run/melania-native-qemu/dynamic-pointer-frame13-acked",b"real KMS ACK\n")?;
            println!("MELANIA_NATIVE_DYNAMIC_POINTER_KMS13_ACKED=true");
            let _=std::io::stdout().flush();
        }
        if self.dynamic_xdg_resize_mode && self.continuous_frame_count==6 {
            // Before the next client commits, issue REAL xdg_toplevel
            // configure events. Clients must ack xdg_surface.configure
            // before attaching the corresponding smaller wl_shm images.
            if self.dynamic_toplevels.len()!=2 {
                return Err("xdg resize: expected two configured toplevel roles".into());
            }
            for (index,top) in self.dynamic_toplevels.iter().enumerate() {
                let (width,height)=if index==0 {(320,192)} else {(220,180)};
                top.with_pending_state(|state| {
                    state.size=Some((width,height).into());
                    state.states.set(xdg_toplevel::State::Resizing);
                });
                top.send_pending_configure();
            }
            println!("MELANIA_NATIVE_DYNAMIC_XDG_RESIZE_CONFIGURES_SENT=true");
            let _=std::io::stdout().flush();
        }
        // Keep the proof at the verified FB_ID ACK, since the clients may
        // destroy their xdg_toplevels after receiving frame.done.
        let two_visible=if self.dynamic_window_mode {
            self.dynamic_scene.window_count()==2
        } else {
            self.multi_scene.window_count()==2
        };
        if self.multi_window_mode && two_visible &&
           self.known_window_surfaces.live_count()==2 {
            if self.continuous_frame_count==12 {
                self.continuous_two_window_presented=true;
            }
            if self.continuous_reopen_mode && self.continuous_frame_count==14 {
                self.continuous_reopen_frame_presented=true;
            }
        }
        if self.continuous_reopen_mode && self.continuous_frame_count==13 {
            if self.multi_scene.window_count()!=1 ||
               self.known_window_surfaces.get(1).is_some() {
                return Err("continuous close KMS ACK did not remove slot one".into());
            }
            // The surviving B client explicitly requested a frame callback
            // before A was destroyed. Never complete it before KMS FB_ID ACK.
            let survivor=self.known_window_surfaces.get(2).cloned()
                .ok_or("continuous close removed the surviving client")?;
            let delivered=complete_presented_frame(&survivor,13*16);
            if delivered!=1 {
                return Err("continuous close did not complete survivor callback".into());
            }
            self.frame_callback_count=self.frame_callback_count.saturating_add(delivered);
            self.continuous_close_frame_presented=true;
            println!("MELANIA_NATIVE_CONTINUOUS_CLOSE_FRAME13_CALLBACK=true");
        }
        // Each surface is independently eligible for a callback. Newer
        // commits of A must not starve an already presented B.
        for completed in self.continuous_client_ledger.acknowledged(clients) {
            let is_live=(1..=MAX_PENDING_CLIENTS as u64)
                .any(|slot|self.known_window_surfaces.get(slot)==Some(&completed.key));
            if completed.notify_frame && is_live {
                let delivered=complete_presented_frame(
                    &completed.key,(self.continuous_frame_count as u32).saturating_mul(16)
                );
                self.frame_callback_count=self.frame_callback_count.saturating_add(delivered);
            }
            completed.resource.release();
        }
        println!("MELANIA_NATIVE_CONTINUOUS_KMS_ACK={} generation={}",
                 self.continuous_frame_count,planned.generation);
        let _=std::io::stdout().flush();
        // If more damage arrived while KMS was in flight, render the latest.
        self.submit_continuous_frame()?;
        Ok(())
    }

    fn begin_generation_proof(&mut self,expected:u64)->Result<(),Box<dyn Error>> {
        if !self.async_kms_mode { return Ok(()); }
        if self.continuous_pending.is_some() {
            return Err("continuous redraw proof already has an outstanding frame".into());
        }
        let candidate=self.continuous_redraw.next_frame()
            .ok_or("continuous redraw proof has no eligible dirty scene")?;
        if candidate.generation!=expected {
            self.continuous_redraw.failed(candidate);
            return Err("continuous redraw proof disagrees with composed scene generation".into());
        }
        self.continuous_pending=Some(candidate);
        Ok(())
    }

    fn reject_generation_proof(&mut self) {
        if let Some(candidate)=self.continuous_pending.take() {
            self.continuous_redraw.failed(candidate);
        }
    }

    fn confirm_generation_proof(&mut self,actual:u64)->Result<(),Box<dyn Error>> {
        if !self.async_kms_mode { return Ok(()); }
        let candidate=self.continuous_pending.take()
            .ok_or("KMS ACK without a planned continuous scene generation")?;
        if candidate.generation!=actual ||
           !self.continuous_redraw.presented(candidate) {
            return Err("KMS ACK does not match the outstanding scene generation".into());
        }
        println!("MELANIA_NATIVE_SCENE_GENERATION_ACK={}",candidate.generation);
        Ok(())
    }

    /// Resolve QEMU physical output coordinates to a retained client surface.
    fn pointer_target(&self,point:Point<f64,Logical>)
        ->Option<(WlSurface,Point<f64,Logical>)>
    {
        if !self.multi_window_mode {
            return self.focused.clone().map(|s|(s,(0.0,0.0).into()));
        }
        let key=if self.dynamic_window_mode {
            self.dynamic_scene.hit_test(point.x,point.y)?
        } else {
            self.multi_scene.hit_test(point.x,point.y)?
        };
        let surface=self.known_window_surfaces.get(key)?.clone();
        let (x,y)=if self.dynamic_window_mode {
            self.dynamic_scene.position(key)?
        } else {
            self.multi_scene.position(key)?
        };
        Some((surface,(f64::from(x),f64::from(y)).into()))
    }

    /// Changes to model geometry are bounded and require an active real
    /// pointer grab. A compositor-only change is not proof of repaint; the
    /// current KMS stream still presents only on subsequent client commits.
    fn apply_window_motion(&mut self,point:Point<f64,Logical>) {
        if !self.multi_window_mode {return;}
        let Some((key,rect))=self.window_interaction.motion(point.x,point.y) else {
            return;
        };
        match self.window_interaction.active_kind() {
            Some(InteractionKind::Move)=>{
                let moved=if self.dynamic_window_mode {
                    // Avoid issuing redundant GPU damage for a pointer event
                    // that didn't change the client's screen position.
                    self.dynamic_scene.position(key)!=Some((rect.x,rect.y)) &&
                        self.dynamic_scene.move_to(key,rect.x,rect.y).is_ok()
                } else {
                    self.multi_scene.move_to(key,rect.x,rect.y).is_ok()
                };
                if moved {
                    if self.managed_xdg_mode && self.dynamic_window_mode {
                        // Save the real pointer-drag placement for restore.
                        if let Some(surface)=self.known_window_surfaces.get(key) {
                            if let Some((_,model))=self.managed_windows.iter_mut()
                                .find(|(role,_)|role==surface) {
                                model.update_normal_geometry(rect);
                            }
                        }
                    }
                    if self.managed_xdg_mode && self.dynamic_window_mode {
                        self.reposition_native_popups_for_parent(key);
                    }
                    self.note_scene_change();
                    println!("MELANIA_NATIVE_WINDOW_DRAG_MODEL={} x={} y={}",
                             key,rect.x,rect.y);
                    if self.dynamic_window_mode {
                        // The dynamic scene is presented through the existing
                        // asynchronous real-ACK KMS path, never on mouse
                        // motion without a verified framebuffer result.
                        if (self.persistent_kms_mode || self.continuous_frame_count<self.continuous_frame_target) {
                            if let Err(error)=self.submit_continuous_frame() {
                                eprintln!("dynamic QEMU drag KMS rejected: {error}");
                            }
                        }
                    } else if let Err(error)=self.submit_drag_frame() {
                        eprintln!("QEMU drag KMS frame rejected: {error}");
                    }
                }
            },
            Some(InteractionKind::ResizeBottomRight | InteractionKind::Resize(_))=>{
                if !self.managed_xdg_mode || !self.dynamic_window_mode {
                    println!("MELANIA_NATIVE_WINDOW_RESIZE_PREVIEW={} w={} h={}",
                             key,rect.width,rect.height);
                    return;
                }
                let Some(surface)=self.known_window_surfaces.get(key).cloned() else{return;};
                let Some(top)=self.xdg_shell.toplevel_surfaces().iter()
                    .find(|top|top.wl_surface()==&surface).cloned() else{return;};
                if self.managed_pending_resize.iter().any(|(role,pending)|
                    role==&surface && *pending==rect) {return;}
                self.managed_pending_resize.retain(|(role,_)|role!=&surface);
                self.managed_pending_resize.push((surface.clone(),rect));
                self.managed_pending_sizes.retain(|(role,_)|role!=&surface);
                self.managed_pending_sizes.push((
                    surface.clone(),(rect.width as usize,rect.height as usize)
                ));
                top.with_pending_state(|pending|{
                    pending.states.set(xdg_toplevel::State::Resizing);
                    pending.size=Some((rect.width,rect.height).into());
                });
                top.send_pending_configure();
                println!("MELANIA_NATIVE_XDG_RESIZE_CONFIGURE={} x={} y={} w={} h={}",
                         key,rect.x,rect.y,rect.width,rect.height);
            },
            None=>{},
        }
    }

    /// A real libinput pointer movement can repaint the compositor's
    /// retained scene without requiring an extra wl_surface.commit.
    /// Stage38 is intentionally limited to ONE additional frame (ordinal 4).
    fn submit_drag_frame(&mut self) -> Result<(),Box<dyn Error>> {
        if !self.drag_redraw_mode || self.pixel_frame_count!=3 ||
           self.async_pending.is_some() ||
           self.multi_scene.layout_generation()==self.presented_layout_generation {
            return Ok(());
        }
        let frame=self.multi_scene.composite()
            .map_err(|_|"QEMU drag scene compose failed")?;
        if !self.render_scheduler.damage() {
            return Err("QEMU drag damage rejected by suspended session".into());
        }
        let ticket=self.render_scheduler.begin_frame()
            .ok_or("QEMU drag presentation already in progress")?;
        if let Err(error)=self.begin_generation_proof(self.multi_scene.layout_generation()) {
            self.render_scheduler.fail_present(ticket);
            return Err(error);
        }
        let worker=self.async_stream.as_ref()
            .ok_or("QEMU drag KMS worker unavailable")?;
        if worker.submit(FrameJob{ticket,ordinal:4,pixels:frame}).is_err() {
            self.reject_generation_proof();
            self.render_scheduler.fail_present(ticket);
            return Err("QEMU drag KMS queue rejected fourth frame".into());
        }
        self.async_pending=Some(PendingAsyncFrame{
            client:None,ticket,ordinal:4,
            layout_generation:self.multi_scene.layout_generation(),
        });
        println!("MELANIA_NATIVE_DRAG_FRAME_SUBMITTED=4");
        std::io::stdout().flush()?;
        Ok(())
    }

    /// QEMU-only fifth presentation: remove the destroyed client's old
    /// pixels from the *actual* KMS framebuffer while its peer stays alive.
    /// The fourth-frame ticket must have been acknowledged first.
    fn submit_close_frame(&mut self) -> Result<(),Box<dyn Error>> {
        if !self.close_redraw_mode || self.pixel_frame_count!=4 ||
           self.async_pending.is_some() ||
           self.multi_scene.window_count()!=1 ||
           !self.render_scheduler.needs_redraw() {
            return Err("QEMU close frame not ready: missing live survivor or damage".into());
        }
        let frame=self.multi_scene.composite()
            .map_err(|_|"QEMU close scene compose failed")?;
        let ticket=self.render_scheduler.begin_frame()
            .ok_or("QEMU close frame could not acquire next KMS ticket")?;
        if let Err(error)=self.begin_generation_proof(self.multi_scene.layout_generation()) {
            self.render_scheduler.fail_present(ticket);
            return Err(error);
        }
        let worker=self.async_stream.as_ref()
            .ok_or("QEMU close frame KMS worker unavailable")?;
        if worker.submit(FrameJob{ticket,ordinal:5,pixels:frame}).is_err() {
            self.reject_generation_proof();
            self.render_scheduler.fail_present(ticket);
            return Err("QEMU close frame was not accepted by KMS worker".into());
        }
        self.async_pending=Some(PendingAsyncFrame{
            client:None,ticket,ordinal:5,
            layout_generation:self.multi_scene.layout_generation(),
        });
        println!("MELANIA_NATIVE_CLOSE_FRAME_SUBMITTED=5");
        std::io::stdout().flush()?;
        Ok(())
    }

    /// Submit exactly one sixth-frame update from the NEW real wl_shm buffer.
    /// The old first wl_surface must already be destroyed and slot 1 freed.
    /// This is deliberately opt-in for the disposable QEMU guest only.
    fn submit_reopen_frame(&mut self,surface:&WlSurface,buffer:wl_buffer::WlBuffer)
        ->Result<(),Box<dyn Error>>
    {
        if !self.reopen_redraw_mode || self.pixel_frame_count!=5 ||
           !self.close_frame_presented || self.async_pending.is_some() ||
           self.known_window_surfaces.get(1).is_some() ||
           self.multi_scene.window_count()!=1 {
            return Err("QEMU sixth frame requires one closed and one live window".into());
        }
        let imported=with_buffer_contents(&buffer,|ptr,len,metadata|{
            native_client_frame::copy_client_shm(ptr,len,metadata)
        });
        let pixels=match imported {
            Ok(Ok(pixels))=>pixels,
            _=>return Err("QEMU reopened Wayland client did not provide valid wl_shm pixels".into()),
        };
        let key=self.known_window_surfaces.register(surface.clone())
            .map_err(|_|"QEMU reopening exceeds stable slot limit")?;
        if key!=1 { return Err("QEMU reopened surface failed to reuse the freed slot".into()); }
        self.multi_scene.upsert(key,pixels)
            .map_err(|_|"QEMU reopened window invalid scene image")?;
        self.note_scene_change();
        let frame=self.multi_scene.composite()
            .map_err(|_|"QEMU reopened window scene cannot be composed")?;
        if !self.render_scheduler.damage() {
            return Err("QEMU reopen damage rejected by seat scheduler".into());
        }
        let ticket=self.render_scheduler.begin_frame()
            .ok_or("QEMU reopen frame could not acquire KMS ticket")?;
        if let Err(error)=self.begin_generation_proof(self.multi_scene.layout_generation()) {
            self.render_scheduler.fail_present(ticket);
            return Err(error);
        }
        let worker=self.async_stream.as_ref()
            .ok_or("QEMU reopen frame KMS worker unavailable")?;
        if worker.submit(FrameJob{ticket,ordinal:6,pixels:frame}).is_err() {
            self.reject_generation_proof();
            self.render_scheduler.fail_present(ticket);
            return Err("QEMU reopening sixth frame rejected by KMS worker".into());
        }
        self.async_pending=Some(PendingAsyncFrame{
            client:Some((surface.clone(),buffer)),
            ticket,ordinal:6,
            layout_generation:self.multi_scene.layout_generation(),
        });
        println!("MELANIA_NATIVE_REOPEN_WINDOW_SLOT={key}");
        println!("MELANIA_NATIVE_CLIENT_REOPENED_XDG_COMMITTED=true");
        println!("MELANIA_NATIVE_REOPEN_FRAME_SUBMITTED=6");
        std::io::stdout().flush()?;
        Ok(())
    }

    /// Poll the worker without blocking the Smithay/calloop dispatcher.
    /// No callback, buffer.release(), or frame counter is issued before the
    /// worker has read the genuine KMS ACK from the VM Rust DRM process.
    fn poll_async_kms(&mut self) -> Result<(),Box<dyn Error>> {
        if !self.async_kms_mode {return Ok(());}
        let worker=self.async_stream.as_ref().ok_or("async KMS worker not connected")?;
        let reply=match worker.try_result() {
            Ok(Some(result))=>result,
            Ok(None)=>return Ok(()),
            Err(error)=>return Err(format!("async KMS worker disconnected: {error}").into()),
        };
        let pending=self.async_pending.take()
            .ok_or("KMS worker returned an ACK without a pending Wayland frame")?;
        if pending.ticket!=reply.ticket || pending.ordinal!=reply.ordinal {
            return Err("async KMS ticket or frame ordinal mismatch".into());
        }
        if let Err(error)=reply.status {
            self.reject_generation_proof();
            self.render_scheduler.fail_present(pending.ticket);
            return Err(format!("async Rust KMS presentation failed: {error}").into());
        }
        if !self.render_scheduler.complete_present(pending.ticket) {
            self.reject_generation_proof();
            return Err("async KMS ACK invalidated by seat pause or GPU change".into());
        }
        // The native scene generation is checked only after the verified
        // CRTC FB_ID ACK and before clients are notified of presentation.
        self.confirm_generation_proof(pending.layout_generation)?;
        self.pixel_frame_count=pending.ordinal;
        self.pixel_ready=self.pixel_frame_count==self.pixel_frame_target;
        self.presented_layout_generation=pending.layout_generation;
        if self.multi_window_mode && pending.ordinal==3 &&
           self.multi_scene.window_count()==2 {
            self.two_window_frame_presented=true;
        }
        if self.drag_redraw_mode && pending.ordinal==4 {
            self.drag_frame_presented=true;
        }
        if self.close_redraw_mode && pending.ordinal==5 {
            self.close_frame_presented=true;
        }
        if self.reopen_redraw_mode && pending.ordinal==6 &&
           self.multi_scene.window_count()==2 &&
           self.known_window_surfaces.get(1).is_some() &&
           self.known_window_surfaces.get(2).is_some() {
            // Capture the state at the actual KMS ACK. The test client is
            // allowed to destroy its window after receiving frame.done.
            self.reopen_frame_presented=true;
        }
        println!("MELANIA_NATIVE_LIVE_KMS_FRAME_ACK={}",pending.ordinal);
        println!("MELANIA_NATIVE_SCHEDULED_FRAME_ACK={}",pending.ordinal);
        println!("MELANIA_NATIVE_ASYNC_FRAME_COMPLETED={}",pending.ordinal);
        if let Some((surface,buffer))=pending.client {
            println!("MELANIA_NATIVE_CLIENT_SHM_IMPORTED=true");
            println!("MELANIA_NATIVE_CLIENT_FRAME_INDEX={}",pending.ordinal);
            println!("MELANIA_NATIVE_CLIENT_FRAME_COMPOSED=true");
            if self.frame_callbacks_probe {
                let delivered=complete_presented_frame(
                    &surface,pending.ordinal as u32*16
                );
                if self.reopen_redraw_mode && pending.ordinal==6 && delivered!=1 {
                    return Err("reopened client did not request a real sixth-frame Wayland callback".into());
                }
                self.frame_callback_count=self.frame_callback_count.saturating_add(delivered);
                if self.reopen_redraw_mode && pending.ordinal==6 {
                    println!("MELANIA_NATIVE_REOPEN_FRAME_CALLBACK_DONE=true");
                }
                println!("MELANIA_NATIVE_FRAME_CALLBACK_DONE={}",pending.ordinal);
                println!("MELANIA_NATIVE_FRAME_CALLBACK_COUNT={}",self.frame_callback_count);
            }
            buffer.release();
            if self.reopen_redraw_mode && pending.ordinal==6 {
                println!("MELANIA_NATIVE_REOPEN_FRAME_ACKED=6");
            }
        } else {
            match pending.ordinal {
                4 if self.drag_redraw_mode=>
                    println!("MELANIA_NATIVE_DRAG_FRAME_ACKED=4"),
                5 if self.close_redraw_mode=>{
                    // A surviving real wl_surface requested this callback
                    // before A was destroyed. Only the fifth DRM FB_ID ACK
                    // may complete it, not display.sync or an idle timer.
                    let survivor=self.known_window_surfaces.get(2).cloned()
                        .ok_or("QEMU surviving Wayland window unavailable after KMS ACK")?;
                    let delivered=complete_presented_frame(&survivor,5*16);
                    if delivered!=1 {
                        return Err("QEMU close KMS presentation did not complete survivor callback".into());
                    }
                    self.frame_callback_count=self.frame_callback_count.saturating_add(delivered);
                    println!("MELANIA_NATIVE_FRAME_CALLBACK_COUNT={}",self.frame_callback_count);
                    println!("MELANIA_NATIVE_CLOSE_FRAME_CALLBACK_DONE=true");
                    println!("MELANIA_NATIVE_CLOSE_FRAME_ACKED=5");
                },
                _=>return Err("unexpected compositor-only KMS frame".into()),
            }
        }
        if self.close_redraw_mode && pending.ordinal==4 {
            self.submit_close_frame()?;
        }
        std::io::stdout().flush()?;
        Ok(())
    }
}

// Smithay 0.7 exposes the ACKed-at-commit popup geometry through
// XdgPopupSurfaceData.current/current_serial, not with_committed_state()
// (which only exists on newer Smithay APIs). Never use server_pending:
// a sent configure is not proof of a client ACKed buffer.
fn acked_native_popup_geometry(surface:&WlSurface)->Option<Rectangle<i32,Logical>> {
    with_states(surface,|states|{
        let role=states.data_map.get::<XdgPopupSurfaceData>()?.lock().ok()?;
        role.current_serial.map(|_|role.current.geometry)
    })
}

impl BridgeState {
    fn evict_native_popup(&mut self,surface:&WlSurface) {
        // Close nested popups before the parent: otherwise children keep
        // retained KMS pixels and stale scene slots after ancestor teardown.
        let descendants=self.native_popup_roles.iter()
            .filter(|(_,parent)|parent==surface)
            .map(|(popup,_)|popup.clone()).collect::<Vec<_>>();
        for child in descendants {
            child.send_popup_done();
            self.evict_native_popup(child.wl_surface());
        }
        self.native_popup_roles.retain(|(popup,_)|popup.wl_surface()!=surface);
        if let Some(key)=self.known_window_surfaces.remove(surface) {
            if self.dynamic_scene.remove(key) {
                self.note_scene_change();
                if self.continuous_kms_mode &&
                   (self.persistent_kms_mode ||
                    self.continuous_frame_count<self.continuous_frame_target) {
                    if let Err(error)=self.submit_continuous_frame() {
                        eprintln!("native popup removal KMS damage rejected: {error}");
                    }
                }
            }
            self.window_interaction.cancel_for_key(key);
        }
    }
    fn reposition_native_popups_for_parent(&mut self,parent_key:u64) {
        // Traverse the bounded parent/popup tree in depth-first order.
        // Moving only immediate children would strand nested submenus.
        let mut pending=vec![parent_key];
        let mut visited=0usize;
        while let Some(key)=pending.pop() {
            visited+=1;
            if visited>9 {
                eprintln!("native popup hierarchy exceeded bounded depth");
                break;
            }
            let Some((px,py,_,_))=self.dynamic_scene.geometry(key) else {continue;};
            let moves=self.native_popup_roles.iter().filter_map(|(popup,parent)|{
                if self.known_window_surfaces.id_of(parent)!=Some(key) {return None;}
                let child_key=self.known_window_surfaces.id_of(popup.wl_surface())?;
                let g=acked_native_popup_geometry(popup.wl_surface())?;
                let placement=PopupPlacement::new(key,g.loc.x,g.loc.y,g.size.w,g.size.h)?;
                let (x,y)=placement.absolute(px,py,1280,800)?;
                Some((child_key,x,y))
            }).collect::<Vec<_>>();
            for (child_key,x,y) in moves {
                if self.dynamic_scene.position(child_key)!=Some((x,y)) {
                    let _=self.dynamic_scene.move_to(child_key,x,y);
                }
                pending.push(child_key);
            }
        }
    }

    /// A state change must revoke the grab of its own surface, not one
    /// belonging to a different Wayland client.
    fn cancel_surface_grab(&mut self,surface:&WlSurface) {
        if let Some(key)=self.known_window_surfaces.id_of(surface) {
            self.window_interaction.cancel_for_key(key);
        }
    }

    /// Apply actual xdg_toplevel requests to the QEMU compositor state and
    /// configure the client. A new client commit is still required for the
    /// maximized-sized wl_shm content to become visible at its full size.
    fn request_managed_maximize(&mut self,top:ToplevelSurface,enabled:bool) {
        if !self.managed_xdg_mode {
            top.send_configure();
            return;
        }
        let surface=top.wl_surface().clone();
        let Some((_,window))=self.managed_windows.iter_mut()
            .find(|(role,_)|role==&surface) else {
            top.send_configure();
            return;
        };
        let changed=if enabled {window.maximize()} else {
            // Do not reset unrelated minimized/fullscreen states on an
            // unmaximize request. They need their own role transition.
            if window.state()==WindowDisplayState::Maximized {
                window.restore()
            } else {false}
        };
        let rectangle=window.geometry();
        if changed { self.cancel_surface_grab(&surface); }
        top.with_pending_state(|pending| {
            pending.states.unset(xdg_toplevel::State::Resizing);
            if enabled {
                pending.states.set(xdg_toplevel::State::Maximized);
                pending.states.unset(xdg_toplevel::State::Fullscreen);
            } else {pending.states.unset(xdg_toplevel::State::Maximized);}
            if let Some(rect)=rectangle {
                pending.size=Some((rect.width,rect.height).into());
            }
        });
        top.send_pending_configure();
        if changed {
            if let Some(rect)=rectangle {
                // The configure is only a request. A previous wl_shm image
                // must NOT be moved or submitted to KMS before the matching
                // client buffer has been committed and validated.
                self.managed_pending_resize.retain(|(role,_)|role!=&surface);
                self.managed_pending_sizes.retain(|(role,_)|role!=&surface);
                self.managed_pending_sizes.push((
                    surface.clone(),(rect.width as usize,rect.height as usize)
                ));
            }
        }
        println!("MELANIA_NATIVE_MANAGED_XDG_MAXIMIZED={enabled}");
    }

    /// The disposable QEMU output is a single 1280x800 display. The optional
    /// xdg fullscreen output hint is therefore ignored, not mistaken for a
    /// real multi-output KMS policy. The client still owns its pixels.
    fn request_managed_fullscreen(&mut self,top:ToplevelSurface,enabled:bool) {
        if !self.managed_xdg_mode {
            top.send_configure();
            return;
        }
        let surface=top.wl_surface().clone();
        let Some((_,window))=self.managed_windows.iter_mut()
            .find(|(role,_)|role==&surface) else {
            top.send_configure();
            return;
        };
        let changed=if enabled {window.fullscreen()} else {
            if window.state()==WindowDisplayState::Fullscreen {
                window.restore()
            } else {false}
        };
        let rectangle=window.geometry();
        if changed { self.cancel_surface_grab(&surface); }
        top.with_pending_state(|pending| {
            pending.states.unset(xdg_toplevel::State::Resizing);
            if enabled {
                pending.states.set(xdg_toplevel::State::Fullscreen);
                pending.states.unset(xdg_toplevel::State::Maximized);
            } else {
                pending.states.unset(xdg_toplevel::State::Fullscreen);
            }
            if let Some(rect)=rectangle {
                pending.size=Some((rect.width,rect.height).into());
            }
        });
        top.send_pending_configure();
        if changed {
            if let Some(rect)=rectangle {
                // The configure is only a request. A previous wl_shm image
                // must NOT be moved or submitted to KMS before the matching
                // client buffer has been committed and validated.
                self.managed_pending_resize.retain(|(role,_)|role!=&surface);
                self.managed_pending_sizes.retain(|(role,_)|role!=&surface);
                self.managed_pending_sizes.push((
                    surface.clone(),(rect.width as usize,rect.height as usize)
                ));
            }
        }
        println!("MELANIA_NATIVE_MANAGED_XDG_FULLSCREEN={enabled}");
    }
}

impl BridgeState {
    /// The current compositor uses the *same* bounded IPC protocol as
    /// Melania Panel, without adding a QEMU-specific command syntax.
    fn control_command(&mut self,command:ControlCommand)->String {
        match command {
            ControlCommand::List=>{
                let windows=self.managed_windows.iter().filter_map(|(surface,model)|{
                    let id=self.known_window_surfaces.id_of(surface)?;
                    let (title,app_id)=with_states(surface,|states|{
                        states.data_map.get::<XdgToplevelSurfaceData>()
                            .map(|data|{
                                let data=data.lock().unwrap();
                                (data.title.clone(),data.app_id.clone())
                            }).unwrap_or((None,None))
                    });
                    Some(WindowInfo{
                        id,
                        title:title.filter(|text|!text.is_empty())
                            .unwrap_or_else(||format!("Finestra {id}"))
                            .chars().take(96).collect(),
                        app_id:app_id.filter(|text|!text.is_empty())
                            .unwrap_or_else(||"applicazione-wayland".into())
                            .chars().take(96).collect(),
                        active:self.focused.as_ref()==Some(surface),
                        minimized:model.state()==WindowDisplayState::Minimized,
                    })
                }).collect::<Vec<_>>();
                respond_list(&windows)
            },
            ControlCommand::Status=>respond_workspace(1),
            ControlCommand::Activate(id)=>{
                let Some(surface)=self.known_window_surfaces.get(id).cloned() else {
                    return respond_error("Finestra non trovata");
                };
                let Some((_,model))=self.managed_windows.iter_mut()
                    .find(|(role,_)|role==&surface) else {
                    return respond_error("Ruolo finestra non trovato");
                };
                let minimized=model.state()==WindowDisplayState::Minimized;
                if minimized {model.restore();}
                let rect=model.geometry();
                let mut changed=false;
                if self.dynamic_scene.geometry(id).is_some(){
                    if minimized {
                        changed|=self.dynamic_scene.set_visible(id,true).unwrap_or(false);
                    }
                    if let Some(rect)=rect {
                        if self.dynamic_scene.position(id)!=Some((rect.x,rect.y)){
                            changed|=self.dynamic_scene.move_to(id,rect.x,rect.y).is_ok();
                        }
                    }
                    changed|=self.dynamic_scene.raise(id).unwrap_or(false);
                }
                if changed {
                    self.note_scene_change();
                    if self.continuous_kms_mode &&
                       (self.persistent_kms_mode || self.continuous_frame_count<self.continuous_frame_target) {
                        if let Err(error)=self.submit_continuous_frame(){
                            eprintln!("native IPC restore KMS rejected: {error}");
                        }
                    }
                }
                if let Some(top)=self.xdg_shell.toplevel_surfaces().iter()
                    .find(|top|top.wl_surface()==&surface){
                    top.with_pending_state(|pending|{
                        pending.states.set(xdg_toplevel::State::Activated);
                        if let Some(rect)=rect{
                            pending.size=Some((rect.width,rect.height).into());
                        }
                    });
                    top.send_pending_configure();
                }
                let keyboard=self.seat.get_keyboard().expect("keyboard");
                keyboard.set_focus(self,Some(surface.clone()),SERIAL_COUNTER.next_serial());
                self.focused=Some(surface);
                println!("MELANIA_NATIVE_CONTROL_ACTIVATED={id}");
                ACK.into()
            },
            ControlCommand::Minimize(id) | ControlCommand::Close(id)=>{
                let top=self.xdg_shell.toplevel_surfaces().iter()
                    .find(|top|self.known_window_surfaces.id_of(top.wl_surface())==Some(id))
                    .cloned();
                let Some(top)=top else {return respond_error("Finestra non trovata");};
                if matches!(command,ControlCommand::Minimize(_)){
                    <BridgeState as XdgShellHandler>::minimize_request(self,top);
                } else {top.send_close();}
                ACK.into()
            },
            _=>respond_error("Comando non supportato nel compositore nativo QEMU"),
        }
    }
}

impl BufferHandler for BridgeState {
    fn buffer_destroyed(&mut self,_:&wl_buffer::WlBuffer) {}
}
impl CompositorHandler for BridgeState {
    fn compositor_state(&mut self)->&mut CompositorState {&mut self.compositor}
    fn client_compositor_state<'a>(&self,client:&'a Client)->&'a CompositorClientState {
        &client.get_data::<BridgeClientState>().expect("registered client").compositor
    }
    fn commit(&mut self,surface:&WlSurface) {
        if self.layer_pixels_mode {
            if let Some(role)=self.layer_surfaces.iter().find(|(_,r)|r.wl_surface()==surface)
                .map(|(_,r)|r.clone()) {
                // GTK may move a still-mapped Start layer off-screen by
                // committing only its margin/role state (no new wl_shm
                // buffer). Revoke focus on that real state transition too.
                let is_start_layer=self.layer_surfaces.iter().any(|(name,r)|
                    name=="melania-start" && r.wl_surface()==surface);
                if is_start_layer {
                    let left=with_states(surface,|states|{
                        states.cached_state.get::<LayerSurfaceCachedState>()
                            .current().margin.left
                    });
                    if left<=-510 {
                        self.refresh_qemu_gtk_start_focus(surface,false);
                    }
                }
                // A protocol configure is NOT pixel evidence. Only a client
                // wl_shm buffer copied out of Smithay's live mapping enters
                // the native retained scene and the Rust KMS worker.
                let assignment=with_states(surface,|states|{
                    states.cached_state.get::<SurfaceAttributes>().current().buffer.take()
                });
                if let Some(BufferAssignment::NewBuffer(buffer))=assignment {
                    let imported=with_buffer_contents(&buffer,|ptr,len,metadata|{
                        melania_native::owned_shm::copy_bounded_client_shm(ptr,len,metadata)
                    });
                    let pixels=match imported {
                        Ok(Ok(image))=>match WindowPixels::from_owned_shm(image) {
                            Ok(image)=>image,
                            Err(_)=>{
                                eprintln!("MELANIA_NATIVE_LAYER_SHM_INVALID=true");
                                buffer.release();
                                return;
                            }
                        },
                        _=>{
                            eprintln!("MELANIA_NATIVE_LAYER_SHM_INVALID=true");
                            buffer.release();
                            return;
                        }
                    };
                    let attrs=with_states(surface,|states|{
                        *states.cached_state.get::<LayerSurfaceCachedState>().current()
                    });
                    let shell_layer=match attrs.layer {
                        WlrLayer::Background=>ShellLayer::Background,
                        WlrLayer::Bottom=>ShellLayer::Bottom,
                        WlrLayer::Top=>ShellLayer::Top,
                        WlrLayer::Overlay=>ShellLayer::Overlay,
                    };
                    let requested=LayerRequest {
                        layer:shell_layer,
                        anchors:Anchors{
                            top:attrs.anchor.contains(WlrAnchor::TOP),
                            bottom:attrs.anchor.contains(WlrAnchor::BOTTOM),
                            left:attrs.anchor.contains(WlrAnchor::LEFT),
                            right:attrs.anchor.contains(WlrAnchor::RIGHT),
                        },
                        width:attrs.size.w.max(0) as u32,
                        height:attrs.size.h.max(0) as u32,
                        margins:Insets{top:attrs.margin.top,right:attrs.margin.right,
                                       bottom:attrs.margin.bottom,left:attrs.margin.left},
                        exclusive_zone:i32::from(attrs.exclusive_zone),
                    };
                    let placement=match layer_surface_layout::place(requested,1280,800) {
                        Ok(v)=>v,
                        Err(error)=>{
                            eprintln!("MELANIA_NATIVE_LAYER_LAYOUT_REJECTED={error:?}");
                            buffer.release();
                            return;
                        }
                    };
                    if pixels.width()!=placement.rect.width as usize ||
                       pixels.height()!=placement.rect.height as usize {
                        eprintln!("MELANIA_NATIVE_LAYER_CONFIGURE_BUFFER_MISMATCH=true");
                        buffer.release();
                        return;
                    }
                    // Restrict allocation to the dynamic QEMU scene, and
                    // retain the layer's z-index for render and pointer hits.
                    let key=match self.known_window_surfaces.register(surface.clone()) {
                        Ok(key)=>key,
                        Err(_)=>{
                            buffer.release();
                            return;
                        }
                    };
                    // Start is on-screen only inside the actual bounded
                    // QEMU output. The hidden GTK role stays mapped offscreen.
                    let start_role=self.layer_surfaces.iter().any(|(name,role)|{
                        name=="melania-start" && role.wl_surface()==surface
                    });
                    let start_visible=placement.rect.x>=0 && placement.rect.y>=0 &&
                        placement.rect.x<1280 && placement.rect.y<800;
                    if start_role && !start_visible {
                        self.refresh_qemu_gtk_start_focus(surface,false);
                    }
                    let role_layer:SceneLayer=placement.layer.scene_layer();
                    if self.dynamic_scene.upsert_in_layer(
                        key,placement.rect.x,placement.rect.y,role_layer,pixels
                    ).is_err() {
                        eprintln!("MELANIA_NATIVE_LAYER_SCENE_REJECTED=true");
                        buffer.release();
                        return;
                    }
                    if start_role && start_visible {
                        // This wl_shm buffer passed both geometry and actual
                        // scene import: it is now eligible for keyboard focus.
                        self.refresh_qemu_gtk_start_focus(surface,true);
                    }
                    match self.continuous_client_ledger.stage(surface.clone(),buffer) {
                        Ok(Some(previous))=>previous.release(),
                        Ok(None)=>{},
                        Err((_,rejected))=>{
                            rejected.release();
                            eprintln!("MELANIA_NATIVE_LAYER_KMS_QUEUE_FULL=true");
                            return;
                        }
                    }
                    self.note_scene_change();
                    if let Err(error)=self.submit_continuous_frame() {
                        eprintln!("MELANIA_NATIVE_LAYER_KMS_SUBMIT_FAILED={error}");
                    } else {
                        println!("MELANIA_NATIVE_LAYER_OWNED_SHM_QUEUED={}x{} layer={role_layer:?}",
                                 placement.rect.width,placement.rect.height);
                    }
                }
                // Never autofocus noninteractive taskbars on wl_surface.commit.
                return;
            }
        }
        // A role-only GTK handshake must never focus hidden/noninteractive
        // layers; it does not attach a wl_shm image or simulate a GTK frame.
        if self.gtk_layer_mode && !self.layer_pixels_mode &&
           self.layer_surfaces.iter().any(|(_,r)|r.wl_surface()==surface) {
            return;
        }
        if self.continuous_kms_mode {
            let assignment=with_states(surface,|states|{
                states.cached_state.get::<SurfaceAttributes>().current().buffer.take()
            });
            if let Some(BufferAssignment::NewBuffer(buffer))=assignment {
                // The importer owns the snapshot before the Smithay pool
                // access callback returns. Fixed QEMU uses its original
                // 360x224 checks; the separate dynamic VM path uses Stage51.
                if self.dynamic_window_mode {
                    let imported=with_buffer_contents(&buffer,|ptr,len,metadata|{
                        melania_native::owned_shm::copy_bounded_client_shm(ptr,len,metadata)
                    });
                    let pixels=match imported {
                        Ok(Ok(image))=>match WindowPixels::from_owned_shm(image) {
                            Ok(pixels)=>pixels,
                            Err(_)=>{
                                eprintln!("dynamic QEMU client has invalid image");
                                buffer.release();
                                return;
                            }
                        },
                        _=>{
                            eprintln!("dynamic QEMU client submitted invalid wl_shm");
                            buffer.release();
                            return;
                        }
                    };
                    let popup_parent=self.native_popup_roles.iter()
                        .find(|(popup,_)|popup.wl_surface()==surface)
                        .map(|(popup,parent)|(popup.clone(),parent.clone()));
                    // Do not turn an unconfigured / rejected XDG popup into a
                    // free-floating normal window by registering its buffer.
                    if self.managed_xdg_mode &&
                       popup_parent.is_none() &&
                       self.xdg_shell.popup_surfaces().iter()
                           .any(|role|role.wl_surface()==surface) {
                        eprintln!("MELANIA_NATIVE_XDG_POPUP_PARENT_REJECTED=true");
                        buffer.release();
                        return;
                    }
                    // An invalid/rejected popup must never reserve a stable
                    // client slot and block unrelated mapped windows.
                    let key=match self.known_window_surfaces.register(surface.clone()) {
                        Ok(key)=>key,
                        Err(_)=>{
                            eprintln!("dynamic QEMU client has no scene slot");
                            buffer.release();
                            return;
                        }
                    };
                    let Some(default_pos)=melania_native::window_placement::default_position(key,1280,800) else {
                        eprintln!("dynamic QEMU client outside bounded placement policy");
                        buffer.release();
                        return;
                    };
                    // Refuse stale-size buffers during a managed role switch.
                    // A successful client ACK alone did not provide pixels.
                    if self.managed_xdg_mode && popup_parent.is_none() {
                        let expected=self.managed_pending_sizes.iter()
                            .find(|(role,_)|role==surface).map(|(_,size)|*size);
                        if !accepts_managed_role_buffer(expected,(pixels.width(),pixels.height())) {
                            eprintln!("MELANIA_NATIVE_MANAGED_XDG_STALE_BUFFER_REJECTED=true");
                            buffer.release();
                            return;
                        }
                    }
                    // Popup geometry is XDG positioner state ACKed by the
                    // client, relative to its mapped parent, NEVER a guessed
                    // slot or a toplevel's default placement.
                    let placement=if let Some((popup,parent))=&popup_parent {
                        let Some(parent_key)=self.known_window_surfaces.id_of(parent) else {
                            buffer.release();
                            return;
                        };
                        let Some((px,py,_,_))=self.dynamic_scene.geometry(parent_key) else {
                            buffer.release();
                            return;
                        };
                        let Some(geometry)=acked_native_popup_geometry(popup.wl_surface()) else {
                            eprintln!("MELANIA_NATIVE_XDG_POPUP_UNACKED_BUFFER_REJECTED=true");
                            buffer.release();
                            return;
                        };
                        let Some(layout)=PopupPlacement::new(
                            parent_key,geometry.loc.x,geometry.loc.y,
                            geometry.size.w,geometry.size.h
                        ) else {
                            buffer.release();
                            return;
                        };
                        if !layout.matches_buffer(pixels.width(),pixels.height()) ||
                           layout.absolute(px,py,1280,800).is_none() {
                            eprintln!("MELANIA_NATIVE_XDG_POPUP_SIZE_MISMATCH=true");
                            buffer.release();
                            return;
                        }
                        layout.absolute(px,py,1280,800)
                    } else {None};
                    // Keep the current position across later wl_shm commits.
                    let (x,y)=if let Some(position)=placement {
                        position
                    } else if self.managed_xdg_mode {
                        // The top/left edge moves with the exact matching
                        // resized wl_shm commit, not when XDG configure is sent.
                        self.managed_pending_resize.iter()
                            .find(|(role,_)|role==surface)
                            .map(|(_,rect)|(rect.x,rect.y))
                            .or_else(||self.managed_windows.iter()
                                .find(|(role,_)|role==surface)
                                .and_then(|(_,window)|window.geometry())
                                .map(|rect|(rect.x,rect.y)))
                            .unwrap_or(default_pos)
                    } else {
                        self.dynamic_scene.position(key).unwrap_or(default_pos)
                    };
                    let width=pixels.width() as i32;
                    let height=pixels.height() as i32;
                    if (if popup_parent.is_some() {
                        self.dynamic_scene.upsert_in_layer(key,x,y,SceneLayer::Top,pixels)
                    } else {
                        self.dynamic_scene.upsert(key,x,y,pixels)
                    }).is_err() {
                        eprintln!("dynamic QEMU client image rejected");
                        buffer.release();
                        return;
                    }
                    if self.managed_xdg_mode && popup_parent.is_none() {
                        // Change the remembered normal geometry only after
                        // the owned client pixels actually entered the scene.
                        if let Some((_,window))=self.managed_windows.iter_mut()
                            .find(|(role,_)|role==surface) {
                            window.update_normal_geometry(WindowRect{x,y,width,height});
                        }
                        self.managed_pending_sizes.retain(|(role,_)|role!=surface);
                        self.managed_pending_resize.retain(|(role,_)|role!=surface);
                        println!("MELANIA_NATIVE_MANAGED_XDG_CLIENT_BUFFER_ACCEPTED={}x{}",width,height);
                    }
                    if popup_parent.is_some() {
                        println!("MELANIA_NATIVE_XDG_POPUP_CLIENT_BUFFER_ACCEPTED={}x{}",width,height);
                    }
                } else {
                    let imported=with_buffer_contents(&buffer,|ptr,len,metadata|{
                        native_client_frame::copy_client_shm(ptr,len,metadata)
                    });
                    let pixels=match imported {
                        Ok(Ok(pixels))=>pixels,
                        _=>{
                            eprintln!("continuous QEMU client submitted invalid wl_shm");
                            return;
                        }
                    };
                    let key=match self.known_window_surfaces.register(surface.clone()) {
                        Ok(key)=>key,
                        Err(_)=>{
                            eprintln!("continuous QEMU client has no free scene slot");
                            return;
                        }
                    };
                    if self.multi_scene.upsert(key,pixels).is_err() {
                        eprintln!("continuous QEMU scene pixels rejected");
                        return;
                    }
                }
                // Retain at most one not-yet-submitted client buffer. Its
                // pixels were copied into the bounded retained scene.
                match self.continuous_client_ledger.stage(surface.clone(),buffer) {
                    Ok(Some(superseded))=>superseded.release(),
                    Ok(None)=>{},
                    Err((_,rejected))=>{
                        rejected.release();
                        eprintln!("continuous Wayland client receipt capacity exceeded");
                        return;
                    }
                }
                self.note_scene_change();
                if let Err(error)=self.submit_continuous_frame() {
                    eprintln!("continuous QEMU KMS scheduling failed: {error}");
                }
            }
            return;
        }
        if self.reopen_redraw_mode && self.pixel_frame_count==5 {
            let assignment=with_states(surface,|states|{
                states.cached_state.get::<SurfaceAttributes>().current().buffer.take()
            });
            if let Some(BufferAssignment::NewBuffer(buffer))=assignment {
                if let Err(error)=self.submit_reopen_frame(surface,buffer) {
                    eprintln!("QEMU real Wayland reopen buffer rejected: {error}");
                }
                return;
            }
        }
        if self.pixel_probe && self.pixel_frame_count<self.pixel_frame_target &&
           self.pixel_frame_count<3 && self.xdg_toplevel_count>0 {
            let assignment=with_states(surface,|states|{
                states.cached_state.get::<SurfaceAttributes>().current().buffer.take()
            });
            if let Some(BufferAssignment::NewBuffer(buffer))=assignment {
                // In the two-window VM test each COMMIT carries pixel data
                // from a distinct wl_surface. Keep the previous window's
                // last owned snapshot for the next full-scene composition.
                let imported=with_buffer_contents(&buffer,|ptr,len,metadata|{
                    if self.multi_window_mode {
                        native_client_frame::copy_client_shm(ptr,len,metadata)
                    } else {
                        native_client_frame::compose_from_shm(ptr,len,metadata)
                    }
                });
                match imported {
                    Ok(Ok(imported))=>{
                        let frame=if self.multi_window_mode {
                            let key=match self.known_window_surfaces.register(surface.clone()) {
                                Ok(key)=>key,
                                Err(_)=>{
                                    eprintln!("QEMU two-window scene exceeded bound");
                                    return;
                                },
                            };
                            if self.multi_scene.upsert(key,imported).is_err() {
                                eprintln!("QEMU retained window rejected invalid wl_shm image");
                                return;
                            }
                            self.note_scene_change();
                            match self.multi_scene.composite() {
                                Ok(frame)=>frame,
                                Err(error)=>{
                                    eprintln!("QEMU multi-window composition failed: {error:?}");
                                    return;
                                }
                            }
                        } else {imported};
                        let next=self.pixel_frame_count+1;
                        // This still uses a bounded, synchronous QEMU stream.
                        // Stage32 adds verified frame identity and coalesced
                        // damage semantics, NOT an asynchronous renderer yet.
                        let scheduled:Option<FrameTicket>=if self.live_stream {
                            if !self.render_scheduler.damage() {
                                eprintln!("native render scheduler rejected damage during seat pause");
                                return;
                            }
                            match self.render_scheduler.begin_frame() {
                                Some(ticket)=>Some(ticket),
                                None=>{
                                    eprintln!("native frame already outstanding or seat inactive");
                                    return;
                                }
                            }
                        } else {None};
                        if self.async_kms_mode {
                            let ticket=scheduled.ok_or("scheduled KMS ticket missing");
                            let ticket=match ticket {
                                Ok(ticket)=>ticket,
                                Err(error)=>{eprintln!("{error}");return;}
                            };
                            if self.async_pending.is_some() {
                                eprintln!("second frame while asynchronous KMS is presenting");
                                self.render_scheduler.fail_present(ticket);
                                return;
                            }
                            if let Err(error)=self.begin_generation_proof(self.multi_scene.layout_generation()) {
                                eprintln!("QEMU generation audit refused native frame: {error}");
                                self.render_scheduler.fail_present(ticket);
                                return;
                            }
                            let worker=match self.async_stream.as_ref() {
                                Some(worker)=>worker,
                                None=>{
                                    eprintln!("async KMS transport not connected");
                                    self.reject_generation_proof();
                                    self.render_scheduler.fail_present(ticket);
                                    return;
                                }
                            };
                            if worker.submit(FrameJob{ticket,ordinal:next,pixels:frame}).is_err() {
                                eprintln!("async KMS transport full or disconnected");
                                self.reject_generation_proof();
                                self.render_scheduler.fail_present(ticket);
                                return;
                            }
                            self.async_pending=Some(PendingAsyncFrame {
                                client:Some((surface.clone(),buffer)),ticket,ordinal:next,
                                layout_generation:self.multi_scene.layout_generation(),
                            });
                            println!("MELANIA_NATIVE_ASYNC_FRAME_SUBMITTED={next}");
                            let _=std::io::stdout().flush();
                            // Return to Smithay NOW. No KMS socket I/O or
                            // waiting for a page-flip acknowledgement here.
                            return;
                        }
                        let handoff:Result<(),Box<dyn Error>>=if self.live_stream {
                            (||->Result<(),Box<dyn Error>>{
                                let stream=self.stream.as_mut()
                                    .ok_or("native DRM stream not connected")?;
                                let length=u32::try_from(frame.len())?;
                                stream.write_all(&length.to_le_bytes())?;
                                stream.write_all(&frame)?;
                                stream.flush()?;
                                let mut ack=[0u8;1];
                                stream.read_exact(&mut ack)?;
                                if ack[0]!=next {
                                    return Err("Rust KMS did not acknowledge the displayed frame".into());
                                }
                                println!("MELANIA_NATIVE_LIVE_KMS_FRAME_ACK={next}");
                                Ok(())
                            })()
                        } else {
                            native_client_frame::write_frame_index(&frame,self.pixel_frame_count)
                        };
                        match handoff {
                            Ok(())=>{
                                if let Some(ticket)=scheduled {
                                    // Do not send a wl_surface.frame completion
                                    // for an invalidated or duplicated ticket.
                                    if !self.render_scheduler.complete_present(ticket) {
                                        eprintln!("native KMS ACK rejected: stale frame ticket");
                                        return;
                                    }
                                    println!("MELANIA_NATIVE_SCHEDULED_FRAME_ACK={next}");
                                }
                                self.pixel_frame_count=next;
                                self.pixel_ready=self.pixel_frame_count==self.pixel_frame_target;
                                println!("MELANIA_NATIVE_CLIENT_SHM_IMPORTED=true");
                                println!("MELANIA_NATIVE_CLIENT_FRAME_INDEX={}",self.pixel_frame_count);
                                println!("MELANIA_NATIVE_CLIENT_FRAME_COMPOSED=true");
                                if self.live_stream && self.frame_callbacks_probe {
                                    // ACK follows successful FB_ID check in drm-rs.
                                    // A frame callback before that point would
                                    // falsely tell the client rendering completed.
                                    let count=complete_presented_frame(
                                        surface,self.pixel_frame_count as u32*16
                                    );
                                    self.frame_callback_count+=count;
                                    println!("MELANIA_NATIVE_FRAME_CALLBACK_DONE={}",self.pixel_frame_count);
                                    println!("MELANIA_NATIVE_FRAME_CALLBACK_COUNT={}",self.frame_callback_count);
                                }
                                let _=std::io::stdout().flush();
                            },
                            Err(error)=>{
                                if let Some(ticket)=scheduled {
                                    self.render_scheduler.fail_present(ticket);
                                }
                                eprintln!("Melania Wayland frame transfer failed: {error}");
                                // Do not acknowledge/release an unpresented wl_buffer.
                                return;
                            },
                        }
                    },
                    Ok(Err(error))=>eprintln!("Melania invalid client pixels: {error}"),
                    Err(error)=>eprintln!("Melania wl_shm access error: {error:?}"),
                }
                buffer.release();
            }
        }
        if self.focused.is_some() {return;}
        self.focused=Some(surface.clone());
        let key=self.seat.get_keyboard().expect("keyboard");
        key.set_focus(self,Some(surface.clone()),SERIAL_COUNTER.next_serial());
        let pointer=self.seat.get_pointer().expect("pointer");
        pointer.motion(self,Some((surface.clone(),(0.0,0.0).into())),
            &MotionEvent{location:(0.0,0.0).into(),
                         serial:SERIAL_COUNTER.next_serial(),time:0});
        pointer.frame(self);
        println!("MELANIA_NATIVE_WAYLAND_SURFACE_FOCUSED=true");
        let _=std::io::stdout().flush();
    }
}
impl OutputHandler for BridgeState {}
impl ShmHandler for BridgeState {
    fn shm_state(&self)->&ShmState {&self.shm}
}
impl SeatHandler for BridgeState {
    type KeyboardFocus=WlSurface;
    type PointerFocus=WlSurface;
    type TouchFocus=WlSurface;
    fn seat_state(&mut self)->&mut SeatState<Self> {&mut self.seats}
    fn focus_changed(&mut self,_:&Seat<Self>,_:Option<&WlSurface>){}
    fn cursor_image(&mut self,_:&Seat<Self>,_:CursorImageStatus){}
}
// Match the protocol implementations of the nested compositor, but keep
// this guest-specific backend headless: configuring a role is NOT rendering.
impl XdgShellHandler for BridgeState {
    fn xdg_shell_state(&mut self)->&mut XdgShellState {&mut self.xdg_shell}
    fn new_toplevel(&mut self,top:ToplevelSurface) {
        self.xdg_toplevel_count=self.xdg_toplevel_count.saturating_add(1);
        if self.managed_xdg_mode {
            // XDG configure requests may precede the first buffer commit.
            // Reserve its stable role slot without pretending pixels exist.
            if let Ok(key)=self.known_window_surfaces.register(top.wl_surface().clone()) {
                if let Some((x,y))=melania_native::window_placement::default_position(key,1280,800) {
                    let rect=WindowRect{x,y,width:360,height:224};
                    if let Some(window)=ManagedWindow::new(rect,1280,800) {
                        self.managed_windows.push((top.wl_surface().clone(),window));
                    }
                }
            }
        }
        if self.dynamic_xdg_resize_mode {
            // Preserve original role order for A/B even before either has
            // attached pixels. The Stage54 QEMU client labels them explicitly.
            self.dynamic_toplevels.push(top.clone());
        }
        top.send_configure();
        println!("MELANIA_NATIVE_XDG_TOPLEVEL_CONFIGURE_SENT=true");
    }
    fn new_popup(&mut self,popup:PopupSurface,positioner:PositionerState) {
        if !self.managed_xdg_mode || !self.dynamic_window_mode {
            let _=popup.send_configure();
            return;
        }
        let Some(parent)=popup.get_parent_surface() else {
            popup.send_popup_done();
            return;
        };
        let Some(parent_id)=self.known_window_surfaces.id_of(&parent) else {
            popup.send_popup_done();
            return;
        };
        let Some((parent_x,parent_y,_,_))=self.dynamic_scene.geometry(parent_id) else {
            popup.send_popup_done();
            return;
        };
        if self.native_popup_roles.len()>=8 {
            popup.send_popup_done();
            return;
        }
        let output=Rectangle::from_loc_and_size(
            (-parent_x,-parent_y),(1280,800));
        let geometry=positioner.get_unconstrained_geometry(output);
        let Some(placement)=PopupPlacement::new(
            parent_id,geometry.loc.x,geometry.loc.y,
            geometry.size.w,geometry.size.h
        ) else {
            popup.send_popup_done();
            return;
        };
        if placement.absolute(parent_x,parent_y,1280,800).is_none() {
            popup.send_popup_done();
            return;
        }
        popup.with_pending_state(|pending|{
            pending.positioner=positioner;
            pending.geometry=geometry;
        });
        self.native_popup_roles.push((popup.clone(),parent));
        let _=popup.send_configure();
        println!("MELANIA_NATIVE_XDG_POPUP_CONFIGURED={}x{}",
                 placement.width,placement.height);
    }
    fn move_request(&mut self,top:ToplevelSurface,seat:wl_seat::WlSeat,serial:Serial){
        // Stage113: accept only a real left-button serial issued to this
        // exact focused xdg_toplevel while the libseat KMS epoch is active.
        // This shares the actual pointer-motion -> scene -> KMS ACK path;
        // it does not create a synthetic move or change window decorations.
        if !self.managed_xdg_mode || !self.dynamic_window_mode ||
           !self.seat.owns(&seat) || !self.lifecycle.seat_active() ||
           self.native_presentation_guard.permanently_revoked() {
            return;
        }
        let surface=top.wl_surface();
        if self.focused.as_ref()!=Some(surface) ||
           !take_xdg_pointer_press(&mut self.last_xdg_press,surface,serial) {
            return;
        }
        let Some(key)=self.known_window_surfaces.id_of(surface) else{return;};
        let Some((x,y,width,height))=self.dynamic_scene.geometry(key) else{return;};
        let rect=WindowRect{x,y,width:width as i32,height:height as i32};
        if self.window_interaction.begin(
            key,rect,self.pointer_position.x,self.pointer_position.y,
            InteractionKind::Move,
        ) {
            println!("MELANIA_NATIVE_XDG_CLIENT_MOVE_GRANTED={key}");
        }
    }
    fn resize_request(&mut self,top:ToplevelSurface,seat:wl_seat::WlSeat,
                      serial:Serial,edge:xdg_toplevel::ResizeEdge){
        let Some(direction)=xdg_resize_direction(edge) else{return;};
        if !self.managed_xdg_mode || !self.dynamic_window_mode ||
           !self.multi_window_mode || !self.seat.owns(&seat) ||
           !self.lifecycle.seat_active() ||
           self.native_presentation_guard.permanently_revoked() {
            return;
        }
        let surface=top.wl_surface();
        if self.focused.as_ref()!=Some(surface) ||
           !take_xdg_pointer_press(&mut self.last_xdg_press,surface,serial) {
            return;
        }
        if !self.managed_windows.iter().any(|(role,window)|
            role==surface && window.state()==WindowDisplayState::Normal) {
            return;
        }
        let Some(key)=self.known_window_surfaces.id_of(surface) else{return;};
        let Some((x,y,width,height))=self.dynamic_scene.geometry(key) else{return;};
        let rect=WindowRect{x,y,width:width as i32,height:height as i32};
        if self.window_interaction.begin(
            key,rect,self.pointer_position.x,self.pointer_position.y,
            InteractionKind::Resize(direction),
        ) {
            println!("MELANIA_NATIVE_XDG_CLIENT_RESIZE_GRANTED={key} edge={direction:?}");
        }
    }
    fn maximize_request(&mut self,top:ToplevelSurface) {
        self.request_managed_maximize(top,true);
    }
    fn unmaximize_request(&mut self,top:ToplevelSurface) {
        self.request_managed_maximize(top,false);
    }
    fn fullscreen_request(&mut self,top:ToplevelSurface,_:Option<WlOutput>) {
        self.request_managed_fullscreen(top,true);
    }
    fn unfullscreen_request(&mut self,top:ToplevelSurface) {
        self.request_managed_fullscreen(top,false);
    }
    fn minimize_request(&mut self,top:ToplevelSurface){
        if !self.managed_xdg_mode {return;}
        let surface=top.wl_surface().clone();
        let Some((_,model))=self.managed_windows.iter_mut()
            .find(|(role,_)|role==&surface) else {return;};
        if !model.minimize(){return;}
        self.managed_pending_resize.retain(|(role,_)|role!=&surface);
        self.managed_pending_sizes.retain(|(role,_)|role!=&surface);
        self.window_interaction.cancel();
        if let Some(key)=self.known_window_surfaces.id_of(&surface){
            if self.dynamic_scene.geometry(key).is_some() &&
               self.dynamic_scene.set_visible(key,false)==Ok(true) {
                self.note_scene_change();
                if self.continuous_kms_mode &&
                   (self.persistent_kms_mode || self.continuous_frame_count<self.continuous_frame_target) {
                    if let Err(error)=self.submit_continuous_frame(){
                        eprintln!("managed xdg minimize repaint refused: {error}");
                    }
                }
            }
        }
        // A hidden surface must lose keyboard and pointer focus.
        if self.focused.as_ref()==Some(&surface){
            self.focused=None;
            let target=self.pointer_target(self.pointer_position);
            let replacement=target.as_ref().map(|(next,_)|next.clone());
            let keyboard=self.seat.get_keyboard().expect("keyboard");
            keyboard.set_focus(self,replacement.clone(),SERIAL_COUNTER.next_serial());
            self.focused=replacement;
            let pointer=self.seat.get_pointer().expect("pointer");
            pointer.motion(self,target,&MotionEvent {
                location:self.pointer_position,
                serial:SERIAL_COUNTER.next_serial(),time:0,
            });
            pointer.frame(self);
        }
        println!("MELANIA_NATIVE_MANAGED_XDG_MINIMIZED=true");
    }
    fn toplevel_destroyed(&mut self,top:ToplevelSurface){
        let surface=top.wl_surface().clone();
        if self.last_xdg_press.as_ref().is_some_and(|(pressed,_)|pressed==&surface) {
            self.last_xdg_press=None;
        }
        self.managed_windows.retain(|(role,_)|role!=&surface);
        self.managed_pending_sizes.retain(|(role,_)|role!=&surface);
        self.managed_pending_resize.retain(|(role,_)|role!=&surface);
        if self.dynamic_xdg_resize_mode {
            self.dynamic_toplevels.retain(|existing|existing.wl_surface()!=&surface);
        }
        if let Some(key)=self.known_window_surfaces.remove(&surface) {
            // The retained pixel copy and stable slot belong to this client.
            // Never leave a closed window occupying the limited QEMU scene.
            let removed=if self.dynamic_window_mode {
                self.dynamic_scene.remove(key)
            } else {
                self.multi_scene.remove(key)
            };
            if removed { self.note_scene_change(); }
            if removed && self.continuous_kms_mode &&
               (self.persistent_kms_mode || self.continuous_frame_count<self.continuous_frame_target) {
                // A destroyed xdg surface changes retained pixels even if
                // no surviving client submits a new wl_shm buffer.
                if let Err(error)=self.submit_continuous_frame() {
                    eprintln!("continuous close KMS frame not scheduled: {error}");
                }
            }
            if removed && self.close_redraw_mode &&
               self.multi_scene.window_count()==1 && self.pixel_frame_count>=3 {
                // Defer the redraw until the outstanding fourth KMS frame
                // receives its ACK. The scheduler coalesces this damage.
                if !self.render_scheduler.damage() {
                    eprintln!("QEMU native close damage ignored on paused seat");
                } else {
                    println!("MELANIA_NATIVE_CLOSE_DAMAGE_QUEUED=true");
                }
            }
            if self.window_interaction.active_key()==Some(key) {
                self.window_interaction.cancel();
            }
            println!("MELANIA_NATIVE_WINDOW_REMOVED={key}");
            if self.close_redraw_mode && key==1 {
                println!("MELANIA_NATIVE_CLIENT_FIRST_WINDOW_DESTROYED=true");
            }
            let _=std::io::stdout().flush();
        }
        // A closed parent invalidates every XDG popup that was attached
        // to its old surface identity, including their retained GPU pixels.
        // A new toplevel cannot inherit an orphaned popup.
        let descendants=self.native_popup_roles.iter()
            .filter(|(_,parent)|parent==&surface)
            .map(|(popup,_)|popup.clone()).collect::<Vec<_>>();
        for popup in descendants {
            popup.send_popup_done();
            self.evict_native_popup(popup.wl_surface());
        }
        if self.focused.as_ref()==Some(&surface) {
            // Do not forward keys or pointer events to a destroyed Wayland
            // role. Where possible retarget the remaining window beneath the
            // last pointer position; otherwise clear keyboard/pointer focus.
            self.focused=None;
            let target=self.pointer_target(self.pointer_position);
            let replacement=target.as_ref().map(|(next,_)|next.clone());
            let keyboard=self.seat.get_keyboard().expect("keyboard");
            keyboard.set_focus(self,replacement.clone(),SERIAL_COUNTER.next_serial());
            self.focused=replacement;
            let pointer=self.seat.get_pointer().expect("pointer");
            pointer.motion(self,target,&MotionEvent{
                location:self.pointer_position,
                serial:SERIAL_COUNTER.next_serial(),time:0,
            });
            pointer.frame(self);
        }
    }
    fn grab(&mut self,_:PopupSurface,_:wl_seat::WlSeat,_:Serial){}
    fn reposition_request(&mut self,popup:PopupSurface,positioner:PositionerState,token:u32){
        if !self.managed_xdg_mode || !self.dynamic_window_mode {
            popup.send_repositioned(token);
            return;
        }
        let Some((_,parent))=self.native_popup_roles.iter()
            .find(|(role,_)|role.wl_surface()==popup.wl_surface()) else {
            popup.send_popup_done();
            return;
        };
        let Some(parent_id)=self.known_window_surfaces.id_of(parent) else {
            popup.send_popup_done();
            return;
        };
        let Some((px,py,_,_))=self.dynamic_scene.geometry(parent_id) else {
            popup.send_popup_done();
            return;
        };
        let target=Rectangle::from_loc_and_size((-px,-py),(1280,800));
        let geometry=positioner.get_unconstrained_geometry(target);
        let Some(placement)=PopupPlacement::new(parent_id,geometry.loc.x,geometry.loc.y,
                                               geometry.size.w,geometry.size.h) else {
            popup.send_popup_done();
            return;
        };
        if placement.absolute(px,py,1280,800).is_none() {
            popup.send_popup_done();
            return;
        }
        popup.with_pending_state(|pending|{
            pending.positioner=positioner;
            pending.geometry=geometry;
        });
        popup.send_repositioned(token);
    }
    fn popup_destroyed(&mut self,popup:PopupSurface) {
        self.evict_native_popup(popup.wl_surface());
    }
}
impl WlrLayerShellHandler for BridgeState {
    fn shell_state(&mut self)->&mut WlrLayerShellState {&mut self.layer_shell}
    fn new_layer_surface(&mut self,surface:WlrLayerSurface,
                         _:Option<WlOutput>,layer:WlrLayer,namespace:String) {
        if self.layer_pixels_mode || self.gtk_layer_mode {
            // Keep Stage82/83A's original 2-surface QEMU behavior unless
            // explicitly exercising the real five-namespace GTK shell.
            let configured=if self.gtk_layer_mode {
                let requested=match layer {
                    WlrLayer::Background=>ShellLayer::Background,
                    WlrLayer::Bottom=>ShellLayer::Bottom,
                    WlrLayer::Top=>ShellLayer::Top,
                    WlrLayer::Overlay=>ShellLayer::Overlay,
                };
                let Some(spec)=gtk_layer_geometry::permit(
                    &namespace,requested,
                    self.layer_surfaces.iter().map(|(name,_)|name.as_str())
                ) else {
                    eprintln!("MELANIA_NATIVE_GTK_LAYER_ROLE_REJECTED={namespace}");
                    surface.send_close();
                    return;
                };
                (spec.configured_width as i32,spec.configured_height as i32)
            } else {
                if self.layer_surfaces.len()>=2 {
                    surface.send_close();
                    return;
                }
                (1280,52)
            };
            // Configure based on the real GTK namespace, not one uniform
            // 1280x52 buffer. Each accepted role retains its own identity.
            surface.with_pending_state(|pending|{
                pending.size=Some(configured.into());
            });
            self.layer_surfaces.push((namespace.clone(),surface.clone()));
            if self.gtk_layer_mode {
                println!("MELANIA_NATIVE_GTK_LAYER_CONFIGURED={namespace} size={}x{}",
                         configured.0,configured.1);
            }
        }
        self.layer_surface_count=self.layer_surface_count.saturating_add(1);
        surface.send_configure();
        println!("MELANIA_NATIVE_LAYER_SURFACE_CONFIGURE_SENT=true");
    }
    fn layer_destroyed(&mut self,role:WlrLayerSurface){
        if !self.layer_pixels_mode && !self.gtk_layer_mode {return;}
        let surface=role.wl_surface().clone();
        if self.layer_surfaces.iter().any(|(name,existing)|
            name=="melania-start" && existing.wl_surface()==&surface) {
            self.refresh_qemu_gtk_start_focus(&surface,false);
        }
        self.layer_surfaces.retain(|(_,existing)|existing.wl_surface()!=&surface);
        if let Some(key)=self.known_window_surfaces.remove(&surface) {
            if self.dynamic_scene.remove(key) {
                self.note_scene_change();
                // The layer's old pixels must be invalidated and replaced
                // by a subsequently ACKed native KMS frame.
                if (self.persistent_kms_mode || self.continuous_frame_count<self.continuous_frame_target) {
                    let _=self.submit_continuous_frame();
                }
            }
        }
    }
}
// Advertise the actual wl_data_device_manager protocol to GTK4 clients.
// The QEMU bridge does not yet claim complete clipboard/DnD UX.
impl SelectionHandler for BridgeState {type SelectionUserData=();}
impl ClientDndGrabHandler for BridgeState {}
impl ServerDndGrabHandler for BridgeState {}
impl DataDeviceHandler for BridgeState {
    fn data_device_state(&self)->&DataDeviceState {&self.data_device}
}
delegate_data_device!(BridgeState);
delegate_compositor!(BridgeState);
delegate_seat!(BridgeState);
delegate_xdg_shell!(BridgeState);
delegate_layer_shell!(BridgeState);
delegate_shm!(BridgeState);
delegate_output!(BridgeState);

fn register(state:&mut BridgeState,device:libinput::Device) {
    let caps=melania_native::input_registry::InputCapabilities{
        keyboard:device.has_capability(DeviceCapability::Keyboard),
        pointer:device.has_capability(DeviceCapability::Pointer),
        touch:device.has_capability(DeviceCapability::Touch),
    };
    if caps.any() {
        let _=state.lifecycle.input_registry_mut().device_added(InputDevice::id(&device),caps);
    }
}

/// Relays only actual libinput events belonging to currently registered
/// devices on the ACTIVE libseat generation. No host-wide synthetic key feed.
// libinput wheel deltas are expressed as fractional v120 units. Constrain
// untrusted device values before converting them to Wayland wl_pointer events.
fn bounded_v120(value:Option<f64>)->Option<i32> {
    value.filter(|v|v.is_finite() && v.abs()<=14400.0)
        .map(|v|v.round() as i32)
}

fn handle_input(state:&mut BridgeState,event:InputEvent<LibinputInputBackend>) {
    // No physical input reaches a Wayland client after a VT/seat pause.
    if state.native_presentation_guard
        .ticket(state.lifecycle.seat_active()).is_none() {return;}
    match event {
        InputEvent::DeviceAdded {device} =>register(state,device),
        InputEvent::DeviceRemoved {device} => {
            state.last_xdg_press=None;
            state.window_interaction.cancel();
            state.keyboard_window_cycle.reset();
            state.lifecycle.input_registry_mut().device_removed(&InputDevice::id(&device));
        },
        InputEvent::Keyboard {event} => {
            let device_registered=state.lifecycle.input_registry_mut()
                .event(&InputDevice::id(&event.device())).is_ok();
            // QEMU-only diagnostics: input-send-event succeeding does NOT
            // prove that the guest kernel or libinput delivered a key.
            // Emit the raw evdev code before applying our Start shortcut,
            // including whether libseat accepted the keyboard event.
            if state.real_gtk_pixel_kms_mode &&
               std::env::var("MELANIA_NATIVE_REAL_GTK_START_INPUT")
                   .as_deref()==Ok("YES_QEMU_VM") {
                let raw:u32=event.key_code().into();
                println!("MELANIA_NATIVE_GTK_QEMU_KEY_RAW={} state={:?} registered={} panel_owned={}",
                         raw,event.state(),device_registered,
                         state.known_window_surfaces.get(1).is_some());
                let _=std::io::stdout().flush();
            }
            if !device_registered {return;}
            // Stage87: ACTUAL USB-keyboard -> libinput -> private GTK Start IPC.
            // The isolated QEMU shell has no initial keyboard-focused xdg
            // client, so the legacy focused-only filter would drop Super.
            // This path is opt-in, VM-gated and never enabled in a login or
            // nested user session. Other keyboard shortcuts still use the
            // existing focused-client Wayland event path.
            let real_gtk_super=state.real_gtk_pixel_kms_mode &&
                std::env::var("MELANIA_NATIVE_REAL_GTK_START_INPUT")
                    .as_deref()==Ok("YES_QEMU_VM");
            if real_gtk_super {
                let code:u32=event.key_code().into();
                // Smithay's Keycode is an XKB keycode: the Linux evdev
                // value plus 8. Real QEMU USB LEFTMETA=125 arrived as
                // 133 in the Stage87 guest log, so normalize before
                // testing the physical Super keys.
                let evdev_code=code.checked_sub(8);
                if matches!(evdev_code,Some(125 | 126)) { // KEY_LEFTMETA / KEY_RIGHTMETA
                    if event.state()==KeyState::Pressed {
                        state.qemu_start_super_down=Some(code);
                    } else if state.qemu_start_super_down.take()==Some(code) {
                        // Only after a genuine release and an already owned
                        // GTK taskbar may the UI be asked to reveal Start.
                        if state.known_window_surfaces.get(1).is_some() {
                            let control=std::env::var_os(SOCKET_ENV)
                                .ok_or("Stage87 missing private panel control socket");
                            if let Ok(control)=control {
                                let destination=start_toggle_socket_path(Path::new(&control));
                                match UnixDatagram::unbound().and_then(|socket|
                                    socket.send_to(b"TOGGLE_START",&destination)) {
                                    Ok(_)=>{
                                        println!("MELANIA_NATIVE_GTK_SUPER_KEY_RELEASED=true");
                                        let _=std::io::stdout().flush();
                                    },
                                    Err(error)=>eprintln!("MELANIA_NATIVE_GTK_SUPER_IPC_FAILED={error}"),
                                }
                            }
                        }
                    }
                    return;
                }
                if event.state()==KeyState::Pressed {
                    state.qemu_start_super_down=None;
                }
            }
            // The actual physical XKB keycode (evdev +8) is the sole
            // source of Alt/Tab. Never switch on an IPC command or a client
            // forged wl_keyboard event. Keep the existing GTK Super behavior.
            if state.managed_xdg_mode && state.dynamic_window_mode &&
               state.multi_window_mode && state.window_interaction.active_key().is_none() {
                let code:u32=event.key_code().into();
                let stack=state.dynamic_scene.focusable_normal_keys_top_first();
                let focused=state.focused.as_ref()
                    .and_then(|role|state.known_window_surfaces.id_of(role));
                match state.keyboard_window_cycle.event(
                    code.checked_sub(8),event.state()==KeyState::Pressed,&stack,focused
                ) {
                    ShortcutResult::Forward=>{},
                    ShortcutResult::Consume=>return,
                    ShortcutResult::Focus(key)=>{
                        let Some(surface)=state.known_window_surfaces.get(key).cloned() else {
                            state.keyboard_window_cycle.cancel_cycle();
                            return;
                        };
                        // Raising alone is scene damage, never fabricated
                        // GPU success. The KMS worker/receipt remains the
                        // only path to wl_surface.frame completion.
                        match state.dynamic_scene.raise(key) {
                            Ok(changed)=>{
                                let keyboard=state.seat.get_keyboard().expect("keyboard");
                                keyboard.set_focus(state,Some(surface.clone()),
                                    SERIAL_COUNTER.next_serial());
                                state.focused=Some(surface);
                                if changed {
                                    state.note_scene_change();
                                    if state.persistent_kms_mode ||
                                       state.continuous_frame_count<state.continuous_frame_target {
                                        if let Err(error)=state.submit_continuous_frame() {
                                            eprintln!("native Alt-Tab KMS submission failed: {error}");
                                        }
                                    }
                                }
                                println!("MELANIA_NATIVE_KEYBOARD_ALT_TAB_FOCUS={key}");
                                let _=std::io::stdout().flush();
                            },
                            Err(_)=>state.keyboard_window_cycle.cancel_cycle(),
                        }
                        return;
                    },
                }
            }
            if state.focused.is_none() {return;}
            let keyboard=state.seat.get_keyboard().expect("keyboard");
            keyboard.input::<(),_>(state,event.key_code(),event.state(),
                SERIAL_COUNTER.next_serial(),event.time_msec(),
                |_,_,_|FilterResult::Forward);
            state.delivered_keys+=1;
        }
        InputEvent::PointerMotion {event} => {
            if (state.focused.is_none() && !state.multi_window_mode) ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let point:Point<f64,Logical>=(
                (state.pointer_position.x+event.delta_x())
                    .clamp(0.0,if state.multi_window_mode {1279.0} else {639.0}),
                (state.pointer_position.y+event.delta_y())
                    .clamp(0.0,if state.multi_window_mode {799.0} else {479.0})
            ).into();
            state.pointer_position=point;
            state.apply_window_motion(point);
            if state.multi_window_mode {
                println!("MELANIA_NATIVE_WINDOW_POINTER_REL=({:.1},{:.1}) target={:?}",
                    point.x,point.y,state.multi_scene.hit_test(point.x,point.y));
                let _=std::io::stdout().flush();
            }
            let pointer=state.seat.get_pointer().expect("pointer");
            let target=state.pointer_target(point);
            pointer.motion(state,target,
                &MotionEvent{location:point,serial:SERIAL_COUNTER.next_serial(),
                             time:event.time_msec()});
            pointer.frame(state);
            state.delivered_motions+=1;
        }
        InputEvent::PointerMotionAbsolute {event} => {
            if (state.focused.is_none() && !state.multi_window_mode) ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let point=if state.multi_window_mode {
                event.position_transformed((1280,800).into())
            } else {
                event.position_transformed((640,480).into())
            };
            state.pointer_position=point;
            state.apply_window_motion(point);
            if state.multi_window_mode {
                println!("MELANIA_NATIVE_WINDOW_POINTER_ABS=({:.1},{:.1}) target={:?}",
                    point.x,point.y,state.multi_scene.hit_test(point.x,point.y));
                let _=std::io::stdout().flush();
            }
            let pointer=state.seat.get_pointer().expect("pointer");
            let target=state.pointer_target(point);
            pointer.motion(state,target,
                &MotionEvent{location:point,serial:SERIAL_COUNTER.next_serial(),
                             time:event.time_msec()});
            pointer.frame(state);
            state.delivered_motions+=1;
        }
        InputEvent::PointerAxis {event} => {
            // Forward actual libinput wheel/touchpad scroll to the Wayland
            // wl_pointer, never to the host X11/Wayland session. The Stage89
            // seat epoch check at the top of this handler remains mandatory.
            if state.lifecycle.input_registry_mut()
                .event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let point=state.pointer_position;
            let Some(target)=state.pointer_target(point) else {return;};
            let source=event.source();
            let mut frame=AxisFrame::new(event.time_msec()).source(source);
            let mut present=false;
            for axis in [Axis::Horizontal,Axis::Vertical] {
                let amount=event.amount(axis).filter(|v|v.is_finite() && v.abs()<=20000.0);
                let v120=bounded_v120(event.amount_v120(axis));
                // Older USB wheels may provide only discrete units; touchpad
                // gestures normally provide a continuous pixel delta.
                let value=amount.or_else(||v120.map(|steps|f64::from(steps)*15.0/120.0));
                if let Some(value)=value {
                    frame=frame.value(axis,value)
                        .relative_direction(axis,event.relative_direction(axis));
                    if source==AxisSource::Finger && value==0.0 {
                        frame=frame.stop(axis);
                    }
                    present=true;
                }
                if let Some(steps)=v120 {
                    frame=frame.v120(axis,steps);
                }
            }
            if !present {return;}
            let pointer=state.seat.get_pointer().expect("pointer");
            pointer.motion(state,Some(target),
                &MotionEvent{location:point,serial:SERIAL_COUNTER.next_serial(),
                             time:event.time_msec()});
            pointer.axis(state,frame);
            pointer.frame(state);
            state.delivered_scroll_frames=state.delivered_scroll_frames.saturating_add(1);
        }
        InputEvent::PointerButton {event} => {
            if (state.focused.is_none() && !state.multi_window_mode) ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let point=state.pointer_position;
            if state.multi_window_mode {
                let target_key=if state.dynamic_window_mode {
                    state.dynamic_scene.hit_test(point.x,point.y)
                } else {
                    state.multi_scene.hit_test(point.x,point.y)
                };
                println!("MELANIA_NATIVE_WINDOW_BUTTON=({:.1},{:.1}) target={:?} state={:?}",
                    point.x,point.y,target_key,event.state());
                let _=std::io::stdout().flush();
            }
            let pointer=state.seat.get_pointer().expect("pointer");
            if state.multi_window_mode {
                let target=state.pointer_target(point);
                pointer.motion(state,target.clone(),
                    &MotionEvent{location:point,
                        serial:SERIAL_COUNTER.next_serial(),time:event.time_msec()});
                // A click, unlike motion, raises a client and grants
                // keyboard focus to its actual wl_surface.
                if event.state()==ButtonState::Pressed {
                    if let Some((surface,_))=target {
                        let id=if state.dynamic_window_mode {
                            state.dynamic_scene.hit_test(point.x,point.y)
                        } else {
                            state.multi_scene.hit_test(point.x,point.y)
                        };
                        if let Some(id)=id {
                            if state.dynamic_window_mode {
                                if state.dynamic_scene.raise(id).unwrap_or(false) {
                                    state.note_scene_change();
                                    if (state.persistent_kms_mode || state.continuous_frame_count<state.continuous_frame_target) {
                                        if let Err(error)=state.submit_continuous_frame() {
                                            eprintln!("dynamic QEMU raise KMS rejected: {error}");
                                        }
                                    }
                                }
                            } else {
                                state.multi_scene.raise(id);
                            }
                            if event.button_code()==0x110 {
                                let geometry=if state.dynamic_window_mode {
                                    state.dynamic_scene.geometry(id)
                                        .map(|(x,y,w,h)|(x,y,w as i32,h as i32))
                                } else {
                                    state.multi_scene.position(id)
                                        .map(|(x,y)|(x,y,360,224))
                                };
                                if let Some((x,y,width,height))=geometry {
                                    let right=f64::from(x)+f64::from(width);
                                    let bottom=f64::from(y)+f64::from(height);
                                    let kind=if point.x>=right-16.0 &&
                                                point.y>=bottom-16.0 &&
                                                !(state.managed_xdg_mode && state.dynamic_window_mode) {
                                        Some(InteractionKind::ResizeBottomRight)
                                    } else if point.y>=f64::from(y) &&
                                              point.y<f64::from(y+26) &&
                                              !(state.managed_xdg_mode && state.dynamic_window_mode) {
                                        // For managed xdg clients, respect the
                                        // actual xdg_toplevel.move request
                                        // rather than imposing a fixed 26px
                                        // server-side titlebar grab.
                                        Some(InteractionKind::Move)
                                    } else {None};
                                    if let Some(kind)=kind {
                                        let original=WindowRect {x,y,width,height};
                                        if state.window_interaction.begin(
                                            id,original,point.x,point.y,kind
                                        ) {
                                            println!("MELANIA_NATIVE_WINDOW_INTERACTION_BEGIN={id} kind={kind:?}");
                                        }
                                    }
                                }
                            }
                            state.keyboard_window_cycle.cancel_cycle();
                            let keyboard=state.seat.get_keyboard().expect("keyboard");
                            keyboard.set_focus(state,Some(surface.clone()),
                                SERIAL_COUNTER.next_serial());
                            state.focused=Some(surface);
                            println!("MELANIA_NATIVE_WINDOW_POINTER_FOCUS={id}");
                            // The QEMU injector observes this event via
                            // the redirected guest serial log. Flush NOW,
                            // not after the next multi-second DRM ACK.
                            let _=std::io::stdout().flush();
                        }
                    }
                }
            }
            // Mint AFTER pointer.motion and keyboard focus changes: serials
            // must follow their actual Wayland event emission order.
            // The authorization token is the precise forwarded button serial.
            let button_serial=SERIAL_COUNTER.next_serial();
            if event.button_code()==0x110 {
                if event.state()==ButtonState::Pressed &&
                   state.managed_xdg_mode && state.dynamic_window_mode {
                    state.last_xdg_press=state.pointer_target(point)
                        .map(|(surface,_)|(surface,button_serial));
                } else {
                    state.last_xdg_press=None;
                }
            }
            pointer.button(state,&ButtonEvent{
                button:event.button_code(),state:event.state(),
                serial:button_serial,time:event.time_msec(),
            });
            pointer.frame(state);
            if state.multi_window_mode && event.button_code()==0x110 &&
               event.state()==ButtonState::Released {
                let was_resize=matches!(state.window_interaction.active_kind(),
                    Some(InteractionKind::ResizeBottomRight | InteractionKind::Resize(_)));
                if let Some(id)=state.window_interaction.end() {
                    if was_resize && state.managed_xdg_mode {
                        if let Some(surface)=state.known_window_surfaces.get(id) {
                            if let Some(top)=state.xdg_shell.toplevel_surfaces().iter()
                                .find(|top|top.wl_surface()==surface) {
                                // XDG requires a final configure with the
                                // Resizing state cleared after button release.
                                top.with_pending_state(|pending|{
                                    pending.states.unset(xdg_toplevel::State::Resizing);
                                });
                                top.send_pending_configure();
                            }
                        }
                    }
                    println!("MELANIA_NATIVE_WINDOW_INTERACTION_END={id}");
                }
            }
            state.delivered_buttons+=1;
        }
        _=>{}
    }
}

/// Keep the original libseat-owned descriptor until the DRM worker exits.
/// The seat's close() callback (not a bare close(2)) retires its device ID.
struct OwnedSeatCard {
    session:LibSeatSession,
    fd:Option<OwnedFd>,
}
impl OwnedSeatCard {
    fn release(&mut self)->Result<(),String> {
        if let Some(fd)=self.fd.take() {
            self.session.close(fd)
                .map_err(|error|format!("libseat failed to release DRM card: {error}"))
        } else {Ok(())}
    }
}
impl Drop for OwnedSeatCard {
    fn drop(&mut self) {
        if let Err(error)=self.release() {
            eprintln!("MELANIA_NATIVE_LIBSEAT_DRM_RELEASE_ERROR={error}");
        }
    }
}

pub fn run() -> Result<(),Box<dyn Error>> {
    // Main has already required explicit consent, QEMU DMI and dedicated VT.
    let (mut session,notifier)=LibSeatSession::new()?;
    let seat_name=session.seat();
    // Like a production Smithay backend, monitor DRM devices through the
    // SAME seat/event loop as libinput and libseat. This is read-only udev
    // discovery: only the isolated KMS experiment may open a DRM card.
    let udev=UdevBackend::new(&seat_name)?;
    let gpu_watch=NativeGpuWatch::from_discovered(
        udev.device_list().map(|(device_id,path)|
            (device_id as u64,path.to_path_buf()))
    );
    if gpu_watch.revoked() {
        return Err("native DRM device baseline contains conflicting IDs".into());
    }
    let mut context=Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
        session.clone().into()
    );
    context.udev_assign_seat(&seat_name)
        .map_err(|_|"Cannot bind libinput to QEMU libseat")?;
    let mut display=Display::<BridgeState>::new()?;
    let mut handle=display.handle();
    let output=Output::new("MELANIA-QEMU-1".into(),PhysicalProperties{
        size:(0,0).into(),subpixel:Subpixel::Unknown,
        make:"Melania".into(),model:"Virtual display (QEMU only)".into(),
    });
    // Protocol-advertised virtual output, not proof of a live KMS renderer.
    // Mode matches the VM-only 1280x800 scanout snapshot tested in Stage 26.
    let virtual_mode=Mode{size:(1280,800).into(),refresh:60_000};
    output.create_global::<BridgeState>(&handle);
    output.change_current_state(Some(virtual_mode),Some(Transform::Normal),
                                None,Some((0,0).into()));
    output.set_preferred(virtual_mode);
    let mut seats=SeatState::new();
    let mut seat=seats.new_wl_seat(&handle,"melania-native-seat0");
    seat.add_keyboard(Default::default(),250,40)?;
    seat.add_pointer();
    let mut state=BridgeState{
        compositor:CompositorState::new::<BridgeState>(&handle),
        shm:ShmState::new::<BridgeState>(&handle,vec![]),
        data_device:DataDeviceState::new::<BridgeState>(&handle),
        output_manager:OutputManagerState::new_with_xdg_output::<BridgeState>(&handle),
        output,
        pixel_probe:std::env::var("MELANIA_NATIVE_CLIENT_PIXELS")
            .as_deref()==Ok("YES_QEMU_VM"),pixel_ready:false,
        pixel_frame_count:0,
        pixel_frame_target:if std::env::var("MELANIA_NATIVE_REOPEN_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM") {6} else if std::env::var("MELANIA_NATIVE_CLOSE_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM") {5} else if std::env::var("MELANIA_NATIVE_DRAG_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM") {4} else if std::env::var("MELANIA_NATIVE_LIVE_STREAM")
            .as_deref()==Ok("YES_QEMU_VM") {3} else if std::env::var("MELANIA_NATIVE_CLIENT_UPDATES")
            .as_deref()==Ok("YES_QEMU_VM") {2} else {1},
        live_stream:std::env::var("MELANIA_NATIVE_LIVE_STREAM")
            .as_deref()==Ok("YES_QEMU_VM"),
        frame_callbacks_probe:std::env::var("MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS")
            .as_deref()==Ok("YES_QEMU_VM"),
        frame_callback_count:0,
        render_scheduler:RenderScheduler::default(),
        continuous_redraw:ContinuousRedraw::default(),continuous_pending:None,
        continuous_kms_mode:std::env::var("MELANIA_NATIVE_CONTINUOUS_KMS")
            .as_deref()==Ok("YES_QEMU_VM"),
        persistent_kms_mode:std::env::var("MELANIA_NATIVE_PERSISTENT_KMS")
            .as_deref()==Ok("YES_DISPOSABLE_QEMU"),
        persistent_shutdown_requested:false,
        continuous_worker:None,continuous_inflight:None,
        continuous_presentation_token:None,
        native_presentation_guard:NativePresentationGuard::default(),
        continuous_client_ledger:if std::env::var("MELANIA_NATIVE_DYNAMIC_SCENE")
            .as_deref()==Ok("YES_QEMU_VM") {
                ClientCommitLedger::with_capacity(16).expect("bounded QEMU receipts")
            } else {ClientCommitLedger::new()}, 
        continuous_frame_count:0,
        continuous_frame_target:if std::env::var("MELANIA_NATIVE_REAL_GTK_PIXEL_KMS")
            .as_deref()==Ok("YES_QEMU_VM") {
                if std::env::var("MELANIA_NATIVE_REAL_GTK_START_KMS")
                    .as_deref()==Ok("YES_QEMU_VM") {3}
                else if std::env::var("MELANIA_NATIVE_REAL_GTK_TWO_FRAME")
                    .as_deref()==Ok("YES_QEMU_VM") {2} else {1}
            } else if std::env::var("MELANIA_NATIVE_CONTINUOUS_REOPEN")
            .as_deref()==Ok("YES_QEMU_VM") {14} else if std::env::var("MELANIA_NATIVE_DYNAMIC_POINTER_PROOF")
            .as_deref()==Ok("YES_QEMU_VM") {13} else {
                // The historical scenario remains twelve frames by default.
                // Extended QEMU-only trials can exercise up to 64 actual
                // wl_shm -> Smithay -> KMS ACK cycles with the same receiver.
                native_frame_budget::frame_budget(
                    std::env::var("MELANIA_NATIVE_QEMU_FRAME_TARGET").ok().as_deref(),12
                )?
            },
        continuous_reopen_mode:std::env::var("MELANIA_NATIVE_CONTINUOUS_REOPEN")
            .as_deref()==Ok("YES_QEMU_VM"),
        continuous_close_frame_presented:false,
        continuous_reopen_frame_presented:false,
        continuous_two_window_presented:false,
        multi_window_mode:std::env::var("MELANIA_NATIVE_TWO_WINDOWS")
            .as_deref()==Ok("YES_QEMU_VM"),
        async_kms_mode:std::env::var("MELANIA_NATIVE_ASYNC_KMS")
            .as_deref()==Ok("YES_QEMU_VM"),
        drag_redraw_mode:std::env::var("MELANIA_NATIVE_DRAG_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM"),
        close_redraw_mode:std::env::var("MELANIA_NATIVE_CLOSE_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM"),close_frame_presented:false,
        reopen_redraw_mode:std::env::var("MELANIA_NATIVE_REOPEN_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM"),reopen_frame_presented:false,
        presented_layout_generation:0,
        drag_frame_presented:false,two_window_frame_presented:false,
        async_stream:None,async_pending:None,
        multi_scene:MultiWindowScene::default(),
        dynamic_window_mode:std::env::var("MELANIA_NATIVE_DYNAMIC_SCENE")
            .as_deref()==Ok("YES_QEMU_VM"),
        dynamic_scene:DynamicWindowScene::new(1280,800,[58,33,27,0])
            .expect("bounded disposable QEMU output"),
        dynamic_confirmed_frame:None,
        dynamic_xdg_resize_mode:std::env::var("MELANIA_NATIVE_XDG_RESIZE")
            .as_deref()==Ok("YES_QEMU_VM"),
        dynamic_pointer_proof_mode:std::env::var("MELANIA_NATIVE_DYNAMIC_POINTER_PROOF")
            .as_deref()==Ok("YES_QEMU_VM"),
        dynamic_toplevels:Vec::new(),
        managed_xdg_mode:std::env::var("MELANIA_NATIVE_MANAGED_XDG")
            .as_deref()==Ok("YES_QEMU_VM"),
        managed_windows:Vec::new(),
        managed_pending_sizes:Vec::new(),
        managed_pending_resize:Vec::new(),
        native_popup_roles:Vec::new(),
        window_interaction:WindowInteraction::default(),
        keyboard_window_cycle:KeyboardWindowCycle::default(),
        last_xdg_press:None,
        known_window_surfaces:if std::env::var("MELANIA_NATIVE_DYNAMIC_SCENE")
            .as_deref()==Ok("YES_QEMU_VM") {
                WindowSlots::with_capacity(16).expect("bounded QEMU window registry")
            } else {WindowSlots::default()}, 
        stream:None,
        xdg_shell:XdgShellState::new::<BridgeState>(&handle),
        layer_shell:WlrLayerShellState::new::<BridgeState>(&handle),
        layer_pixels_mode:std::env::var("MELANIA_NATIVE_LAYER_PIXELS")
            .as_deref()==Ok("YES_QEMU_VM"),
        gtk_layer_mode:std::env::var("MELANIA_NATIVE_GTK_MULTISURFACE")
            .as_deref()==Ok("YES_QEMU_VM"),
        real_gtk_pixel_kms_mode:std::env::var("MELANIA_NATIVE_REAL_GTK_PIXEL_KMS")
            .as_deref()==Ok("YES_QEMU_VM"),
        layer_surfaces:Vec::new(),
        xdg_toplevel_count:0,layer_surface_count:0,
        seats,seat,focused:None,pointer_position:(0.0,0.0).into(),
        lifecycle:NativeLifecycle::default(),gpu_watch,delivered_keys:0,
        qemu_start_super_down:None,delivered_buttons:0,delivered_motions:0,
        delivered_scroll_frames:0,
    };
    // Never pretend an inactive libseat owns the first KMS presentation.
    state.native_presentation_guard.arm_initial(session.is_active())
        .map_err(|_|"isolated native bridge requires initially active libseat")?;
    state.lifecycle.on_session(SessionEvent::ActivateSession);
    state.render_scheduler.set_seat_active(true);
    state.continuous_redraw.set_seat_active(true);
    context.dispatch()?;
    for event in &mut context {
        if let libinput::Event::Device(libinput::event::DeviceEvent::Added(added))=event {
            register(&mut state,libinput::event::EventTrait::device(&added));
        }
    }
    let backend=LibinputInputBackend::new(context.clone());
    let mut events=calloop::EventLoop::<BridgeState>::try_new()?;
    events.handle().insert_source(backend,|event,_,state|handle_input(state,event))?;
    events.handle().insert_source(notifier,move |event,_,state|{
        match event {
            SessionEvent::PauseSession=>{
                state.last_xdg_press=None;
                state.window_interaction.cancel();
                state.keyboard_window_cycle.reset();
                // Revoke *before* suspending input. Outstanding DRM worker
                // completions are no longer eligible for Wayland callbacks.
                state.native_presentation_guard.revoke();
                // A revoked callback is not enough: a pending transport job
                // could still ask the KMS presenter for a new page flip.
                // Interrupt the socket and join its worker BEFORE returning
                // from the libseat pause notification. The outer error path
                // then joins the DRM presenter, attempts CRTC restoration,
                // and releases the card through libseat.
                state.continuous_worker.take();
                state.continuous_inflight.take();
                state.continuous_presentation_token.take();
                state.continuous_pending=None;
                context.suspend();
            },
            SessionEvent::ActivateSession=>{
                if context.resume().is_err() {
                    state.native_presentation_guard.revoke();
                    // If libinput cannot resume, never re-use a socket or
                    // presentation ticket from the old DRM/VT generation.
                    state.continuous_worker.take();
                    state.continuous_inflight.take();
                    state.continuous_presentation_token.take();
                    state.lifecycle.on_session(SessionEvent::PauseSession);
                    state.render_scheduler.set_seat_active(false);
                    state.continuous_redraw.set_seat_active(false);
                    state.continuous_pending=None;
                    return;
                }
            }
        }
        let active=matches!(event,SessionEvent::ActivateSession);
        state.lifecycle.on_session(event);
        state.render_scheduler.set_seat_active(active);
        state.continuous_redraw.set_seat_active(active);
        if !active {
            state.continuous_pending=None;
            state.window_interaction.cancel();
            state.keyboard_window_cycle.reset();
        }
    })?;
    events.handle().insert_source(udev,|event,_,state|{
        // Do not trust an old framebuffer, CRTC route or GBM handle after
        // any real udev GPU topology change. This bridge cannot re-acquire
        // and rebuild those kernel resources, so it must abort safely.
        let change=match &event {
            UdevEvent::Added{device_id,path}=>DeviceChange::Added{
                id:*device_id as u64,path:path.to_path_buf()},
            UdevEvent::Changed{device_id}=>DeviceChange::Changed{
                id:*device_id as u64},
            UdevEvent::Removed{device_id}=>DeviceChange::Removed{
                id:*device_id as u64},
        };
        let reason=state.gpu_watch.observe(&change);
        state.lifecycle.on_udev(event);
        if let Some(reason)=reason {
            // Revoke BEFORE KMS transport teardown. The outer error branch
            // joins the presenter, attempts CRTC rollback and calls libseat
            // close() on the original device FD; no Wayland callback is sent.
            state.native_presentation_guard.revoke();
            state.continuous_worker.take();
            state.continuous_inflight.take();
            state.continuous_presentation_token.take();
            state.continuous_pending=None;
             state.dynamic_confirmed_frame=None;
            state.lifecycle.on_session(SessionEvent::PauseSession);
            state.render_scheduler.set_seat_active(false);
            state.continuous_redraw.set_seat_active(false);
            state.window_interaction.cancel();
            state.last_xdg_press=None;
            eprintln!("MELANIA_NATIVE_DRM_TOPOLOGY_REVOKED={reason:?}");
        }
    })?;

    let live_stream=std::env::var("MELANIA_NATIVE_LIVE_STREAM")
        .as_deref()==Ok("YES_QEMU_VM");
    if live_stream && !state.pixel_probe {
        return Err("live KMS stream needs explicit QEMU client-pixel probe".into());
    }
    if state.frame_callbacks_probe && !live_stream {
        return Err("frame presentation probe needs live VM Rust DRM stream".into());
    }
    if state.continuous_kms_mode &&
       (!live_stream || !state.frame_callbacks_probe || state.async_kms_mode) {
        return Err("continuous KMS requires a dedicated QEMU probe".into());
    }
    let continuous_two_windows=std::env::var("MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS")
        .as_deref()==Ok("YES_QEMU_VM");
    if state.continuous_kms_mode &&
       (state.multi_window_mode != continuous_two_windows) {
        return Err("continuous QEMU window layout flags must agree".into());
    }
    // Extended frame counts are for the existing plain QEMU continuous
    // renderer only: GTK three-frame captures, interactive pointer and
    // close/reopen proofs have fixed frame numbers and separate invariants.
    if std::env::var_os("MELANIA_NATIVE_QEMU_FRAME_TARGET").is_some() &&
       (!state.continuous_kms_mode || state.real_gtk_pixel_kms_mode ||
        state.continuous_reopen_mode || state.dynamic_pointer_proof_mode ||
        state.layer_pixels_mode || state.gtk_layer_mode) {
        return Err("extended native frames require plain QEMU continuous KMS".into());
    }
    if state.continuous_reopen_mode &&
       (!state.continuous_kms_mode || !state.multi_window_mode) {
        return Err("continuous reopen requires two real windows in disposable QEMU".into());
    }
    if state.dynamic_window_mode &&
       (!state.continuous_kms_mode || !state.multi_window_mode ||
        state.continuous_reopen_mode || !continuous_two_windows) {
        return Err("dynamic KMS requires dedicated two-window QEMU proof".into());
    }
    let gtk_role_protocol_only=std::env::var("MELANIA_NATIVE_SHELL_PROBE")
        .as_deref()==Ok("YES_QEMU_VM");
    if state.gtk_layer_mode && !state.layer_pixels_mode && !gtk_role_protocol_only {
        return Err("native GTK multisurface requires QEMU layer pixels or role-only smoke".into());
    }
    if state.layer_pixels_mode &&
       (!state.dynamic_window_mode || !state.continuous_kms_mode ||
        !state.multi_window_mode || !state.frame_callbacks_probe ||
        state.continuous_reopen_mode) {
        return Err("native layer pixels require isolated QEMU dynamic multi-client KMS".into());
    }
    if state.real_gtk_pixel_kms_mode &&
       (!state.gtk_layer_mode || !state.layer_pixels_mode ||
        !state.dynamic_window_mode || !state.continuous_kms_mode ||
        !state.live_stream || !state.frame_callbacks_probe ||
        !state.multi_window_mode || !state.pixel_probe || gtk_role_protocol_only ||
        state.continuous_reopen_mode || state.dynamic_pointer_proof_mode) {
        return Err("real GTK pixel KMS requires isolated QEMU single-frame pipeline".into());
    }
    if state.managed_xdg_mode && !state.dynamic_window_mode &&
       std::env::var("MELANIA_NATIVE_SHELL_PROBE").as_deref()!=Ok("YES_QEMU_VM") {
        return Err("managed xdg requires QEMU dynamic KMS or role-only shell probe".into());
    }
    if std::env::var("MELANIA_NATIVE_MANAGED_MINIMIZE_QMP")
        .as_deref()==Ok("YES_QEMU_VM") &&
       (!state.managed_xdg_mode || !state.dynamic_window_mode ||
        !state.continuous_kms_mode || !state.multi_window_mode ||
        !state.frame_callbacks_probe || state.continuous_frame_target!=14 ||
        state.continuous_reopen_mode || state.dynamic_pointer_proof_mode ||
        state.real_gtk_pixel_kms_mode || state.layer_pixels_mode) {
        return Err("managed minimize pixel proof requires 14-frame isolated QEMU xdg KMS".into());
    }
    if state.dynamic_xdg_resize_mode && !state.dynamic_window_mode {
        return Err("xdg resize configure is only permitted in disposable QEMU dynamic mode".into());
    }
    if state.dynamic_pointer_proof_mode &&
       (!state.dynamic_xdg_resize_mode || !state.dynamic_window_mode ||
        !state.continuous_kms_mode || state.continuous_reopen_mode) {
        return Err("dynamic pointer frame13 requires real xdg resize and opt-in VM KMS".into());
    }
    if state.multi_window_mode && !state.continuous_kms_mode &&
       (!live_stream || !state.frame_callbacks_probe ||
        state.pixel_frame_target!=if state.reopen_redraw_mode {6} else if state.close_redraw_mode {5} else if state.drag_redraw_mode {4} else {3}) {
        return Err("two real windows require bounded QEMU streaming and callbacks".into());
    }
    if state.async_kms_mode && !state.multi_window_mode {
        return Err("nonblocking KMS probe only enabled for QEMU two-window mode".into());
    }
    if state.drag_redraw_mode &&
       (!state.multi_window_mode || !state.async_kms_mode) {
        return Err("native layout redraw only supported with QEMU async two-window mode".into());
    }
    if state.close_redraw_mode &&
       (!state.drag_redraw_mode || !state.async_kms_mode || !state.live_stream) {
        return Err("close redraw needs QEMU two-window async KMS drag mode".into());
    }
    if state.reopen_redraw_mode &&
       (!state.close_redraw_mode || !state.multi_window_mode) {
        return Err("reopen redraw requires an isolated QEMU close-redraw test".into());
    }
    // SINGLE-PROCESS QEMU CHECKPOINT: use a private UnixStream pair to pass
    // the live Smithay wl_shm frames to the real Rust DRM/KMS presenter.
    // The presenter is compiled into this executable, not an external KMS
    // process. This is still a disposable VM proof, NOT a libseat-owned
    // production GPU, atomic rollback or login-manager session.
    let integrated_kms=std::env::var("MELANIA_NATIVE_IN_PROCESS_KMS")
        .as_deref()==Ok("YES_QEMU_VM");
    let persistent_kms=state.persistent_kms_mode;
    if persistent_kms && (
        !integrated_kms || state.continuous_reopen_mode ||
        state.dynamic_pointer_proof_mode || state.dynamic_xdg_resize_mode ||
        state.real_gtk_pixel_kms_mode || state.gtk_layer_mode ||
        state.layer_pixels_mode ||
        std::env::var_os("MELANIA_NATIVE_QEMU_FRAME_TARGET").is_some() ||
        std::env::var_os("MELANIA_QEMU_CONTINUOUS_FRAMES").is_some()
    ) {
        return Err("persistent KMS requires in-process QEMU without finite DRM/GTK proofs".into());
    }
    // Never register native login or run outside the existing disposable
    // VM + dedicated VT guard in main.rs/receiver.
    let shutdown_flag=Arc::new(AtomicBool::new(false));
    if persistent_kms {
        for signal in [SIGINT,SIGTERM] {
            signal_hook::flag::register(signal,shutdown_flag.clone())?;
        }
        println!("MELANIA_NATIVE_PERSISTENT_KMS_STARTED=true");
    }
    if integrated_kms && (
        !state.continuous_kms_mode || !state.live_stream ||
        !state.frame_callbacks_probe || state.async_kms_mode ||
        std::env::var("MELANIA_QEMU_CONTINUOUS_KMS").as_deref()
            !=Ok("YES_DISPOSABLE_QEMU") ||
        (!persistent_kms &&
         std::env::var("MELANIA_QEMU_CONTINUOUS_FRAMES")
             .ok().as_deref()!=Some(state.continuous_frame_target.to_string().as_str()))
    ) {
        return Err("in-process KMS requires matching QEMU frame targets and explicit safeguards".into());
    }
    // The DRM descriptor must correspond to the actual primary device
    // discovered by Smithay udev on THIS libseat seat. Never guess card0
    // or silently choose a GPU when the current compositor has no output
    // routing for multiple cards.
    let selected_kms_card=if integrated_kms {
        Some(state.gpu_watch.single_primary_card()
            .map_err(|error|format!("QEMU seat DRM selection rejected: {error:?}"))?
            .to_path_buf())
    } else {None};
    let mut integrated_thread=None;
    let mut integrated_drm:Option<OwnedSeatCard>=None;
    // Include every fallible initialization in the cleanup boundary. An error
    // while binding the Wayland socket, creating the DRM worker, or flushing
    // startup output must join the embedded presenter before libseat closes
    // the original DRM descriptor; not just errors in the dispatch loop.
    let run_result=(||->Result<(),Box<dyn Error>> {
    if integrated_kms {
        if !session.is_active() || !state.lifecycle.seat_active() {
            return Err("cannot acquire QEMU DRM card from an inactive libseat session".into());
        }
        // In contrast with the historical separate-process receiver, this
        // path obtains DRM from the SAME libseat seat as Smithay/libinput.
        // Only a clone of the FD crosses into the DRM thread; the original
        // FD remains registered until the verified rollback/join succeeds.
        let card_path=selected_kms_card.as_deref()
            .ok_or("integrated KMS requires a seat-discovered DRM primary device")?;
        let fd=session.open(
            card_path,
            OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
        )?;
        println!("MELANIA_NATIVE_SEAT_DRM_CARD={}",card_path.display());
        integrated_drm=Some(OwnedSeatCard{session:session.clone(),fd:Some(fd)});
        let presenter_fd=integrated_drm.as_ref().unwrap().fd.as_ref().unwrap().try_clone()?;
        let (sender,receiver)=UnixStream::pair()?;
        sender.set_read_timeout(Some(Duration::from_secs(8)))?;
        sender.set_write_timeout(Some(Duration::from_secs(7)))?;
        state.continuous_worker=Some(ContinuousKmsWorker::spawn(sender)?);
        integrated_thread=Some(std::thread::spawn(move || {
            qemu_continuous_receiver::run_embedded(
                receiver,std::fs::File::from(presenter_fd)
            ).map_err(|error|error.to_string())
        }));
        println!("MELANIA_NATIVE_LIBSEAT_DRM_ACQUIRED=true");
        println!("MELANIA_NATIVE_IN_PROCESS_KMS_WORKER_STARTED=true");
    }
    let frame_listener=if live_stream && !integrated_kms {
        let path=if state.continuous_kms_mode {
            "/run/melania-native-qemu/continuous-frame-stream.sock"
        } else {"/run/melania-native-qemu/frame-stream.sock"};
        let listener=UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;
        println!("MELANIA_NATIVE_FRAME_STREAM_SOCKET_READY=true");
        Some(listener)
    } else {None};
    // VM-only user-private control socket. Never advertise a production
    // display-manager session from this experimental seat bridge.
    let control_listener=if state.managed_xdg_mode {
        let runtime=PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")
            .ok_or("QEMU native control requires XDG_RUNTIME_DIR")?);
        let meta=fs::symlink_metadata(&runtime)?;
        if !meta.file_type().is_dir() || meta.permissions().mode()&0o077!=0 {
            return Err("QEMU native control runtime must be private (0700)".into());
        }
        let path=runtime.join(format!("melania-{}-control.sock",std::process::id()));
        let listener=UnixListener::bind(&path)?;
        let cleanup=NativeControlSocket(path.clone());
        fs::set_permissions(&path,fs::Permissions::from_mode(0o600))?;
        listener.set_nonblocking(true)?;
        println!("MELANIA_NATIVE_CONTROL_SOCKET_READY={}",path.display());
        Some((listener,cleanup))
    } else {None};
    let mut control_poller=NativeControlPoller::new();
    let listener=ListeningSocket::bind_auto("melania-native-seat",1..=8)?;
    let socket=listener.socket_name().ok_or("Missing native Wayland socket name")?;
    println!("MELANIA_NATIVE_WAYLAND_SOCKET_READY={}",socket.to_string_lossy());
    println!("MELANIA_NATIVE_WAYLAND_RENDER_READY=false");
    println!("MELANIA_NATIVE_WL_OUTPUT_ADVERTISED=1280x800");
    std::io::stdout().flush()?;
    // Only a disposable QEMU VM may select shell protocol handshake mode.
    // The parent process enforces the opt-in, DMI and Linux VT gates.
    let shell_probe=std::env::var("MELANIA_NATIVE_SHELL_PROBE")
        .as_deref()==Ok("YES_QEMU_VM");
    // A separate, strictly protocol-only smoke launches the real GTK4
    // process in QEMU. It must not be mistaken for a pixel/KMS proof.
    let real_gtk_smoke=std::env::var("MELANIA_NATIVE_REAL_GTK_SMOKE")
        .as_deref()==Ok("YES_QEMU_VM");
    if real_gtk_smoke && (!shell_probe || !state.gtk_layer_mode ||
                          state.layer_pixels_mode || state.continuous_kms_mode) {
        return Err("real GTK startup smoke requires QEMU five-layer role-only mode".into());
    }
    // A longer, explicitly bounded VM frame sequence needs more than the
    // historical twelve-frame watchdog. Keep the old 70-second default;
    // scale only this QEMU experiment (never a production login session).
    let continuous_deadline_secs=state.continuous_frame_target
        .saturating_mul(3).clamp(70,240);
    let deadline=Instant::now()+Duration::from_secs(
        if state.continuous_kms_mode {continuous_deadline_secs}
        else if state.reopen_redraw_mode {68}
        else if state.close_redraw_mode {55}
        else if state.drag_redraw_mode {45} else {27}
    );
    let mut both_shell_roles_seen_at=None;
    let mut real_gtk_seen_since=None;
    let mut pixel_ready_since=None;
    let mut continuous_ready_since=None;
    // Always unwind the integrated renderer before libseat drops its FD.
    // A client error, libseat pause or Wayland dispatch failure must NOT
    // bypass KMS rollback by returning directly from this function.
    while (persistent_kms && !shutdown_flag.load(Ordering::Relaxed)) ||
          (!persistent_kms && Instant::now()<deadline) {
        // Accept the Rust DRM consumer before processing client commits;
        // the socket is private to this disposable VM, not the host.
        if state.live_stream && state.stream.is_none() && state.async_stream.is_none() && state.continuous_worker.is_none() {
            if let Some(listener)=&frame_listener {
                match listener.accept() {
                    Ok((stream,_))=>{
                        stream.set_write_timeout(Some(Duration::from_secs(7)))?;
                        stream.set_read_timeout(Some(Duration::from_secs(8)))?;
                        if state.continuous_kms_mode {
                            state.continuous_worker=Some(ContinuousKmsWorker::spawn(stream)?);
                            println!("MELANIA_NATIVE_CONTINUOUS_KMS_WORKER_STARTED=true");
                        } else if state.async_kms_mode {
                            state.async_stream=Some(AsyncKmsWorker::spawn(stream));
                            println!("MELANIA_NATIVE_ASYNC_KMS_WORKER_STARTED=true");
                        } else {
                            state.stream=Some(stream);
                        }
                        println!("MELANIA_NATIVE_FRAME_STREAM_CONNECTED=true");
                        let _=std::io::stdout().flush();
                    }
                    Err(error) if error.kind()==std::io::ErrorKind::WouldBlock=>{},
                    Err(error)=>return Err(error.into()),
                }
            }
        }
        events.dispatch(Some(Duration::from_millis(12)),&mut state)?;
        // A real pause requires complete KMS/VT re-acquisition and rollback
        // before resuming. The Stage89 VM bridge deliberately has no such
        // operation: exit instead of accepting stale DRM ACKs or input.
        if state.native_presentation_guard.permanently_revoked() {
            return Err("native QEMU bridge stopped: libseat paused or DRM hotplug; safe revalidation unavailable".into());
        }
        while let Some(stream)=listener.accept()? {
            handle.insert_client(stream,Arc::new(BridgeClientState::default()))?;
        }
        display.dispatch_clients(&mut state)?;
        // The QEMU native IPC socket cannot stall the Wayland/libinput
        // thread while a peer waits to send EOF or read a large LIST reply.
        if let Some((listener,_cleanup))=&control_listener {
            control_poller.pump(listener,|request|state.control_command(request))?;
        }
        if state.async_kms_mode && state.async_stream.is_some() {
            state.poll_async_kms()?;
        }
        if state.continuous_kms_mode && state.continuous_worker.is_some() {
            state.poll_continuous_kms()?;
        }
        display.flush_clients()?;
        if state.continuous_kms_mode && !persistent_kms &&
           state.continuous_frame_count==state.continuous_frame_target {
            // Frame 13 in the optional tablet proof is compositor-owned
            // damage: NO new client commit and therefore NO callback.
            let compositor_only_minimize=std::env::var("MELANIA_NATIVE_MANAGED_MINIMIZE_QMP")
                .as_deref()==Ok("YES_QEMU_VM");
            let expected_callbacks=if state.dynamic_pointer_proof_mode ||
                compositor_only_minimize {12} else {state.continuous_frame_target};
            if !state.real_gtk_pixel_kms_mode &&
               u64::from(state.frame_callback_count)!=expected_callbacks {
                return Err("Wayland callbacks not completed after continuous KMS".into());
            }
            // The real GTK program is a single panel, despite sharing the
            // existing multi-client-capable KMS machinery with old scenarios.
            if state.multi_window_mode && !state.continuous_two_window_presented {
                // The original multi-window invariant remains mandatory
                // outside this one-image, real GTK panel QEMU experiment.
                if !state.real_gtk_pixel_kms_mode {
                    return Err("continuous KMS did not present both retained windows".into());
                }
            }
            if state.continuous_reopen_mode &&
               (!state.continuous_close_frame_presented ||
                !state.continuous_reopen_frame_presented) {
                return Err("continuous close and reopen never reached the real CRTC".into());
            }
            // Keep dispatching display.sync for the client after the last
            // callback, rather than shutting down its socket prematurely.
            let since=continuous_ready_since.get_or_insert_with(Instant::now);
            if since.elapsed()>=Duration::from_secs(2) {
                println!("MELANIA_NATIVE_CONTINUOUS_WAYLAND_KMS_FRAMES_PASS={}",
                         state.continuous_frame_target);
                if state.real_gtk_pixel_kms_mode {
                    // This branch is reachable only after a genuine DRM FB_ID
                    // receipt, not after wl_surface.configure alone.
                    let start_overlay=std::env::var("MELANIA_NATIVE_REAL_GTK_START_KMS")
                        .as_deref()==Ok("YES_QEMU_VM");
                    let expected_count=if start_overlay {2} else {1};
                    if state.dynamic_scene.window_count()!=expected_count ||
                       !state.layer_surfaces.iter().any(|(name,role)|{
                           name=="melania-panel" &&
                           state.known_window_surfaces.get(1)==Some(role.wl_surface())
                       }) ||
                       (start_overlay &&
                        !state.layer_surfaces.iter().any(|(name,role)|{
                            name=="melania-start" &&
                            state.known_window_surfaces.get(2)==Some(role.wl_surface())
                        })) {
                        return Err("real GTK panel/Start did not own KMS image".into());
                    }
                    println!("MELANIA_NATIVE_REAL_GTK_PANEL_KMS_ACK={}",
                             state.continuous_frame_target);
                    if state.continuous_frame_target>=2 {
                        println!("MELANIA_NATIVE_REAL_GTK_TWO_FRAME_KMS_PASS=true");
                    }
                    if start_overlay && state.continuous_frame_target==3 {
                        println!("MELANIA_NATIVE_REAL_GTK_START_OVERLAY_KMS_PASS=true");
                    }
                }
                if state.continuous_reopen_mode {
                    println!("MELANIA_NATIVE_CONTINUOUS_REOPEN_14_FRAMES_PASS=true");
                }
                if state.dynamic_pointer_proof_mode {
                    println!("MELANIA_NATIVE_DYNAMIC_POINTER_FRAME13_PASS=true");
                }
                if let Some(join)=integrated_thread.take() {
                    let outcome=join.join()
                        .map_err(|_|"in-process KMS presenter panicked during rollback")?;
                    outcome.map_err(|e|format!("in-process KMS presenter failed: {e}"))?;
                    // The kernel presenter has completed verified rollback.
                    // Stop and join the IPC transport before closing the
                    // original DRM device through libseat, even on success.
                    state.continuous_worker.take();
                    // The DRM worker has completed the final kernel
                    // verification and restored the old CRTC/connector.
                    // libseat must now explicitly close its original device.
                    let Some(mut owner)=integrated_drm.take() else {
                        return Err("libseat DRM ownership disappeared before cleanup".into());
                    };
                    owner.release().map_err(|error|format!("{error}"))?;
                    println!("MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true");
                    println!("MELANIA_NATIVE_IN_PROCESS_KMS_FRAMES_PASS={}",
                             state.continuous_frame_target);
                }
                println!("MELANIA_NATIVE_SESSION_READY=false");
                return Ok(());
            }
        }
        if state.pixel_probe && state.pixel_ready {
            let since=pixel_ready_since.get_or_insert_with(Instant::now);
            if since.elapsed()>=Duration::from_secs(
                if state.multi_window_mode {6} else {3}
            ) {
                println!("MELANIA_NATIVE_CLIENT_FRAME_SNAPSHOT_READY=true");
                println!("MELANIA_NATIVE_CLIENT_FRAME_COUNT={}",state.pixel_frame_count.min(3));
                if state.drag_redraw_mode {
                    // Destruction dirties the next scene, but cannot undo the
                    // fourth frame which KMS already acknowledged.
                    if state.pixel_frame_count<4 || !state.drag_frame_presented {
                        return Err("QEMU drag frame not acknowledged by KMS".into());
                    }
                    println!("MELANIA_NATIVE_DRAG_FRAME_VISIBLE=true");
                }
                if state.close_redraw_mode {
                    if state.pixel_frame_count<5 || !state.close_frame_presented {
                        return Err("QEMU close redraw not acknowledged by KMS".into());
                    }
                    println!("MELANIA_NATIVE_CLOSE_FRAME_VISIBLE=true");
                }
                if state.reopen_redraw_mode {
                    if state.pixel_frame_count!=6 || !state.reopen_frame_presented {
                        return Err("QEMU sixth KMS frame never presented both window surfaces".into());
                    }
                    println!("MELANIA_NATIVE_REOPEN_FRAME_VISIBLE=true");
                }
                if state.live_stream {
                    println!("MELANIA_NATIVE_KMS_STREAM_THREE_FRAMES=true");
                }
                if state.multi_window_mode {
                    if !state.two_window_frame_presented {
                        return Err("QEMU native scene never presented both client windows".into());
                    }
                    println!("MELANIA_NATIVE_TWO_REAL_WINDOWS_COMPOSITED=true");
                }
                if state.frame_callbacks_probe {
                    if state.frame_callback_count!=if state.reopen_redraw_mode {5} else if state.close_redraw_mode {4} else if state.drag_redraw_mode {3} else {state.pixel_frame_target as u32} {
                        return Err("not all Wayland frame callbacks completed after KMS".into());
                    }
                    println!("MELANIA_NATIVE_FRAME_CALLBACKS_PRESENTED_PASS=true");
                }
                println!("MELANIA_NATIVE_LIVE_KMS_RENDERING=false");
                println!("MELANIA_NATIVE_SESSION_READY=false");
                return Ok(());
            }
        }
        if real_gtk_smoke && state.layer_surfaces.iter()
            .any(|(name,_)|name=="melania-panel") {
            let since=real_gtk_seen_since.get_or_insert_with(Instant::now);
            // Give the actual GtkApplication time to finish constructing
            // widgets and process the Wayland configure; do NOT claim KMS.
            if since.elapsed()>=Duration::from_secs(5) {
                println!("MELANIA_NATIVE_REAL_GTK_PANEL_LAYER_CONFIGURED=true");
                println!("MELANIA_NATIVE_SESSION_READY=false");
                return Ok(());
            }
        }
        if shell_probe && !real_gtk_smoke && (if state.gtk_layer_mode {
            state.layer_surface_count>=gtk_layer_geometry::MAX_GTK_LAYER_SURFACES as u32
        } else {state.xdg_toplevel_count>0 && state.layer_surface_count>0}) {
            let since=both_shell_roles_seen_at.get_or_insert_with(Instant::now);
            // Let the real client receive, ack and finish its configure
            // roundtrips before closing the Wayland server.
            if since.elapsed()>=Duration::from_secs(3) {
                println!("MELANIA_NATIVE_XDG_TOPLEVEL_COUNT={}",state.xdg_toplevel_count);
                println!("MELANIA_NATIVE_LAYER_SURFACE_COUNT={}",state.layer_surface_count);
                println!("MELANIA_NATIVE_SHELL_PROTOCOLS_READY=true");
                println!("MELANIA_NATIVE_SHELL_RENDERED=false");
                println!("MELANIA_NATIVE_SESSION_READY=false");
                return Ok(());
            }
        }
        if !shell_probe && !state.pixel_probe && state.delivered_keys>=2 && state.delivered_buttons>=2 && state.focused.is_some() {
            println!("MELANIA_NATIVE_WAYLAND_KEYBOARD_EVENTS={}",state.delivered_keys);
            println!("MELANIA_NATIVE_WAYLAND_POINTER_BUTTON_EVENTS={}",state.delivered_buttons);
            println!("MELANIA_NATIVE_WAYLAND_INPUT_FORWARDED=true");
            println!("MELANIA_NATIVE_SESSION_READY=false");
            return Ok(());
        }
    }
    if persistent_kms && shutdown_flag.load(Ordering::Relaxed) {
        // Stop generating new KMS jobs, but allow an already-submitted frame
        // to finish its real CRTC/vblank receipt before closing the transport.
        // A timeout or transport failure exits via the ERROR cleanup branch.
        state.persistent_shutdown_requested=true;
        let drain_deadline=Instant::now()+Duration::from_secs(4);
        while state.continuous_inflight.is_some() &&
              Instant::now()<drain_deadline {
            state.poll_continuous_kms()?;
            // Only after shutdown was requested: never block the live event loop.
            std::thread::park_timeout(Duration::from_millis(10));
        }
        if state.continuous_inflight.is_some() {
            state.native_presentation_guard.revoke();
            return Err("persistent KMS failed to drain pending DRM flip before shutdown".into());
        }
        state.native_presentation_guard.revoke();
        // With no jobs in flight this shuts down the writer; the receiver
        // observes EOF at a complete frame boundary, rolls the CRTC back,
        // frees FB objects and returns before libseat closes the device.
        state.continuous_worker.take();
        if let Some(join)=integrated_thread.take() {
            join.join()
                .map_err(|_|"persistent DRM rollback thread panicked")?
                .map_err(|error|format!("persistent DRM rollback failed: {error}"))?;
        } else {
            return Err("persistent DRM missing owned presenter thread".into());
        }
        let Some(mut owner)=integrated_drm.take() else {
            return Err("persistent DRM lost its libseat owner".into());
        };
        owner.release().map_err(|error|format!("{error}"))?;
        println!("MELANIA_NATIVE_LIBSEAT_DRM_RELEASED=true");
        println!("MELANIA_NATIVE_PERSISTENT_KMS_SHUTDOWN_OK=true");
        println!("NATIVE_SESSION_READY=false");
        return Ok(());
    }
    Err(format!(
        "QEMU Wayland input bridge incomplete: focused={} keys={} buttons={}",
        state.focused.is_some(),state.delivered_keys,state.delivered_buttons
    ).into())
    })();
    if integrated_kms && run_result.is_err() {
        // Close the pending-job producer first, allowing the KMS receiver
        // to observe EOF. Its normal error/Drop path attempts CRTC rollback.
        // Never release the libseat descriptor before this thread ends.
        state.continuous_worker.take();
        if let Some(join)=integrated_thread.take() {
            match join.join() {
                Ok(Ok(()))=>println!("MELANIA_NATIVE_IN_PROCESS_KMS_EARLY_STOP=true"),
                Ok(Err(error))=>eprintln!("MELANIA_NATIVE_IN_PROCESS_KMS_ERROR={error}"),
                Err(_)=>eprintln!("MELANIA_NATIVE_IN_PROCESS_KMS_THREAD_PANICKED=true"),
            }
        }
        if let Some(mut owner)=integrated_drm.take() {
            if let Err(error)=owner.release() {
                eprintln!("MELANIA_NATIVE_LIBSEAT_DRM_RELEASE_ERROR={error}");
            }
        }
        println!("MELANIA_NATIVE_IN_PROCESS_KMS_ERROR_CLEANUP_ATTEMPTED=true");
    }
    run_result
}

#[cfg(test)]
mod managed_role_commit_tests {
    use super::accepts_managed_role_buffer;

    #[test]
    fn role_resize_refuses_stale_client_images_until_matching_wl_shm_arrives() {
        assert!(accepts_managed_role_buffer(None,(360,224)));
        assert!(accepts_managed_role_buffer(None,(400,240)));
        assert!(!accepts_managed_role_buffer(Some((1280,800)),(360,224)));
        assert!(!accepts_managed_role_buffer(Some((1280,800)),(1280,799)));
        assert!(accepts_managed_role_buffer(Some((1280,800)),(1280,800)));
        assert!(!accepts_managed_role_buffer(Some((360,224)),(1280,800)));
        assert!(accepts_managed_role_buffer(Some((360,224)),(360,224)));
    }
}

#[cfg(test)]
mod native_xdg_move_grant_tests {
    use super::take_xdg_pointer_press;
    use smithay::utils::SERIAL_COUNTER;
    #[test]
    fn exact_real_press_serial_surface_and_one_shot_are_required() {
        let serial=SERIAL_COUNTER.next_serial();
        let unrelated=SERIAL_COUNTER.next_serial();
        let mut grant=Some((42u64,serial));
        assert!(!take_xdg_pointer_press(&mut grant,&42u64,unrelated));
        assert!(grant.is_none());
        grant=Some((42,serial));
        assert!(!take_xdg_pointer_press(&mut grant,&43,serial));
        assert!(grant.is_none());
        grant=Some((42,serial));
        assert!(take_xdg_pointer_press(&mut grant,&42,serial));
        assert!(!take_xdg_pointer_press(&mut grant,&42,serial));
    }
}

#[cfg(test)]
mod native_scroll_stage90_tests {
    use super::bounded_v120;

    #[test]
    fn fractional_wheel_units_round_with_finite_bounds() {
        assert_eq!(bounded_v120(Some(120.0)),Some(120));
        assert_eq!(bounded_v120(Some(-120.0)),Some(-120));
        assert_eq!(bounded_v120(Some(59.8)),Some(60));
        assert_eq!(bounded_v120(None),None);
    }

    #[test]
    fn corrupt_scroll_payload_cannot_reach_wayland() {
        assert_eq!(bounded_v120(Some(f64::NAN)),None);
        assert_eq!(bounded_v120(Some(f64::INFINITY)),None);
        assert_eq!(bounded_v120(Some(1e30)),None);
    }
}
