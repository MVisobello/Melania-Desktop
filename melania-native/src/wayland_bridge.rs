//! QEMU-only native Wayland seat/keyboard/pointer bridge.
 //!
 //! This is a real Smithay wl_compositor + wl_seat endpoint receiving libinput
 //! from libseat in a dedicated VM console, NOT a full graphical compositor.
 //! A client can create a wl_surface and receive keyboard/pointer events, but
 //! xdg-shell, wl_output, KMS scanout of client buffers and login remain OFF.
use std::{error::Error, io::Write, sync::Arc, time::{Duration,Instant}};
use smithay::{
    backend::{
        input::{InputEvent, KeyboardKeyEvent, PointerButtonEvent, PointerMotionEvent,
                AbsolutePositionEvent, Event},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Session, Event as SessionEvent},
    },
    delegate_compositor, delegate_seat,
    input::{keyboard::FilterResult, pointer::{ButtonEvent, CursorImageStatus,
           MotionEvent}, Seat, SeatHandler, SeatState},
    reexports::{
        input::{self as libinput, DeviceCapability, Libinput},
        wayland_server::{
            backend::{ClientData,ClientId,DisconnectReason},
            protocol::{wl_buffer,wl_surface::WlSurface},
            Client, Display, ListeningSocket,
        },
    },
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::{
        buffer::BufferHandler,
        compositor::{CompositorClientState,CompositorHandler,CompositorState},
    },
};
use crate::{NativeLifecycle, DeviceChange};
use smithay::backend::input::Device as InputDevice;

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
impl SeatHandler for BridgeState {
    type KeyboardFocus=WlSurface;
    type PointerFocus=WlSurface;
    type TouchFocus=WlSurface;
    fn seat_state(&mut self)->&mut SeatState<Self> {&mut self.seats}
    fn focus_changed(&mut self,_:&Seat<Self>,_:Option<&WlSurface>){}
    fn cursor_image(&mut self,_:&Seat<Self>,_:CursorImageStatus){}
}
delegate_compositor!(BridgeState);
delegate_seat!(BridgeState);

fn register(state:&mut BridgeState,device:libinput::Device) {
    let caps=crate::input_registry::InputCapabilities{
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
    let handle=display.handle();
    let mut seats=SeatState::new();
    let mut seat=seats.new_wl_seat(&handle,"melania-native-seat0");
    seat.add_keyboard(Default::default(),250,40)?;
    seat.add_pointer();
    let mut state=BridgeState{
        compositor:CompositorState::new::<BridgeState>(&handle),
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

    let listener=ListeningSocket::bind_auto("melania-native-seat",1..=8)?;
    let socket=listener.socket_name().ok_or("Missing native Wayland socket name")?;
    println!("MELANIA_NATIVE_WAYLAND_SOCKET_READY={}",socket.to_string_lossy());
    println!("MELANIA_NATIVE_WAYLAND_RENDER_READY=false");
    std::io::stdout().flush()?;
    let deadline=Instant::now()+Duration::from_secs(27);
    while Instant::now()<deadline {
        events.dispatch(Some(Duration::from_millis(12)),&mut state)?;
        while let Some(stream)=listener.accept()? {
            handle.insert_client(stream,Arc::new(BridgeClientState::default()))?;
        }
        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;
        if state.delivered_keys>=2 && state.delivered_buttons>=2 && state.focused.is_some() {
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
