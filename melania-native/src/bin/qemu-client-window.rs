//! QEMU-only real wl_shm xdg_toplevel client (Stage 26).
//! Client pixels come from this executable. The native Rust bridge must
//! acquire them through Wayland, then compose them to a separate snapshot.
use std::{error::Error,fs::OpenOptions,io::Write,os::fd::AsFd};
use wayland_client::{
    Connection,Dispatch,QueueHandle,
    protocol::{wl_registry,wl_compositor,wl_surface,wl_shm,wl_shm_pool,wl_buffer,wl_output},
};
use wayland_protocols::xdg::shell::client::{
    xdg_wm_base,xdg_surface,xdg_toplevel,
};

const W:usize=360;
const H:usize=224;

#[derive(Default)]
struct Client {
    compositor:Option<wl_compositor::WlCompositor>,
    wm:Option<xdg_wm_base::XdgWmBase>,
    shm:Option<wl_shm::WlShm>,
    output:Option<wl_output::WlOutput>,
    output_mode_received:bool,
    xdg_acked:bool,
    buffer_released:bool,
}

impl Dispatch<wl_registry::WlRegistry,()> for Client {
    fn event(state:&mut Self,registry:&wl_registry::WlRegistry,
             event:wl_registry::Event,_:&(),_:&Connection,qh:&QueueHandle<Self>) {
        if let wl_registry::Event::Global{name,interface,version}=event {
            match interface.as_str() {
                "wl_compositor"=>state.compositor=Some(registry.bind(name,version.min(4),qh,())),
                "xdg_wm_base"=>state.wm=Some(registry.bind(name,version.min(3),qh,())),
                "wl_shm"=>state.shm=Some(registry.bind(name,version.min(1),qh,())),
                "wl_output"=>state.output=Some(registry.bind(name,version.min(3),qh,())),
                _=>{}
            }
        }
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase,()> for Client {
    fn event(_: &mut Self,wm:&xdg_wm_base::XdgWmBase,
             event:xdg_wm_base::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping{serial}=event {wm.pong(serial);}
    }
}
impl Dispatch<xdg_surface::XdgSurface,()> for Client {
    fn event(state:&mut Self,surface:&xdg_surface::XdgSurface,
             event:xdg_surface::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_surface::Event::Configure{serial}=event {
            surface.ack_configure(serial);
            state.xdg_acked=true;
        }
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel,()> for Client {
    fn event(_: &mut Self,_:&xdg_toplevel::XdgToplevel,_:xdg_toplevel::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_compositor::WlCompositor,()> for Client {
    fn event(_: &mut Self,_:&wl_compositor::WlCompositor,_:wl_compositor::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_surface::WlSurface,()> for Client {
    fn event(_: &mut Self,_:&wl_surface::WlSurface,_:wl_surface::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_output::WlOutput,()> for Client {
    fn event(state:&mut Self,_:&wl_output::WlOutput,event:wl_output::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let wl_output::Event::Mode{width,height,refresh,..}=event {
            if width==1280 && height==800 && refresh>0 {
                state.output_mode_received=true;
            }
        }
    }
}
impl Dispatch<wl_shm::WlShm,()> for Client {
    fn event(_: &mut Self,_:&wl_shm::WlShm,_:wl_shm::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_shm_pool::WlShmPool,()> for Client {
    fn event(_: &mut Self,_:&wl_shm_pool::WlShmPool,_:wl_shm_pool::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_buffer::WlBuffer,()> for Client {
    fn event(state: &mut Self,_:&wl_buffer::WlBuffer,event:wl_buffer::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let wl_buffer::Event::Release=event {state.buffer_released=true;}
    }
}

fn main()->Result<(),Box<dyn Error>> {
    let connection=Connection::connect_to_env()?;
    let mut queue=connection.new_event_queue::<Client>();
    let qh=queue.handle();
    let _registry=connection.display().get_registry(&qh,());
    let mut state=Client::default();
    queue.roundtrip(&mut state)?;
    queue.roundtrip(&mut state)?;
    if state.output.is_none() || !state.output_mode_received {
        return Err("native wl_output 1280x800 mode missing on real client".into());
    }
    println!("MELANIA_NATIVE_CLIENT_WL_OUTPUT_MODE=1280x800");
    let compositor=state.compositor.take().ok_or("wl_compositor absent")?;
    let wm=state.wm.take().ok_or("xdg_wm_base absent")?;
    let shm=state.shm.take().ok_or("wl_shm absent")?;
    let surface=compositor.create_surface(&qh,());
    let xdg=wm.get_xdg_surface(&surface,&qh,());
    let toplevel=xdg.get_toplevel(&qh,());
    toplevel.set_title("Melania QEMU real-client window".into());
    surface.commit();
    for _ in 0..4 {
        queue.roundtrip(&mut state)?;
        if state.xdg_acked {break;}
    }
    if !state.xdg_acked {return Err("server failed to configure xdg_toplevel".into());}
    let mut pixels=vec![0u8;W*H*4];
    for (index,p) in pixels.chunks_exact_mut(4).enumerate() {
        let x=index%W;
        // Distinctive client-owned orange and teal halves. Rust compositor
        // must not synthesize either color, only copy these wl_shm pixels.
        let (r,g,b)=if x<W/2 {(245u8,108u8,34u8)} else {(16u8,205u8,155u8)};
        p.copy_from_slice(&[b,g,r,0]);
    }
    let mut file=OpenOptions::new().create_new(true).read(true).write(true)
        .open("/run/melania-native-qemu/client-window-shm.bin")?;
    file.write_all(&pixels)?;
    file.sync_all()?;
    let pool=shm.create_pool(file.as_fd(),pixels.len() as i32,&qh,());
    let buffer=pool.create_buffer(
        0,W as i32,H as i32,(W*4) as i32,wl_shm::Format::Xrgb8888,&qh,()
    );
    surface.attach(Some(&buffer),0,0);
    surface.damage_buffer(0,0,W as i32,H as i32);
    surface.commit();
    for _ in 0..5 {
        queue.roundtrip(&mut state)?;
        if state.buffer_released {break;}
    }
    if !state.buffer_released {
        return Err("compositor did not copy and release actual wl_shm pixels".into());
    }
    println!("MELANIA_NATIVE_CLIENT_WL_SHM_RELEASED=true");
    println!("MELANIA_NATIVE_CLIENT_XDG_WINDOW_DRAWN=true");
    std::io::stdout().flush()?;
    buffer.destroy();
    pool.destroy();
    toplevel.destroy();
    xdg.destroy();
    surface.destroy();
    wm.destroy();
    drop(shm);
    drop(state.output.take());
    connection.flush()?;
    Ok(())
}
