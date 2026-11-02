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
        input::{InputEvent, KeyboardKeyEvent, PointerButtonEvent, PointerMotionEvent,
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
        compositor::{with_states,BufferAssignment,SurfaceAttributes,CompositorClientState,CompositorHandler,CompositorState},
        shm::{ShmState,ShmHandler,with_buffer_contents},
        output::{OutputHandler,OutputManagerState},
        shell::{xdg::{XdgShellState,XdgShellHandler,ToplevelSurface,PopupSurface,PositionerState},
                wlr_layer::{WlrLayerShellState,WlrLayerShellHandler,
                           LayerSurface as WlrLayerSurface,Layer as WlrLayer}},
    },
};
use melania_native::NativeLifecycle;
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

impl BufferHandler for BridgeState {
    fn buffer_destroyed(&mut self,_:&wl_buffer::WlBuffer) {}
}
impl CompositorHandler for BridgeState {
    fn compositor_state(&mut self)->&mut CompositorState {&mut self.compositor}
    fn client_compositor_state<'a>(&self,client:&'a Client)->&'a CompositorClientState {
        &client.get_data::<BridgeClientState>().expect("registered client").compositor
    }
    fn commit(&mut self,surface:&WlSurface) {
        if self.pixel_probe && self.pixel_frame_count<self.pixel_frame_target && self.xdg_toplevel_count>0 {
            let assignment=with_states(surface,|states|{
                states.cached_state.get::<SurfaceAttributes>().current().buffer.take()
            });
            if let Some(BufferAssignment::NewBuffer(buffer))=assignment {
                let frame=with_buffer_contents(&buffer,|ptr,len,metadata|{
                    native_client_frame::compose_from_shm(ptr,len,metadata)
                });
                match frame {
                    Ok(Ok(frame))=>{
                        let next=self.pixel_frame_count+1;
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
                                self.pixel_frame_count=next;
                                self.pixel_ready=self.pixel_frame_count==self.pixel_frame_target;
                                println!("MELANIA_NATIVE_CLIENT_SHM_IMPORTED=true");
                                println!("MELANIA_NATIVE_CLIENT_FRAME_INDEX={}",self.pixel_frame_count);
                                println!("MELANIA_NATIVE_CLIENT_FRAME_COMPOSED=true");
                                let _=std::io::stdout().flush();
                            },
                            Err(error)=>{
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
    fn toplevel_destroyed(&mut self,_:ToplevelSurface){}
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
            if state.focused.is_none() ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let point:Point<f64,Logical>=(
                (state.pointer_position.x+event.delta_x()).clamp(0.0,639.0),
                (state.pointer_position.y+event.delta_y()).clamp(0.0,479.0)
            ).into();
            state.pointer_position=point;
            let pointer=state.seat.get_pointer().expect("pointer");
            let target=state.focused.clone().map(|s|(s,(0.0,0.0).into()));
            pointer.motion(state,target,
                &MotionEvent{location:point,serial:SERIAL_COUNTER.next_serial(),
                             time:event.time_msec()});
            pointer.frame(state);
            state.delivered_motions+=1;
        }
        InputEvent::PointerMotionAbsolute {event} => {
            if state.focused.is_none() ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let point=event.position_transformed((640,480).into());
            state.pointer_position=point;
            let pointer=state.seat.get_pointer().expect("pointer");
            let target=state.focused.clone().map(|s|(s,(0.0,0.0).into()));
            pointer.motion(state,target,
                &MotionEvent{location:point,serial:SERIAL_COUNTER.next_serial(),
                             time:event.time_msec()});
            pointer.frame(state);
            state.delivered_motions+=1;
        }
        InputEvent::PointerButton {event} => {
            if state.focused.is_none() ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let pointer=state.seat.get_pointer().expect("pointer");
            pointer.button(state,&ButtonEvent{
                button:event.button_code(),state:event.state(),
                serial:SERIAL_COUNTER.next_serial(),time:event.time_msec(),
            });
            pointer.frame(state);
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
        pixel_frame_target:if std::env::var("MELANIA_NATIVE_LIVE_STREAM")
            .as_deref()==Ok("YES_QEMU_VM") {3} else if std::env::var("MELANIA_NATIVE_CLIENT_UPDATES")
            .as_deref()==Ok("YES_QEMU_VM") {2} else {1},
        live_stream:std::env::var("MELANIA_NATIVE_LIVE_STREAM")
            .as_deref()==Ok("YES_QEMU_VM"),stream:None,
        xdg_shell:XdgShellState::new::<BridgeState>(&handle),
        layer_shell:WlrLayerShellState::new::<BridgeState>(&handle),
        xdg_toplevel_count:0,layer_surface_count:0,
        seats,seat,focused:None,pointer_position:(0.0,0.0).into(),
        lifecycle:NativeLifecycle::default(),delivered_keys:0,
        delivered_buttons:0,delivered_motions:0,
    };
    if session.is_active() {state.lifecycle.on_session(SessionEvent::ActivateSession);}
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
                    return;
                }
            }
        }
        state.lifecycle.on_session(event);
    })?;

    let live_stream=std::env::var("MELANIA_NATIVE_LIVE_STREAM")
        .as_deref()==Ok("YES_QEMU_VM");
    if live_stream && !state.pixel_probe {
        return Err("live KMS stream needs explicit QEMU client-pixel probe".into());
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
    let deadline=Instant::now()+Duration::from_secs(27);
    let mut both_shell_roles_seen_at=None;
    let mut pixel_ready_since=None;
    while Instant::now()<deadline {
        // Accept the Rust DRM consumer before processing client commits;
        // the socket is private to this disposable VM, not the host.
        if state.live_stream && state.stream.is_none() {
            if let Some(listener)=&frame_listener {
                match listener.accept() {
                    Ok((stream,_))=>{
                        stream.set_write_timeout(Some(Duration::from_secs(7)))?;
                        stream.set_read_timeout(Some(Duration::from_secs(8)))?;
                        state.stream=Some(stream);
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
        display.flush_clients()?;
        if state.pixel_probe && state.pixel_ready {
            let since=pixel_ready_since.get_or_insert_with(Instant::now);
            if since.elapsed()>=Duration::from_secs(3) {
                println!("MELANIA_NATIVE_CLIENT_FRAME_SNAPSHOT_READY=true");
                println!("MELANIA_NATIVE_CLIENT_FRAME_COUNT={}",state.pixel_frame_count);
                if state.live_stream {
                    println!("MELANIA_NATIVE_KMS_STREAM_THREE_FRAMES=true");
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
