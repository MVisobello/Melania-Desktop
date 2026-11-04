//! Disposable QEMU proof: one real xdg_toplevel AND a real wlr-layer-shell
//! taskbar send 12 alternating, owned wl_shm images through Smithay/DRM.
use std::{error::Error,fs::OpenOptions,io::Write,os::fd::AsFd,thread,time::Duration};
use wayland_client::{Connection,Dispatch,QueueHandle,
    protocol::{wl_registry,wl_compositor,wl_surface,wl_shm,wl_shm_pool,
               wl_buffer,wl_output,wl_callback}};
use wayland_protocols::xdg::shell::client::{xdg_wm_base,xdg_surface,xdg_toplevel};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,zwlr_layer_surface_v1};

#[derive(Default)]
struct Client {
    compositor:Option<wl_compositor::WlCompositor>,
    wm:Option<xdg_wm_base::XdgWmBase>,
    shm:Option<wl_shm::WlShm>,
    layer_shell:Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    output:Option<wl_output::WlOutput>,
    output_ready:bool,
    xdg_acked:bool,
    layer_acked:bool,
    frame_done:u32,
    buffer_released:bool,
}
impl Dispatch<wl_registry::WlRegistry,()> for Client {
    fn event(s:&mut Self,r:&wl_registry::WlRegistry,e:wl_registry::Event,
             _:&(),_:&Connection,q:&QueueHandle<Self>){
        if let wl_registry::Event::Global{name,interface,version}=e {
            match interface.as_str() {
                "wl_compositor"=>s.compositor=Some(r.bind(name,version.min(4),q,())),
                "wl_shm"=>s.shm=Some(r.bind(name,version.min(1),q,())),
                "xdg_wm_base"=>s.wm=Some(r.bind(name,version.min(3),q,())),
                "wl_output"=>s.output=Some(r.bind(name,version.min(3),q,())),
                "zwlr_layer_shell_v1"=>s.layer_shell=Some(r.bind(name,version.min(1),q,())),
                _=>{},
            }
        }
    }
}
impl Dispatch<wl_output::WlOutput,()> for Client {
    fn event(s:&mut Self,_:&wl_output::WlOutput,e:wl_output::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){
        if let wl_output::Event::Mode{width,height,refresh,..}=e {
            if (width,height)==(1280,800) && refresh>0 {s.output_ready=true;}
        }
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase,()> for Client {
    fn event(_:&mut Self,wm:&xdg_wm_base::XdgWmBase,e:xdg_wm_base::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){
        if let xdg_wm_base::Event::Ping{serial}=e {wm.pong(serial);}
    }
}
impl Dispatch<xdg_surface::XdgSurface,()> for Client {
    fn event(s:&mut Self,r:&xdg_surface::XdgSurface,e:xdg_surface::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){
        if let xdg_surface::Event::Configure{serial}=e {
            r.ack_configure(serial);s.xdg_acked=true;
        }
    }
}
impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,()> for Client {
    fn event(s:&mut Self,r:&zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
             e:zwlr_layer_surface_v1::Event,_:&(),_:&Connection,_:&QueueHandle<Self>){
        match e {
            zwlr_layer_surface_v1::Event::Configure{serial,width,height}=>{
                if (width,height)!=(1280,52) {
                    panic!("native layer QEMU configured wrong panel dimensions");
                }
                r.ack_configure(serial);
                s.layer_acked=true;
            },
            zwlr_layer_surface_v1::Event::Closed=>panic!("native panel unexpectedly closed"),
            _=>{},
        }
    }
}
impl Dispatch<wl_buffer::WlBuffer,()> for Client {
    fn event(s:&mut Self,_:&wl_buffer::WlBuffer,e:wl_buffer::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){
        if let wl_buffer::Event::Release=e {s.buffer_released=true;}
    }
}
impl Dispatch<wl_callback::WlCallback,()> for Client {
    fn event(s:&mut Self,_:&wl_callback::WlCallback,e:wl_callback::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){
        if let wl_callback::Event::Done{..}=e {
            s.frame_done=s.frame_done.saturating_add(1);
        }
    }
}
macro_rules! ignore_events {
    ($($ty:ty),* $(,)?)=>{
        $(impl Dispatch<$ty,()> for Client {
            fn event(_:&mut Self,_:&$ty,_:<$ty as wayland_client::Proxy>::Event,
                     _:&(),_:&Connection,_:&QueueHandle<Self>){}
        })*
    };
}
ignore_events!(
    wl_compositor::WlCompositor,wl_surface::WlSurface,wl_shm::WlShm,
    wl_shm_pool::WlShmPool,xdg_toplevel::XdgToplevel,
    zwlr_layer_shell_v1::ZwlrLayerShellV1
);

fn main()->Result<(),Box<dyn Error>> {
    if std::env::var("MELANIA_QEMU_LAYER_PANEL_CLIENT").as_deref()!=Ok("YES_QEMU_VM") {
        return Err("QEMU-only real Wayland layer panel opt-in missing".into());
    }
    let conn=Connection::connect_to_env()?;
    let mut queue=conn.new_event_queue::<Client>();
    let q=queue.handle();
    let _registry=conn.display().get_registry(&q,());
    let mut s=Client::default();
    queue.roundtrip(&mut s)?;
    queue.roundtrip(&mut s)?;
    if !s.output_ready {return Err("virtual output 1280x800 missing".into());}
    let compositor=s.compositor.take().ok_or("wl_compositor missing")?;
    let wm=s.wm.take().ok_or("xdg_wm_base missing")?;
    let shm=s.shm.take().ok_or("wl_shm missing")?;
    let shell=s.layer_shell.take().ok_or("wlr-layer-shell missing")?;
    let a=compositor.create_surface(&q,());
    let xdg=wm.get_xdg_surface(&a,&q,());
    let top=xdg.get_toplevel(&q,());
    top.set_title("Melania QEMU real xdg window under taskbar".into());
    a.commit();
    let b=compositor.create_surface(&q,());
    let panel=shell.get_layer_surface(
        &b,None,zwlr_layer_shell_v1::Layer::Top,
        "melania-qemu-owned-layer-panel".into(),&q,());
    panel.set_anchor(zwlr_layer_surface_v1::Anchor::Bottom |
                     zwlr_layer_surface_v1::Anchor::Left |
                     zwlr_layer_surface_v1::Anchor::Right);
    // The real GTK taskbar requests width=0 between left/right anchors.
    // The compositor must configure the full output width and 52px height.
    panel.set_size(0,52);
    panel.set_exclusive_zone(40);
    b.commit();
    for _ in 0..8 {
        queue.roundtrip(&mut s)?;
        if s.xdg_acked && s.layer_acked {break;}
    }
    if !s.xdg_acked || !s.layer_acked {
        return Err("real xdg and layer configure/ACK not received".into());
    }
    println!("MELANIA_NATIVE_REAL_LAYER_AND_XDG_CONFIGURED=true");
    std::io::stdout().flush()?;

    for index in 1..=12u32 {
        let is_panel=index%2==0;
        let surface=if is_panel {&b} else {&a};
        let (w,h)=if is_panel {(1280usize,52usize)} else {(360,224)};
        let red=(30+(index*17)%200) as u8;
        let green=(40+(index*31)%190) as u8;
        let blue=(25+(index*43)%210) as u8;
        let mut pixels=vec![0u8;w*h*4];
        for p in pixels.chunks_exact_mut(4) {p.copy_from_slice(&[blue,green,red,0]);}
        let path=format!("/run/melania-native-qemu/layer-panel-client-{index}.bgra");
        let mut file=OpenOptions::new().create_new(true).read(true).write(true).open(path)?;
        file.write_all(&pixels)?;
        file.sync_all()?;
        let pool=shm.create_pool(file.as_fd(),pixels.len() as i32,&q,());
        let buffer=pool.create_buffer(
            0,w as i32,h as i32,(w*4) as i32,wl_shm::Format::Xrgb8888,&q,());
        s.buffer_released=false;
        let _callback=surface.frame(&q,());
        surface.attach(Some(&buffer),0,0);
        surface.damage_buffer(0,0,w as i32,h as i32);
        surface.commit();
        // Calls only resolve after Smithay verifies Rust KMS CRTC FB_ID.
        let deadline=std::time::Instant::now()+Duration::from_secs(11);
        while !s.buffer_released || s.frame_done<index {
            if std::time::Instant::now()>=deadline {
                return Err(format!("layer/xdg KMS callback {index} timed out").into());
            }
            queue.roundtrip(&mut s)?;
            thread::sleep(Duration::from_millis(15));
        }
        if s.frame_done!=index {
            return Err(format!("unexpected callback count at frame {index}").into());
        }
        println!("MELANIA_NATIVE_REAL_LAYER_KMS_CALLBACK={index} role={}",
                 if is_panel {"layer"} else {"xdg"});
        std::io::stdout().flush()?;
        buffer.destroy();
        pool.destroy();
    }
    println!("MELANIA_NATIVE_REAL_LAYER_12_KMS_FRAMES_PASS=true");
    std::io::stdout().flush()?;
    panel.destroy();b.destroy();
    top.destroy();xdg.destroy();a.destroy();
    shell.destroy();wm.destroy();conn.flush()?;
    Ok(())
}
