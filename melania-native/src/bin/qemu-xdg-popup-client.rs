//! QEMU-only real xdg_popup -> ACKed wl_shm -> GPU/KMS/CRTC smoke client.
//! The popup is a separate mapped Wayland surface, never a compositor mock.
use std::{error::Error,fs::OpenOptions,io::Write,os::fd::AsFd,
          thread,time::{Duration,Instant}};
use wayland_client::{
    Connection,Dispatch,QueueHandle,
    protocol::{wl_registry,wl_compositor,wl_surface,wl_shm,wl_shm_pool,
              wl_buffer,wl_callback},
};
use wayland_protocols::xdg::shell::client::{
    xdg_wm_base,xdg_surface,xdg_toplevel,xdg_popup,xdg_positioner,
};

type Result<T>=std::result::Result<T,Box<dyn Error>>;
const W:usize=360;
const H:usize=224;
const POP_W:usize=180;
const POP_H:usize=100;

#[derive(Default)]
struct Client {
    compositor:Option<wl_compositor::WlCompositor>,
    shm:Option<wl_shm::WlShm>,
    wm:Option<xdg_wm_base::XdgWmBase>,
    root_configured:bool,
    popup_configured:bool,
    dismissed_unexpectedly:bool,
    popup_done_after_frame2:bool,
    popup_rect:Option<(i32,i32,i32,i32)>,
    frame_callbacks:u32,
    buffer_released:bool,
}
impl Dispatch<wl_registry::WlRegistry,()> for Client {
    fn event(s:&mut Self,r:&wl_registry::WlRegistry,e:wl_registry::Event,
             _:&(),_:&Connection,q:&QueueHandle<Self>) {
        if let wl_registry::Event::Global{name,interface,version}=e {
            match interface.as_str() {
                "wl_compositor"=>s.compositor=Some(r.bind(name,version.min(4),q,())),
                "wl_shm"=>s.shm=Some(r.bind(name,version.min(1),q,())),
                "xdg_wm_base"=>s.wm=Some(r.bind(name,version.min(3),q,())),
                _=>{}
            }
        }
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase,()> for Client {
    fn event(_: &mut Self,wm:&xdg_wm_base::XdgWmBase,e:xdg_wm_base::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping{serial}=e {wm.pong(serial);}
    }
}
impl Dispatch<xdg_surface::XdgSurface,u8> for Client {
    fn event(s:&mut Self,role:&xdg_surface::XdgSurface,e:xdg_surface::Event,
             index:&u8,_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_surface::Event::Configure{serial}=e {
            role.ack_configure(serial);
            if *index==1 {s.root_configured=true;}
            else if *index==2 && s.popup_rect.is_some() {s.popup_configured=true;}
        }
    }
}
impl Dispatch<xdg_popup::XdgPopup,u8> for Client {
    fn event(s:&mut Self,_:&xdg_popup::XdgPopup,e:xdg_popup::Event,
             _:&u8,_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_popup::Event::Configure{x,y,width,height}=e {
            s.popup_rect=Some((x,y,width,height));
        }
        if let xdg_popup::Event::PopupDone=e {
            if s.frame_callbacks>=2 {
                s.popup_done_after_frame2=true;
                println!("MELANIA_NATIVE_REAL_POPUP_CLIENT_POPUP_DONE_AFTER_ESCAPE=true");
                let _=std::io::stdout().flush();
            } else {
                s.dismissed_unexpectedly=true;
                eprintln!("MELANIA_NATIVE_POPUP_UNEXPECTED_DISMISSAL=true");
            }
        }
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel,u8> for Client {
    fn event(_: &mut Self,_:&xdg_toplevel::XdgToplevel,_:xdg_toplevel::Event,
             _:&u8,_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<xdg_positioner::XdgPositioner,()> for Client {
    fn event(_: &mut Self,_:&xdg_positioner::XdgPositioner,_:xdg_positioner::Event,
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
impl Dispatch<wl_shm::WlShm,()> for Client {
    fn event(_: &mut Self,_:&wl_shm::WlShm,_:wl_shm::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_shm_pool::WlShmPool,()> for Client {
    fn event(_: &mut Self,_:&wl_shm_pool::WlShmPool,_:wl_shm_pool::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_buffer::WlBuffer,()> for Client {
    fn event(s:&mut Self,_:&wl_buffer::WlBuffer,e:wl_buffer::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let wl_buffer::Event::Release=e {s.buffer_released=true;}
    }
}
impl Dispatch<wl_callback::WlCallback,()> for Client {
    fn event(s:&mut Self,_:&wl_callback::WlCallback,e:wl_callback::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let wl_callback::Event::Done{..}=e {
            s.frame_callbacks=s.frame_callbacks.saturating_add(1);
        }
    }
}
fn draw(
    surface:&wl_surface::WlSurface,shm:&wl_shm::WlShm,
    queue:&mut wayland_client::EventQueue<Client>,state:&mut Client,
    w:usize,h:usize,rgb:(u8,u8,u8),index:u32
)->Result<()> {
    let (r,g,b)=rgb;
    let mut pixels=vec![0u8;w*h*4];
    for p in pixels.chunks_exact_mut(4){p.copy_from_slice(&[b,g,r,0]);}
    let path=format!("/run/melania-native-qemu/xdg-popup-stage118-{index}.bgra");
    let mut file=OpenOptions::new().create_new(true).read(true).write(true).open(path)?;
    file.write_all(&pixels)?;
    file.sync_all()?;
    let q=queue.handle();
    let pool=shm.create_pool(file.as_fd(),(w*h*4) as i32,&q,());
    let buffer=pool.create_buffer(0,w as i32,h as i32,(w*4) as i32,
                                  wl_shm::Format::Xrgb8888,&q,());
    state.buffer_released=false;
    let _callback=surface.frame(&q,());
    surface.attach(Some(&buffer),0,0);
    surface.damage_buffer(0,0,w as i32,h as i32);
    surface.commit();
    let deadline=Instant::now()+Duration::from_secs(40);
    while !state.buffer_released || state.frame_callbacks<index {
        if Instant::now()>=deadline {
            return Err(format!("popup client frame {index} KMS ACK/release absent").into());
        }
        queue.roundtrip(state)?;
        if state.dismissed_unexpectedly {
            return Err("xdg_popup dismissed before pixel evidence".into());
        }
    }
    if state.frame_callbacks!=index {
        return Err("duplicate or premature popup client callback".into());
    }
    println!("MELANIA_NATIVE_REAL_POPUP_CLIENT_FRAME{index}_ACK=true");
    std::io::stdout().flush()?;
    buffer.destroy();
    pool.destroy();
    Ok(())
}
fn main()->Result<()> {
    if std::env::var("MELANIA_QEMU_XDG_POPUP_CLIENT").as_deref()!=Ok("YES_QEMU_VM"){
        return Err("real popup client allowed only in disposable QEMU".into());
    }
    let connection=Connection::connect_to_env()?;
    let mut queue=connection.new_event_queue::<Client>();
    let q=queue.handle();
    let _registry=connection.display().get_registry(&q,());
    let mut state=Client::default();
    queue.roundtrip(&mut state)?;
    let compositor=state.compositor.take().ok_or("wl_compositor missing")?;
    let wm=state.wm.take().ok_or("xdg_wm_base missing")?;
    let shm=state.shm.take().ok_or("wl_shm missing")?;
    let root=compositor.create_surface(&q,());
    let root_xdg=wm.get_xdg_surface(&root,&q,1u8);
    let root_top=root_xdg.get_toplevel(&q,1u8);
    root_top.set_title("Melania QEMU real popup parent".into());
    root.commit();
    let deadline=Instant::now()+Duration::from_secs(12);
    while !state.root_configured {
        if Instant::now()>deadline {return Err("real parent XDG configure missing".into());}
        queue.roundtrip(&mut state)?;
    }
    draw(&root,&shm,&mut queue,&mut state,W,H,(185,50,75),1)?;

    let positioner=wm.create_positioner(&q,());
    positioner.set_size(POP_W as i32,POP_H as i32);
    positioner.set_anchor_rect(220,85,60,30);
    positioner.set_anchor(xdg_positioner::Anchor::BottomRight);
    positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
    let popup_surface=compositor.create_surface(&q,());
    let popup_xdg=wm.get_xdg_surface(&popup_surface,&q,2u8);
    let popup=popup_xdg.get_popup(Some(&root_xdg),&positioner,&q,2u8);
    popup_surface.commit();
    let deadline=Instant::now()+Duration::from_secs(15);
    while !state.popup_configured {
        if Instant::now()>=deadline {return Err("real popup configure/ACK missing".into());}
        queue.roundtrip(&mut state)?;
        if state.dismissed_unexpectedly {
            return Err("popup dismissed before XDG configure".into());
        }
    }
    let (x,y,w,h)=state.popup_rect.ok_or("XDG popup geometry missing")?;
    if (x,y,w,h)!=(280,115,POP_W as i32,POP_H as i32) {
        return Err(format!("unexpected real popup geometry ({x},{y},{w},{h})").into());
    }
    println!("MELANIA_NATIVE_REAL_POPUP_CONFIGURE_ACK=280,115 180x100");
    std::io::stdout().flush()?;
    draw(&popup_surface,&shm,&mut queue,&mut state,POP_W,POP_H,(245,180,50),2)?;
    // Keep popup mapped while the host captures the genuine GPU scanout.
    thread::sleep(Duration::from_secs(10));
    // Host first validates the actual popup pixels on the virtual CRTC,
    // then sends a real USB Escape. Only the compositor's xdg_popup.popup_done
    // may trigger teardown. Do not pre-emptively remove the client pixels.
    let deadline=Instant::now()+Duration::from_secs(35);
    while !state.popup_done_after_frame2 {
        if Instant::now()>=deadline {
            return Err("real QEMU USB Escape did not deliver xdg_popup.popup_done".into());
        }
        queue.roundtrip(&mut state)?;
        if state.dismissed_unexpectedly {
            return Err("popup dismissed before QEMU CRTC capture".into());
        }
    }
    popup.destroy();
    popup_xdg.destroy();
    popup_surface.destroy();
    connection.flush()?;
    println!("MELANIA_NATIVE_REAL_POPUP_CLIENT_DESTROYED=true");
    std::io::stdout().flush()?;
    // The compositor sends a separate compositor-only KMS frame to clear
    // popup pixels. Keep the root mapped through that real framebuffer ACK.
    thread::sleep(Duration::from_secs(10));
    root_top.destroy();
    root_xdg.destroy();
    root.destroy();
    positioner.destroy();
    connection.flush()?;
    Ok(())
}
