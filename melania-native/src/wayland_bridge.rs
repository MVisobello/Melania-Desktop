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
    io::{Read,Write},
    os::unix::net::{UnixListener,UnixStream},
    sync::Arc,
    time::{Duration,Instant},
};
use smithay::{
    backend::{
        input::{InputEvent, ButtonState, KeyboardKeyEvent, PointerButtonEvent, PointerMotionEvent,
                AbsolutePositionEvent, Event},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Session, Event as SessionEvent},
    },
    delegate_compositor, delegate_seat, delegate_xdg_shell, delegate_layer_shell, delegate_shm, delegate_output,
    input::{keyboard::FilterResult, pointer::{ButtonEvent, CursorImageStatus,
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
        output::{OutputHandler,OutputManagerState},
        shell::{xdg::{XdgShellState,XdgShellHandler,ToplevelSurface,PopupSurface,PositionerState},
                wlr_layer::{WlrLayerShellState,WlrLayerShellHandler,
                           LayerSurface as WlrLayerSurface,Layer as WlrLayer}},
    },
};
use melania_native::{NativeLifecycle,render_scheduler::{FrameTicket,RenderScheduler},multi_window_scene::MultiWindowScene,window_slots::WindowSlots,async_kms_stream::{AsyncKmsWorker,FrameJob},window_interaction::{WindowInteraction,WindowRect,InteractionKind}};
use smithay::backend::input::Device as InputDevice;
use wayland_protocols::xdg::shell::server::xdg_toplevel;
use crate::native_client_frame;

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
    output:Output,
    output_manager:OutputManagerState,
    pixel_probe:bool,
    pixel_ready:bool,
    live_stream:bool,
    frame_callbacks_probe:bool,
    frame_callback_count:u32,
    render_scheduler:RenderScheduler,
    multi_window_mode:bool,
    async_kms_mode:bool,
    drag_redraw_mode:bool,
    close_redraw_mode:bool,
    close_frame_presented:bool,
    presented_layout_generation:u64,
    // Store proof of a successful KMS presentation separately from the
    // current scene: a client may destroy its toplevel after the frame ACK.
    drag_frame_presented:bool,
    two_window_frame_presented:bool,
    async_stream:Option<AsyncKmsWorker>,
    async_pending:Option<PendingAsyncFrame>,
    multi_scene:MultiWindowScene,
    window_interaction:WindowInteraction,
    known_window_surfaces:WindowSlots<WlSurface>,
    stream:Option<UnixStream>,
    pixel_frame_count:u8,
    pixel_frame_target:u8,
    xdg_shell: XdgShellState,
    layer_shell: WlrLayerShellState,
    xdg_toplevel_count: u32,
    layer_surface_count: u32,
    seats: SeatState<Self>,
    seat: Seat<Self>,
    focused: Option<WlSurface>,
    pointer_position: Point<f64,Logical>,
    lifecycle: NativeLifecycle,
    delivered_keys: u32,
    delivered_buttons: u32,
    delivered_motions: u32,
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
    /// Resolve QEMU physical output coordinates to a retained client surface.
    fn pointer_target(&self,point:Point<f64,Logical>)
        ->Option<(WlSurface,Point<f64,Logical>)>
    {
        if !self.multi_window_mode {
            return self.focused.clone().map(|s|(s,(0.0,0.0).into()));
        }
        let key=self.multi_scene.hit_test(point.x,point.y)?;
        let surface=self.known_window_surfaces.get(key)?.clone();
        let (x,y)=self.multi_scene.position(key)?;
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
                if self.multi_scene.move_to(key,rect.x,rect.y).is_ok() {
                    println!("MELANIA_NATIVE_WINDOW_DRAG_MODEL={} x={} y={}",
                             key,rect.x,rect.y);
                    if let Err(error)=self.submit_drag_frame() {
                        eprintln!("QEMU drag KMS frame rejected: {error}");
                    }
                }
            },
            Some(InteractionKind::ResizeBottomRight)=>{
                // A real xdg.configure + client ACK must precede actual
                // resize/scanout. For now this is a non-destructive preview.
                println!("MELANIA_NATIVE_WINDOW_RESIZE_PREVIEW={} w={} h={}",
                         key,rect.width,rect.height);
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
        let worker=self.async_stream.as_ref()
            .ok_or("QEMU drag KMS worker unavailable")?;
        if worker.submit(FrameJob{ticket,ordinal:4,pixels:frame}).is_err() {
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
        let worker=self.async_stream.as_ref()
            .ok_or("QEMU close frame KMS worker unavailable")?;
        if worker.submit(FrameJob{ticket,ordinal:5,pixels:frame}).is_err() {
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
            self.render_scheduler.fail_present(pending.ticket);
            return Err(format!("async Rust KMS presentation failed: {error}").into());
        }
        if !self.render_scheduler.complete_present(pending.ticket) {
            return Err("async KMS ACK invalidated by seat pause or GPU change".into());
        }
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
                self.frame_callback_count=self.frame_callback_count.saturating_add(delivered);
                println!("MELANIA_NATIVE_FRAME_CALLBACK_DONE={}",pending.ordinal);
                println!("MELANIA_NATIVE_FRAME_CALLBACK_COUNT={}",self.frame_callback_count);
            }
            buffer.release();
        } else {
            match pending.ordinal {
                4 if self.drag_redraw_mode=>
                    println!("MELANIA_NATIVE_DRAG_FRAME_ACKED=4"),
                5 if self.close_redraw_mode=>
                    println!("MELANIA_NATIVE_CLOSE_FRAME_ACKED=5"),
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

impl BufferHandler for BridgeState {
    fn buffer_destroyed(&mut self,_:&wl_buffer::WlBuffer) {}
}
impl CompositorHandler for BridgeState {
    fn compositor_state(&mut self)->&mut CompositorState {&mut self.compositor}
    fn client_compositor_state<'a>(&self,client:&'a Client)->&'a CompositorClientState {
        &client.get_data::<BridgeClientState>().expect("registered client").compositor
    }
    fn commit(&mut self,surface:&WlSurface) {
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
                            let worker=match self.async_stream.as_ref() {
                                Some(worker)=>worker,
                                None=>{
                                    eprintln!("async KMS transport not connected");
                                    self.render_scheduler.fail_present(ticket);
                                    return;
                                }
                            };
                            if worker.submit(FrameJob{ticket,ordinal:next,pixels:frame}).is_err() {
                                eprintln!("async KMS transport full or disconnected");
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
        top.send_configure();
        println!("MELANIA_NATIVE_XDG_TOPLEVEL_CONFIGURE_SENT=true");
    }
    fn new_popup(&mut self,popup:PopupSurface,_:PositionerState) {
        let _=popup.send_configure();
    }
    fn move_request(&mut self,_:ToplevelSurface,_:wl_seat::WlSeat,_:Serial){}
    fn resize_request(&mut self,_:ToplevelSurface,_:wl_seat::WlSeat,
                      _:Serial,_:xdg_toplevel::ResizeEdge){}
    fn maximize_request(&mut self,top:ToplevelSurface) {top.send_configure();}
    fn unmaximize_request(&mut self,top:ToplevelSurface) {top.send_configure();}
    fn minimize_request(&mut self,_:ToplevelSurface){}
    fn toplevel_destroyed(&mut self,top:ToplevelSurface){
        let surface=top.wl_surface().clone();
        if let Some(key)=self.known_window_surfaces.remove(&surface) {
            // The retained pixel copy and stable slot belong to this client.
            // Never leave a closed window occupying the limited QEMU scene.
            if self.multi_scene.remove(key) && self.close_redraw_mode &&
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
            let _=std::io::stdout().flush();
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
    fn reposition_request(&mut self,popup:PopupSurface,_:PositionerState,token:u32){
        popup.send_repositioned(token);
    }
}
impl WlrLayerShellHandler for BridgeState {
    fn shell_state(&mut self)->&mut WlrLayerShellState {&mut self.layer_shell}
    fn new_layer_surface(&mut self,surface:WlrLayerSurface,
                         _:Option<WlOutput>,_:WlrLayer,_:String) {
        self.layer_surface_count=self.layer_surface_count.saturating_add(1);
        surface.send_configure();
        println!("MELANIA_NATIVE_LAYER_SURFACE_CONFIGURE_SENT=true");
    }
    fn layer_destroyed(&mut self,_:WlrLayerSurface){}
}
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
fn handle_input(state:&mut BridgeState,event:InputEvent<LibinputInputBackend>) {
    match event {
        InputEvent::DeviceAdded {device} =>register(state,device),
        InputEvent::DeviceRemoved {device} =>
            state.lifecycle.input_registry_mut().device_removed(&InputDevice::id(&device)),
        InputEvent::Keyboard {event} => {
            if state.focused.is_none() ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
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
        InputEvent::PointerButton {event} => {
            if (state.focused.is_none() && !state.multi_window_mode) ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let point=state.pointer_position;
            if state.multi_window_mode {
                println!("MELANIA_NATIVE_WINDOW_BUTTON=({:.1},{:.1}) target={:?} state={:?}",
                    point.x,point.y,state.multi_scene.hit_test(point.x,point.y),event.state());
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
                        if let Some(id)=state.multi_scene.hit_test(point.x,point.y) {
                            state.multi_scene.raise(id);
                            if event.button_code()==0x110 {
                                if let Some((x,y))=state.multi_scene.position(id) {
                                    let right=f64::from(x+360);
                                    let bottom=f64::from(y+224);
                                    let kind=if point.x>=right-16.0 &&
                                                point.y>=bottom-16.0 {
                                        Some(InteractionKind::ResizeBottomRight)
                                    } else if point.y>=f64::from(y) &&
                                              point.y<f64::from(y+26) {
                                        Some(InteractionKind::Move)
                                    } else {None};
                                    if let Some(kind)=kind {
                                        let original=WindowRect {
                                            x,y,width:360,height:224
                                        };
                                        if state.window_interaction.begin(
                                            id,original,point.x,point.y,kind
                                        ) {
                                            println!("MELANIA_NATIVE_WINDOW_INTERACTION_BEGIN={id} kind={kind:?}");
                                        }
                                    }
                                }
                            }
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
            pointer.button(state,&ButtonEvent{
                button:event.button_code(),state:event.state(),
                serial:SERIAL_COUNTER.next_serial(),time:event.time_msec(),
            });
            pointer.frame(state);
            if state.multi_window_mode && event.button_code()==0x110 &&
               event.state()==ButtonState::Released {
                if let Some(id)=state.window_interaction.end() {
                    println!("MELANIA_NATIVE_WINDOW_INTERACTION_END={id}");
                }
            }
            state.delivered_buttons+=1;
        }
        _=>{}
    }
}

pub fn run() -> Result<(),Box<dyn Error>> {
    // Main has already required explicit consent, QEMU DMI and dedicated VT.
    let (session,notifier)=LibSeatSession::new()?;
    let seat_name=session.seat();
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
        output_manager:OutputManagerState::new_with_xdg_output::<BridgeState>(&handle),
        output,
        pixel_probe:std::env::var("MELANIA_NATIVE_CLIENT_PIXELS")
            .as_deref()==Ok("YES_QEMU_VM"),pixel_ready:false,
        pixel_frame_count:0,
        pixel_frame_target:if std::env::var("MELANIA_NATIVE_CLOSE_REDRAW")
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
        multi_window_mode:std::env::var("MELANIA_NATIVE_TWO_WINDOWS")
            .as_deref()==Ok("YES_QEMU_VM"),
        async_kms_mode:std::env::var("MELANIA_NATIVE_ASYNC_KMS")
            .as_deref()==Ok("YES_QEMU_VM"),
        drag_redraw_mode:std::env::var("MELANIA_NATIVE_DRAG_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM"),
        close_redraw_mode:std::env::var("MELANIA_NATIVE_CLOSE_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM"),close_frame_presented:false,
        presented_layout_generation:0,
        drag_frame_presented:false,two_window_frame_presented:false,
        async_stream:None,async_pending:None,
        multi_scene:MultiWindowScene::default(),
        window_interaction:WindowInteraction::default(),
        known_window_surfaces:WindowSlots::default(),
        stream:None,
        xdg_shell:XdgShellState::new::<BridgeState>(&handle),
        layer_shell:WlrLayerShellState::new::<BridgeState>(&handle),
        xdg_toplevel_count:0,layer_surface_count:0,
        seats,seat,focused:None,pointer_position:(0.0,0.0).into(),
        lifecycle:NativeLifecycle::default(),delivered_keys:0,
        delivered_buttons:0,delivered_motions:0,
    };
    if session.is_active() {
        state.lifecycle.on_session(SessionEvent::ActivateSession);
        state.render_scheduler.set_seat_active(true);
    }
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
            SessionEvent::PauseSession=>context.suspend(),
            SessionEvent::ActivateSession=>{
                if context.resume().is_err() {
                    state.lifecycle.on_session(SessionEvent::PauseSession);
                    state.render_scheduler.set_seat_active(false);
                    return;
                }
            }
        }
        let active=matches!(event,SessionEvent::ActivateSession);
        state.lifecycle.on_session(event);
        state.render_scheduler.set_seat_active(active);
        if !active {state.window_interaction.cancel();}
    })?;

    let live_stream=std::env::var("MELANIA_NATIVE_LIVE_STREAM")
        .as_deref()==Ok("YES_QEMU_VM");
    if live_stream && !state.pixel_probe {
        return Err("live KMS stream needs explicit QEMU client-pixel probe".into());
    }
    if state.frame_callbacks_probe && !live_stream {
        return Err("frame presentation probe needs live VM Rust DRM stream".into());
    }
    if state.multi_window_mode &&
       (!live_stream || !state.frame_callbacks_probe ||
        state.pixel_frame_target!=if state.close_redraw_mode {5} else if state.drag_redraw_mode {4} else {3}) {
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
    let frame_listener=if live_stream {
        let path="/run/melania-native-qemu/frame-stream.sock";
        let listener=UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;
        println!("MELANIA_NATIVE_FRAME_STREAM_SOCKET_READY=true");
        Some(listener)
    } else {None};
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
    let deadline=Instant::now()+Duration::from_secs(
        if state.close_redraw_mode {55} else if state.drag_redraw_mode {45} else {27}
    );
    let mut both_shell_roles_seen_at=None;
    let mut pixel_ready_since=None;
    while Instant::now()<deadline {
        // Accept the Rust DRM consumer before processing client commits;
        // the socket is private to this disposable VM, not the host.
        if state.live_stream && state.stream.is_none() && state.async_stream.is_none() {
            if let Some(listener)=&frame_listener {
                match listener.accept() {
                    Ok((stream,_))=>{
                        stream.set_write_timeout(Some(Duration::from_secs(7)))?;
                        stream.set_read_timeout(Some(Duration::from_secs(8)))?;
                        if state.async_kms_mode {
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
        while let Some(stream)=listener.accept()? {
            handle.insert_client(stream,Arc::new(BridgeClientState::default()))?;
        }
        display.dispatch_clients(&mut state)?;
        if state.async_kms_mode && state.async_stream.is_some() {
            state.poll_async_kms()?;
        }
        display.flush_clients()?;
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
                    if state.pixel_frame_count!=5 || !state.close_frame_presented {
                        return Err("QEMU close redraw not acknowledged by KMS".into());
                    }
                    println!("MELANIA_NATIVE_CLOSE_FRAME_VISIBLE=true");
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
                    if state.frame_callback_count!=if state.drag_redraw_mode {3} else {state.pixel_frame_target as u32} {
                        return Err("not all Wayland frame callbacks completed after KMS".into());
                    }
                    println!("MELANIA_NATIVE_FRAME_CALLBACKS_PRESENTED_PASS=true");
                }
                println!("MELANIA_NATIVE_LIVE_KMS_RENDERING=false");
                println!("MELANIA_NATIVE_SESSION_READY=false");
                return Ok(());
            }
        }
        if shell_probe && state.xdg_toplevel_count>0 && state.layer_surface_count>0 {
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
    Err(format!(
        "QEMU Wayland input bridge incomplete: focused={} keys={} buttons={}",
        state.focused.is_some(),state.delivered_keys,state.delivered_buttons
    ).into())
}
