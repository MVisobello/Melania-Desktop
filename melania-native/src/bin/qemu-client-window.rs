//! QEMU-only real wl_shm xdg_toplevel client (Stage 26).
//! Client pixels come from this executable. The native Rust bridge must
//! acquire them through Wayland, then compose them to a separate snapshot.
use std::{error::Error,fs::OpenOptions,io::Write,os::fd::AsFd,thread,time::{Duration,Instant}};
use wayland_client::{
    Connection,Dispatch,QueueHandle,
    protocol::{wl_registry,wl_compositor,wl_surface,wl_shm,wl_shm_pool,wl_buffer,wl_output,wl_callback},
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
    frame_done_count:u32,
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
impl Dispatch<wl_callback::WlCallback,()> for Client {
    fn event(state:&mut Self,_:&wl_callback::WlCallback,event:wl_callback::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let wl_callback::Event::Done{callback_data}=event {
            state.frame_done_count=state.frame_done_count.saturating_add(1);
            println!("MELANIA_NATIVE_CLIENT_FRAME_CALLBACK_DONE={} at_ms={}",
                     state.frame_done_count,callback_data);
        }
    }
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
    let callback_probe=std::env::var("MELANIA_NATIVE_WAIT_PRESENT_CALLBACK")
        .as_deref()==Ok("YES_QEMU_VM");
    let three_frames=std::env::var("MELANIA_NATIVE_THREE_FRAMES")
        .as_deref()==Ok("YES_QEMU_VM");
    if callback_probe && !three_frames {
        return Err("QEMU frame callback test needs three live commits".into());
    }
    let two_windows=std::env::var("MELANIA_NATIVE_TWO_WINDOWS")
        .as_deref()==Ok("YES_QEMU_VM");
    if two_windows && (!callback_probe || !three_frames) {
        return Err("two-window test must use three verified KMS frame callbacks".into());
    }
    // A second genuine xdg_toplevel has its OWN wl_surface and wl_shm
    // commits. Frame 2 changes only this surface; the first stays mapped.
    let secondary=if two_windows {
        let other=compositor.create_surface(&qh,());
        let xdg_other=wm.get_xdg_surface(&other,&qh,());
        let top_other=xdg_other.get_toplevel(&qh,());
        top_other.set_title("Melania QEMU second real window".into());
        other.commit();
        for _ in 0..3 { queue.roundtrip(&mut state)?; }
        println!("MELANIA_NATIVE_CLIENT_SECOND_XDG_CONFIGURED=true");
        Some((other,xdg_other,top_other))
    } else {None};
    let two_frames=std::env::var("MELANIA_NATIVE_TWO_FRAMES")
        .as_deref()==Ok("YES_QEMU_VM");
    let count=if three_frames {3} else if two_frames {2} else {1};
    for frame_index in 0..count {
        let surface=if two_windows && frame_index==1 {
            &secondary.as_ref().ok_or("second client window missing")?.0
        } else {&surface};
        // Both frames originate from a real client's wl_shm pool. The
        // compositor never invents these colors or fills these pixels.
        let mut pixels=vec![0u8;W*H*4];
        for (index,p) in pixels.chunks_exact_mut(4).enumerate() {
            let left=index%W<W/2;
            let (r,g,b)=match (frame_index,left) {
                (0,true)=>(245u8,108u8,34u8),    // orange
                (0,false)=>(16u8,205u8,155u8),   // teal
                (1,true)=>(73u8,126u8,237u8),    // blue
                (1,false)=>(247u8,211u8,48u8),  // yellow
                (2,true)=>(222u8,64u8,118u8),   // rose
                (2,false)=>(117u8,238u8,90u8), // lime
                _=>unreachable!(),
            };
            p.copy_from_slice(&[b,g,r,0]);
        }
        // The Stage28 test uses the default filenames earlier in this same
        // guest boot; the concurrent Stage29 run must not reuse them.
        let path=if two_windows {
            match frame_index {
                0=>"/run/melania-native-qemu/two-window-live-1.bin",
                1=>"/run/melania-native-qemu/two-window-live-2.bin",
                _=>"/run/melania-native-qemu/two-window-live-3.bin",
            }
        } else if three_frames {
            match frame_index {
                0=>"/run/melania-native-qemu/client-live-1.bin",
                1=>"/run/melania-native-qemu/client-live-2.bin",
                _=>"/run/melania-native-qemu/client-live-3.bin",
            }
        } else if frame_index==0 {
            "/run/melania-native-qemu/client-window-shm.bin"
        } else {
            "/run/melania-native-qemu/client-window-shm-2.bin"
        };
        let mut file=OpenOptions::new().create_new(true).read(true).write(true)
            .open(path)?;
        file.write_all(&pixels)?;
        file.sync_all()?;
        let pool=shm.create_pool(file.as_fd(),pixels.len() as i32,&qh,());
        let buffer=pool.create_buffer(
            0,W as i32,H as i32,(W*4) as i32,wl_shm::Format::Xrgb8888,&qh,()
        );
        state.buffer_released=false;
        // wl_surface.frame must be requested BEFORE commit. This is
        // deliberately tested on the QEMU-only live presentation path.
        let _frame_callback=if callback_probe {
            Some(surface.frame(&qh,()))
        } else {None};
        surface.attach(Some(&buffer),0,0);
        surface.damage_buffer(0,0,W as i32,H as i32);
        surface.commit();
        // A real asynchronous KMS presenter does not block Smithay's
        // display.sync roundtrips. Keep the client responsive while waiting
        // for the actual KMS ACK and wl_callback.done, never treat the
        // first few successful syncs as proof of presentation.
        let present_deadline=Instant::now()+Duration::from_secs(9);
        loop {
            queue.roundtrip(&mut state)?;
            if state.buffer_released &&
               (!callback_probe || state.frame_done_count==frame_index as u32+1) {
                break;
            }
            if Instant::now()>=present_deadline {
                return Err(format!("KMS presentation deadline exceeded for client frame {}",
                                   frame_index+1).into());
            }
            thread::sleep(Duration::from_millis(35));
        }
        if !state.buffer_released ||
           (callback_probe && state.frame_done_count!=frame_index as u32+1) {
            return Err(format!("KMS presentation did not complete client frame callback {}",
                               frame_index+1).into());
        }
        println!("MELANIA_NATIVE_CLIENT_WL_SHM_RELEASED=true");
        println!("MELANIA_NATIVE_CLIENT_FRAME_COMMITTED={}",frame_index+1);
        if two_windows {
            println!("MELANIA_NATIVE_CLIENT_FRAME_WINDOW_ID={}",if frame_index==1 {2} else {1});
        }
        if callback_probe {
            println!("MELANIA_NATIVE_CLIENT_FRAME_PRESENTED={}",frame_index+1);
        }
        buffer.destroy();
        pool.destroy();
    }
    // Keep both genuine xdg clients alive briefly after the third ACK.
    // The QMP focus injector sends B→A while the compositor is still
    // dispatching input; a destroyed surface cannot acquire real focus.
    if two_windows {
        for _ in 0..20 {
            queue.roundtrip(&mut state)?;
            thread::sleep(Duration::from_millis(200));
        }
        println!("MELANIA_NATIVE_CLIENT_FOCUS_INPUT_HOLD=true");
    }
    if callback_probe {
        if state.frame_done_count!=3 {
            return Err("not all three frame callbacks arrived from native renderer".into());
        }
        println!("MELANIA_NATIVE_CLIENT_FRAME_CALLBACKS_PRESENTED_PASS=true");
    }
    println!("MELANIA_NATIVE_CLIENT_XDG_WINDOW_DRAWN=true");
    std::io::stdout().flush()?;
    if let Some((other,xdg_other,top_other))=secondary {
        top_other.destroy();
        xdg_other.destroy();
        other.destroy();
    }
    toplevel.destroy();
    xdg.destroy();
    surface.destroy();
    wm.destroy();
    drop(shm);
    drop(state.output.take());
    connection.flush()?;
    Ok(())
}
