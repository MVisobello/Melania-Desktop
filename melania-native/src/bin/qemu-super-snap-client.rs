//! Disposable QEMU-only wl_shm -> real USB Super+Arrow -> XDG ACK -> KMS client.
//! Never authorizes native login or uses any synthetic Wayland configure.
use std::{error::Error,fs::OpenOptions,io::Write,os::fd::AsFd,time::{Duration,Instant}};
use wayland_client::{Connection,Dispatch,QueueHandle,WEnum,
    protocol::{wl_registry,wl_compositor,wl_surface,wl_shm,wl_shm_pool,
              wl_buffer,wl_callback,wl_seat,wl_keyboard}};
use wayland_protocols::xdg::shell::client::{xdg_wm_base,xdg_surface,xdg_toplevel};

type Result<T>=std::result::Result<T,Box<dyn Error>>;
const ORIGINAL:(usize,usize)=(360,224);
const HALF:(usize,usize)=(640,800);
const F8_EVDEV:u32=66;

#[derive(Default)]
struct State {
    compositor:Option<wl_compositor::WlCompositor>,
    shm:Option<wl_shm::WlShm>,
    wm:Option<xdg_wm_base::XdgWmBase>,
    seat:Option<wl_seat::WlSeat>,
    configured:bool,
    snapped:bool,
    restored:bool,
    size:Option<(usize,usize)>,
    callbacks:u32,
    released:bool,
    finish_key:bool,
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
             _:&(),_:&Connection,_:&QueueHandle<Self>) {}
}
impl Dispatch<wl_keyboard::WlKeyboard,()> for State {
    fn event(s:&mut Self,_:&wl_keyboard::WlKeyboard,event:wl_keyboard::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let wl_keyboard::Event::Key{key,state,..}=event {
            if key==F8_EVDEV && state==WEnum::Value(wl_keyboard::KeyState::Pressed) &&
               s.callbacks==3 {
                s.finish_key=true;
                println!("MELANIA_NATIVE_SNAP_CLIENT_HOST_RELEASE_ACK=true");
                let _=std::io::stdout().flush();
            }
        }
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase,()> for State {
    fn event(_: &mut Self,wm:&xdg_wm_base::XdgWmBase,
             event:xdg_wm_base::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping{serial}=event {wm.pong(serial);}
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel,()> for State {
    fn event(s:&mut Self,_:&xdg_toplevel::XdgToplevel,
             event:xdg_toplevel::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_toplevel::Event::Configure{width,height,..}=event {
            if width>0 && height>0 {
                s.size=Some((width as usize,height as usize));
            }
        }
    }
}
impl Dispatch<xdg_surface::XdgSurface,()> for State {
    fn event(s:&mut Self,role:&xdg_surface::XdgSurface,
             event:xdg_surface::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_surface::Event::Configure{serial}=event {
            role.ack_configure(serial);
            s.configured=true;
            if s.size==Some(HALF) {
                s.snapped=true;
                println!("MELANIA_NATIVE_SNAP_CLIENT_XDG_HALF_ACK=true");
            } else if s.snapped && s.size==Some(ORIGINAL) {
                s.restored=true;
                println!("MELANIA_NATIVE_SNAP_CLIENT_XDG_RESTORE_ACK=true");
            }
            let _=std::io::stdout().flush();
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
        if let wl_buffer::Event::Release=event {s.released=true;}
    }
}
impl Dispatch<wl_callback::WlCallback,()> for State {
    fn event(s:&mut Self,_:&wl_callback::WlCallback,event:wl_callback::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let wl_callback::Event::Done{..}=event {
            s.callbacks=s.callbacks.saturating_add(1);
        }
    }
}
fn paint(surface:&wl_surface::WlSurface,shm:&wl_shm::WlShm,
         queue:&mut wayland_client::EventQueue<State>,state:&mut State,
         size:(usize,usize),index:u32)->Result<()> {
    let (w,h)=size;
    if w<96 || h<72 || w>1280 || h>800 {
        return Err("unexpected XDG configured dimensions".into());
    }
    let color=match index {
        1=>[62u8,44u8,180u8,0],
        2=>[219u8,175u8,46u8,0],
        3=>[94u8,201u8,34u8,0],
        _=>return Err("unbounded frame".into())
    };
    let mut pixels=vec![0u8;w*h*4];
    for pixel in pixels.chunks_exact_mut(4) {pixel.copy_from_slice(&color);}
    let path=format!("/run/melania-native-qemu/snap-client-{}-{index}.bgra",
                     std::process::id());
    let mut file=OpenOptions::new().create_new(true).read(true).write(true)
        .open(&path)?;
    file.write_all(&pixels)?;
    file.sync_all()?;
    let q=queue.handle();
    let pool=shm.create_pool(file.as_fd(),pixels.len() as i32,&q,());
    std::fs::remove_file(&path)?;
    let buffer=pool.create_buffer(0,w as i32,h as i32,(w*4) as i32,
                                  wl_shm::Format::Xrgb8888,&q,());
    state.released=false;
    let _callback=surface.frame(&q,());
    surface.attach(Some(&buffer),0,0);
    surface.damage_buffer(0,0,w as i32,h as i32);
    surface.commit();
    let deadline=Instant::now()+Duration::from_secs(35);
    while !state.released || state.callbacks<index {
        if Instant::now()>=deadline {
            return Err("kernel-acknowledged Wayland frame callback/release missing".into());
        }
        queue.roundtrip(state)?;
    }
    if state.callbacks!=index {return Err("duplicate frame callback".into());}
    buffer.destroy();
    pool.destroy();
    println!("MELANIA_NATIVE_SNAP_CLIENT_FRAME{index}_ACK=true");
    std::io::stdout().flush()?;
    Ok(())
}
fn main()->Result<()> {
    if std::env::var("MELANIA_QEMU_SUPER_SNAP_CLIENT").as_deref()!=Ok("YES_QEMU_VM") {
        return Err("Super Snap client requires disposable QEMU opt-in".into());
    }
    let connection=Connection::connect_to_env()?;
    let mut queue=connection.new_event_queue::<State>();
    let q=queue.handle();
    let _registry=connection.display().get_registry(&q,());
    let mut state=State::default();
    queue.roundtrip(&mut state)?;
    let compositor=state.compositor.take().ok_or("wl_compositor missing")?;
    let shm=state.shm.take().ok_or("wl_shm missing")?;
    let wm=state.wm.take().ok_or("xdg_wm_base missing")?;
    let seat=state.seat.take().ok_or("wl_seat missing")?;
    let _keyboard=seat.get_keyboard(&q,());
    let surface=compositor.create_surface(&q,());
    let role=wm.get_xdg_surface(&surface,&q,());
    let top=role.get_toplevel(&q,());
    top.set_title("Melania QEMU real USB Super Snap".into());
    surface.commit();
    let initial_deadline=Instant::now()+Duration::from_secs(15);
    while !state.configured {
        if Instant::now()>=initial_deadline {return Err("initial xdg configure missing".into());}
        queue.roundtrip(&mut state)?;
    }
    paint(&surface,&shm,&mut queue,&mut state,ORIGINAL,1)?;
    let deadline=Instant::now()+Duration::from_secs(40);
    while !state.snapped {
        if Instant::now()>=deadline {return Err("Super+Left xdg configure never ACKed".into());}
        queue.roundtrip(&mut state)?;
    }
    if state.size!=Some(HALF) {return Err("half XDG size changed before commit".into());}
    paint(&surface,&shm,&mut queue,&mut state,HALF,2)?;
    let deadline=Instant::now()+Duration::from_secs(40);
    while !state.restored {
        if Instant::now()>=deadline {return Err("Super+Down restore never ACKed".into());}
        queue.roundtrip(&mut state)?;
    }
    if state.size!=Some(ORIGINAL) {return Err("restore XDG size invalid".into());}
    paint(&surface,&shm,&mut queue,&mut state,ORIGINAL,3)?;
    let deadline=Instant::now()+Duration::from_secs(35);
    while !state.finish_key {
        if Instant::now()>=deadline {return Err("physical QMP screenshot release key missing".into());}
        queue.roundtrip(&mut state)?;
    }
    println!("MELANIA_NATIVE_SNAP_CLIENT_ALL_THREE_REAL_FRAMES_PASS=true");
    std::io::stdout().flush()?;
    top.destroy();
    role.destroy();
    surface.destroy();
    Ok(())
}
