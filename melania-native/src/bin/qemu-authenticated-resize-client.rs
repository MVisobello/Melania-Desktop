//! Disposable QEMU-only client: libinput press -> xdg_toplevel.resize ->
//! client-owned wl_shm -> verified Wayland frame callback after GPU/KMS.
use std::{error::Error,fs::OpenOptions,io::Write,os::fd::AsFd,time::{Duration,Instant}};
use wayland_client::{
    Connection,Dispatch,QueueHandle,WEnum,
    protocol::{wl_registry,wl_compositor,wl_surface,wl_shm,wl_shm_pool,
              wl_buffer,wl_callback,wl_seat,wl_pointer},
};
use wayland_protocols::xdg::shell::client::{xdg_wm_base,xdg_surface,xdg_toplevel};

const W:usize=360;
const H:usize=224;
type Result<T> = std::result::Result<T,Box<dyn Error>>;

#[derive(Default)]
struct State {
    compositor:Option<wl_compositor::WlCompositor>,
    shm:Option<wl_shm::WlShm>,
    wm:Option<xdg_wm_base::XdgWmBase>,
    seat:Option<wl_seat::WlSeat>,
    toplevel:Option<xdg_toplevel::XdgToplevel>,
    surface:Option<wl_surface::WlSurface>,
    initial_configure:bool,
    entered:bool,
    requested:bool,
    resized_size:Option<(usize,usize)>,
    resized_acked:bool,
    released:bool,
    final_configure_without_resizing:bool,
    final_acked:bool,
    buffer_released:bool,
    callbacks:u32,
}
impl Dispatch<wl_registry::WlRegistry,()> for State {
    fn event(s:&mut Self,r:&wl_registry::WlRegistry,event:wl_registry::Event,
             _:&(),_:&Connection,q:&QueueHandle<Self>) {
        if let wl_registry::Event::Global{name,interface,version}=event {
            match interface.as_str() {
                "wl_compositor"=>s.compositor=Some(r.bind(name,version.min(4),q,())),
                "wl_shm"=>s.shm=Some(r.bind(name,version.min(1),q,())),
                "wl_seat"=>s.seat=Some(r.bind(name,version.min(7),q,())),
                "xdg_wm_base"=>s.wm=Some(r.bind(name,version.min(3),q,())),
                _=>{}
            }
        }
    }
}
impl Dispatch<wl_seat::WlSeat,()> for State {
    fn event(_: &mut Self,_:&wl_seat::WlSeat,_:wl_seat::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_pointer::WlPointer,()> for State {
    fn event(s:&mut Self,_:&wl_pointer::WlPointer,event:wl_pointer::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        match event {
            wl_pointer::Event::Enter{surface,..}=>{
                s.entered=s.surface.as_ref()==Some(&surface);
            }
            wl_pointer::Event::Leave{..}=>s.entered=false,
            wl_pointer::Event::Button{serial,button,state,..} if button==0x110=>{
                if state==WEnum::Value(wl_pointer::ButtonState::Pressed) &&
                   s.entered && !s.requested && s.callbacks==1 {
                    if let (Some(top),Some(seat))=(&s.toplevel,&s.seat) {
                        top.resize(seat,serial,xdg_toplevel::ResizeEdge::TopLeft);
                        s.requested=true;
                        println!("MELANIA_NATIVE_AUTH_RESIZE_REQUEST_SENT=true");
                        let _=std::io::stdout().flush();
                    }
                }
                if state==WEnum::Value(wl_pointer::ButtonState::Released) && s.requested {
                    s.released=true;
                    println!("MELANIA_NATIVE_AUTH_RESIZE_REAL_RELEASE=true");
                    let _=std::io::stdout().flush();
                }
            }
            _=>{}
        }
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase,()> for State {
    fn event(_: &mut Self,wm:&xdg_wm_base::XdgWmBase,event:xdg_wm_base::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping{serial}=event {wm.pong(serial);}
    }
}
impl Dispatch<xdg_surface::XdgSurface,()> for State {
    fn event(s:&mut Self,role:&xdg_surface::XdgSurface,event:xdg_surface::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_surface::Event::Configure{serial}=event {
            role.ack_configure(serial);
            s.initial_configure=true;
            if s.requested && s.resized_size.is_some() {s.resized_acked=true;}
            if s.released && s.final_configure_without_resizing {s.final_acked=true;}
        }
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel,()> for State {
    fn event(s:&mut Self,_:&xdg_toplevel::XdgToplevel,event:xdg_toplevel::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_toplevel::Event::Configure{width,height,states}=event {
            let resizing=states.chunks_exact(4).any(|bytes|
                u32::from_ne_bytes([bytes[0],bytes[1],bytes[2],bytes[3]]) ==
                    xdg_toplevel::State::Resizing as u32);
            if s.requested && resizing && width>0 && height>0 {
                s.resized_size=Some((width as usize,height as usize));
                s.resized_acked=false;
            } else if s.released && !resizing &&
                      s.resized_size==Some((width as usize,height as usize)) {
                s.final_configure_without_resizing=true;
            }
        }
    }
}
impl Dispatch<wl_compositor::WlCompositor,()> for State {
    fn event(_: &mut Self,_:&wl_compositor::WlCompositor,_:wl_compositor::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_surface::WlSurface,()> for State {
    fn event(_: &mut Self,_:&wl_surface::WlSurface,_:wl_surface::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_shm::WlShm,()> for State {
    fn event(_: &mut Self,_:&wl_shm::WlShm,_:wl_shm::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_shm_pool::WlShmPool,()> for State {
    fn event(_: &mut Self,_:&wl_shm_pool::WlShmPool,_:wl_shm_pool::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_buffer::WlBuffer,()> for State {
    fn event(s:&mut Self,_:&wl_buffer::WlBuffer,event:wl_buffer::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let wl_buffer::Event::Release=event {s.buffer_released=true;}
    }
}
impl Dispatch<wl_callback::WlCallback,()> for State {
    fn event(s:&mut Self,_:&wl_callback::WlCallback,event:wl_callback::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let wl_callback::Event::Done{..}=event {s.callbacks=s.callbacks.saturating_add(1);}
    }
}
fn paint_frame(
    surface:&wl_surface::WlSurface,shm:&wl_shm::WlShm,
    queue:&mut wayland_client::EventQueue<State>,
    state:&mut State,size:(usize,usize),index:u32,
)->Result<()> {
    let (w,h)=size;
    if w<96 || h<72 || w>1280 || h>800 {
        return Err("invalid compositor-requested resize geometry".into());
    }
    let (r,g,b)=if index==1 {(180u8,44u8,62u8)} else {(46u8,175u8,219u8)};
    let mut pixels=vec![0u8;w*h*4];
    for pixel in pixels.chunks_exact_mut(4) {pixel.copy_from_slice(&[b,g,r,0]);}
    let path=format!("/run/melania-native-qemu/auth-resize-client-frame{index}.bgra");
    let mut file=OpenOptions::new().create_new(true).read(true).write(true).open(path)?;
    file.write_all(&pixels)?;
    file.sync_all()?;
    let q=queue.handle();
    let pool=shm.create_pool(file.as_fd(),pixels.len() as i32,&q,());
    let buffer=pool.create_buffer(0,w as i32,h as i32,(w*4) as i32,
                                  wl_shm::Format::Xrgb8888,&q,());
    state.buffer_released=false;
    let _callback=surface.frame(&q,());
    surface.attach(Some(&buffer),0,0);
    surface.damage_buffer(0,0,w as i32,h as i32);
    surface.commit();
    let deadline=Instant::now()+Duration::from_secs(32);
    while !state.buffer_released || state.callbacks<index {
        if Instant::now()>=deadline {
            return Err(format!("no real KMS callback/release for resize frame {index}").into());
        }
        queue.roundtrip(state)?;
    }
    if state.callbacks!=index {return Err("duplicate Wayland frame callback".into());}
    buffer.destroy();
    pool.destroy();
    println!("MELANIA_NATIVE_AUTH_RESIZE_FRAME{index}_ACK=true");
    std::io::stdout().flush()?;
    Ok(())
}
fn main()->Result<()> {
    if std::env::var("MELANIA_QEMU_AUTH_RESIZE_CLIENT").as_deref()!=Ok("YES_QEMU_VM") {
        return Err("authenticated resize client is disposable-QEMU only".into());
    }
    let connection=Connection::connect_to_env()?;
    let mut queue=connection.new_event_queue::<State>();
    let q=queue.handle();
    let _registry=connection.display().get_registry(&q,());
    let mut state=State::default();
    queue.roundtrip(&mut state)?;
    let compositor=state.compositor.take().ok_or("wl_compositor unavailable")?;
    let shm=state.shm.take().ok_or("wl_shm unavailable")?;
    let wm=state.wm.take().ok_or("xdg_wm_base unavailable")?;
    let seat=state.seat.clone().ok_or("real wl_seat unavailable")?;
    let _pointer=seat.get_pointer(&q,());
    let surface=compositor.create_surface(&q,());
    let role=wm.get_xdg_surface(&surface,&q,());
    let top=role.get_toplevel(&q,());
    top.set_title("Melania QEMU authenticated TopLeft resize".into());
    state.toplevel=Some(top.clone());
    state.surface=Some(surface.clone());
    surface.commit();
    for _ in 0..5 {
        queue.roundtrip(&mut state)?;
        if state.initial_configure {break;}
    }
    if !state.initial_configure {return Err("initial xdg configure missing".into());}
    paint_frame(&surface,&shm,&mut queue,&mut state,(W,H),1)?;
    let deadline=Instant::now()+Duration::from_secs(38);
    while !(state.resized_acked &&
             state.resized_size.is_some_and(|(w,h)|w<W && h<H)) {
        if Instant::now()>=deadline {
            return Err("real pointer-authenticated XDG configure absent".into());
        }
        queue.roundtrip(&mut state)?;
    }
    let size=state.resized_size.ok_or("resizing XDG toplevel geometry absent")?;
    if size.0>=W || size.1>=H {
        return Err("TopLeft resize did not shrink both dimensions".into());
    }
    println!("MELANIA_NATIVE_AUTH_RESIZE_CLIENT_CONFIGURE={}x{}",size.0,size.1);
    paint_frame(&surface,&shm,&mut queue,&mut state,size,2)?;
    let deadline=Instant::now()+Duration::from_secs(38);
    while !state.final_acked {
        if Instant::now()>=deadline {
            return Err("mouse release did not clear XDG Resizing state".into());
        }
        queue.roundtrip(&mut state)?;
    }
    println!("MELANIA_NATIVE_AUTH_RESIZE_FINAL_CONFIGURE_ACK=true");
    std::io::stdout().flush()?;
    top.destroy();
    role.destroy();
    surface.destroy();
    Ok(())
}
