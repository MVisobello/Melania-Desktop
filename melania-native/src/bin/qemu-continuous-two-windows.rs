//! Stage 48: two independent Wayland xdg_toplevel surfaces, each retained across real KMS flips.
//! VM-only scenario; every buffer release and callback requires a KMS ACK.
use std::{error::Error,fs::OpenOptions,io::{Read,Write},net::Shutdown,
    os::{fd::AsFd,unix::net::UnixStream},thread,time::{Duration,Instant}};
use wayland_client::{
    Connection,Dispatch,QueueHandle,
    protocol::{wl_registry,wl_compositor,wl_surface,wl_shm,wl_shm_pool,wl_buffer,wl_output,wl_callback,wl_seat,wl_keyboard},
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
    seat:Option<wl_seat::WlSeat>,
    alt_tab_probe:bool,
    tab_probe:bool,
    combined_probe:bool,
    tab_focus_b:bool,
    tab_focus_a:bool,
    tab_guest_release:bool,
    first_surface:Option<wl_surface::WlSurface>,
    second_surface:Option<wl_surface::WlSurface>,
    saw_focus_b:bool,
    saw_focus_a:bool,
    saw_alt_f4_close_a:bool,
    output_mode_received:bool,
    xdg_acked:bool,
    buffer_released:bool,
    frame_done_count:u32,
    dynamic_xdg_resize:bool,
    resize_requested_a:Option<(usize,usize)>,
    resize_requested_b:Option<(usize,usize)>,
    resize_acked_a:bool,
    resize_acked_b:bool,
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
                "wl_seat"=>state.seat=Some(registry.bind(name,version.min(7),qh,())),
                _=>{}
            }
        }
    }
}
impl Dispatch<wl_seat::WlSeat,()> for Client {
    fn event(_: &mut Self,_:&wl_seat::WlSeat,_:wl_seat::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_keyboard::WlKeyboard,()> for Client {
    fn event(s:&mut Self,_:&wl_keyboard::WlKeyboard,event:wl_keyboard::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if !(s.alt_tab_probe || s.tab_probe) || s.frame_done_count<12 {return;}
        match event {
            wl_keyboard::Event::Enter{surface,..}=>{
                if s.tab_probe {
                    if s.second_surface.as_ref()==Some(&surface) && !s.tab_focus_b {
                        s.tab_focus_b=true;
                        println!("MELANIA_QEMU_TAB_REAL_WL_KEYBOARD_ENTER=B");
                        let _=std::io::stdout().flush();
                    } else if s.first_surface.as_ref()==Some(&surface) && s.tab_focus_b {
                        s.tab_focus_a=true;
                        println!("MELANIA_QEMU_TAB_REAL_WL_KEYBOARD_ENTER=A");
                        let _=std::io::stdout().flush();
                    }
                } else if s.second_surface.as_ref()==Some(&surface) && !s.saw_focus_b {
                    s.saw_focus_b=true;
                    println!("MELANIA_QEMU_ALT_TAB_REAL_WL_KEYBOARD_ENTER=B");
                    let _=std::io::stdout().flush();
                } else if s.first_surface.as_ref()==Some(&surface) && s.saw_focus_b {
                    s.saw_focus_a=true;
                    println!("MELANIA_QEMU_ALT_TAB_REAL_WL_KEYBOARD_ENTER=A");
                    let _=std::io::stdout().flush();
                }
            },
            wl_keyboard::Event::Key{key,state,..}
                if s.tab_probe && key==66 &&
                   state==wayland_client::WEnum::Value(wl_keyboard::KeyState::Pressed) =>{
                // Real physical USB F8 is the shutdown gate *after* QMP
                // verified all four retained CRTC tab transitions.
                s.tab_guest_release=true;
                println!("MELANIA_QEMU_TAB_REAL_USB_RELEASE=F8");
                let _=std::io::stdout().flush();
            },
            _=>{},
        }
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase,()> for Client {
    fn event(_: &mut Self,wm:&xdg_wm_base::XdgWmBase,
             event:xdg_wm_base::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping{serial}=event {wm.pong(serial);}
    }
}
impl Dispatch<xdg_surface::XdgSurface,u8> for Client {
    fn event(state:&mut Self,surface:&xdg_surface::XdgSurface,
             event:xdg_surface::Event,slot:&u8,_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_surface::Event::Configure{serial}=event {
            // ACK the actual xdg_surface serial, never merely the paired
            // xdg_toplevel size notification.
            surface.ack_configure(serial);
            state.xdg_acked=true;
            if state.dynamic_xdg_resize {
                if *slot==1 && state.resize_requested_a.is_some() {
                    state.resize_acked_a=true;
                } else if *slot==2 && state.resize_requested_b.is_some() {
                    state.resize_acked_b=true;
                }
            }
        }
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel,u8> for Client {
    fn event(state:&mut Self,_:&xdg_toplevel::XdgToplevel,
             event:xdg_toplevel::Event,slot:&u8,_:&Connection,_:&QueueHandle<Self>) {
        if state.alt_tab_probe && state.saw_focus_a && *slot==1 &&
           matches!(&event,xdg_toplevel::Event::Close) {
            state.saw_alt_f4_close_a=true;
            println!("MELANIA_QEMU_ALT_F4_REAL_XDG_CLOSE_RECEIVED=A");
            let _=std::io::stdout().flush();
        }
        if state.dynamic_xdg_resize {
            if let xdg_toplevel::Event::Configure{width,height,..}=event {
                if width>0 && height>0 {
                    let requested=(width as usize,height as usize);
                    match *slot {
                        1=>{state.resize_requested_a=Some(requested);
                            state.resize_acked_a=false;},
                        2=>{state.resize_requested_b=Some(requested);
                            state.resize_acked_b=false;},
                        _=>{}
                    }
                }
            }
        }
    }
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

/// The same bounded IPC used by Melania Panel. Stage99 exercises a
/// *real* compositor-only transition without attaching a new client buffer.
fn control_request(path:&str,command:&str)->Result<String,Box<dyn Error>>{
    let mut stream=UnixStream::connect(path)?;
    stream.set_read_timeout(Some(Duration::from_millis(700)))?;
    stream.set_write_timeout(Some(Duration::from_millis(700)))?;
    stream.write_all(command.as_bytes())?;
    stream.shutdown(Shutdown::Write)?;
    let mut reply=String::new();
    stream.take(8192).read_to_string(&mut reply)?;
    Ok(reply)
}

fn await_presented_frame(frame:u32)->Result<(),Box<dyn Error>>{
    let path=format!("/run/melania-native-qemu/managed-minimize-frame{frame}-acked");
    let deadline=Instant::now()+Duration::from_secs(20);
    while !std::path::Path::new(&path).is_file(){
        if Instant::now()>=deadline{
            return Err(format!("no verified kernel CRTC ACK for compositor-only frame {frame}").into());
        }
        thread::sleep(Duration::from_millis(25));
    }
    let marker=std::fs::read_to_string(&path)?;
    if marker!="verified CRTC route and framebuffer ACK\n" {
        return Err(format!("invalid compositor/DRM ACK marker for frame {frame}").into());
    }
    Ok(())
}

fn main()->Result<(),Box<dyn Error>> {
    if std::env::var("MELANIA_QEMU_CONTINUOUS_TWO_CLIENTS").as_deref()!=Ok("YES_QEMU_VM") {
        return Err("two-window continuous Wayland probe requires explicit disposable VM opt-in".into());
    }
    let connection=Connection::connect_to_env()?;
    let mut queue=connection.new_event_queue::<Client>();
    let qh=queue.handle();
    let _registry=connection.display().get_registry(&qh,());
    let mut state=Client::default();
    let managed_role_pixels=std::env::var("MELANIA_QEMU_MANAGED_ROLE_CLIENT")
        .as_deref()==Ok("YES_QEMU_VM");
    state.alt_tab_probe=std::env::var("MELANIA_QEMU_ALT_TAB_CLIENT")
        .as_deref()==Ok("YES_QEMU_VM");
    state.combined_probe=std::env::var("MELANIA_QEMU_COMBINED_TAB_CLIENT")
        .as_deref()==Ok("YES_QEMU_VM");
    state.tab_probe=std::env::var("MELANIA_QEMU_TAB_CLIENT")
        .as_deref()==Ok("YES_QEMU_VM") || state.combined_probe;
    if state.alt_tab_probe && state.tab_probe {
        return Err("two QEMU keyboard acceptance modes cannot overlap".into());
    }
    state.dynamic_xdg_resize=std::env::var("MELANIA_QEMU_XDG_RESIZE_CLIENT")
        .as_deref()==Ok("YES_QEMU_VM") || managed_role_pixels ||
        state.combined_probe;
    queue.roundtrip(&mut state)?;
    queue.roundtrip(&mut state)?;
    if !state.output_mode_received {
        return Err("two-window continuous client did not see VM 1280x800 output".into());
    }
    let _keyboard=if state.alt_tab_probe || state.tab_probe {
        Some(state.seat.as_ref().ok_or("real wl_seat required for Alt+Tab")?
            .get_keyboard(&qh,()))
    } else {None};
    let compositor=state.compositor.take().ok_or("wl_compositor missing")?;
    let wm=state.wm.take().ok_or("xdg_wm_base missing")?;
    let shm=state.shm.take().ok_or("wl_shm missing")?;
    let first=compositor.create_surface(&qh,());
    state.first_surface=Some(first.clone());
    let first_xdg=wm.get_xdg_surface(&first,&qh,1u8);
    let first_top=first_xdg.get_toplevel(&qh,1u8);
    first_top.set_title("Melania QEMU retained continuous A".into());
    first.commit();
    for _ in 0..5 {
        queue.roundtrip(&mut state)?;
        if state.xdg_acked {break;}
    }
    if !state.xdg_acked {return Err("first xdg configure not received".into());}
    state.xdg_acked=false;
    let second=compositor.create_surface(&qh,());
    state.second_surface=Some(second.clone());
    let second_xdg=wm.get_xdg_surface(&second,&qh,2u8);
    let second_top=second_xdg.get_toplevel(&qh,2u8);
    second_top.set_title("Melania QEMU retained continuous B".into());
    second.commit();
    for _ in 0..5 {
        queue.roundtrip(&mut state)?;
        if state.xdg_acked {break;}
    }
    if !state.xdg_acked {return Err("second xdg configure not received".into());}
    println!("MELANIA_NATIVE_CONTINUOUS_TWO_XDG_CONFIGURED=true");
    std::io::stdout().flush()?;

    // A1 B2 A3 B4 ... A11 B12. A's old image survives B's commit.
    // Each client owns an independent wl_shm buffer and wl_surface.
    let dynamic=std::env::var("MELANIA_QEMU_CONTINUOUS_DYNAMIC_CLIENT")
        .as_deref()==Ok("YES_QEMU_VM");
    if dynamic {
        println!("MELANIA_NATIVE_CONTINUOUS_DYNAMIC_SIZES=320x192;220x180");
        println!("MELANIA_NATIVE_CONTINUOUS_DYNAMIC_SHRINK=400x240->320x192;280x200->220x180");
        std::io::stdout().flush()?;
    }
    for index in 1..=12u32 {
        let surface=if index%2==1 {&first} else {&second};
        if managed_role_pixels && matches!(index,3|5|7|9) {
            // A real client request (not a state-model fixture) changes the
            // xdg_toplevel role. Wait for the paired xdg_surface ack before
            // attaching a new wl_shm image of exactly the configured size.
            state.resize_requested_a=None;
            state.resize_acked_a=false;
            match index {
                3=>first_top.set_maximized(),
                5=>first_top.unset_maximized(),
                7=>first_top.set_fullscreen(None),
                9=>first_top.unset_fullscreen(),
                _=>unreachable!(),
            }
            let deadline=Instant::now()+Duration::from_secs(10);
            while !state.resize_acked_a {
                if Instant::now()>=deadline {
                    return Err(format!("managed role frame {index} configure/ACK timed out").into());
                }
                queue.roundtrip(&mut state)?;
            }
            let expected=if index==3 || index==7 {(1280,800)} else {(W,H)};
            if state.resize_requested_a!=Some(expected) {
                return Err(format!(
                    "managed role frame {index} size {:?} not {expected:?}",
                    state.resize_requested_a).into());
            }
            println!("MELANIA_NATIVE_MANAGED_ROLE_CLIENT_ACK={index} size={}x{}",
                     expected.0,expected.1);
            std::io::stdout().flush()?;
        }
        // Frames 7 and 8 must honor the compositor's xdg_toplevel.configure
        // dimensions and the matching xdg_surface.ack_configure serial.
        // No buffer from the smaller geometry is submitted beforehand.
        if dynamic && state.dynamic_xdg_resize && (index==7 || index==8) {
            let deadline=Instant::now()+Duration::from_secs(10);
            loop {
                let done=if index==7 {state.resize_acked_a}
                         else {state.resize_acked_b};
                if done {break;}
                if Instant::now()>deadline {
                    return Err("xdg resize configure/ACK roundtrip timed out".into());
                }
                queue.roundtrip(&mut state)?;
            }
            let size=if index==7 {state.resize_requested_a}
                     else {state.resize_requested_b};
            let expected=if index==7 {(320,192)} else {(220,180)};
            if size!=Some(expected) {
                return Err(format!("xdg configure geometry mismatch at frame {index}").into());
            }
            println!("MELANIA_NATIVE_CLIENT_XDG_RESIZE_ACK_{}=true",
                     if index==7 {"A"} else {"B"});
            std::io::stdout().flush()?;
        }
        // Both clients replace their old wl_shm content with the requested
        // geometry and the CPU compositor clears the old pixels in QMP.
        let (client_w,client_h)=if managed_role_pixels {
            if index%2==1 {state.resize_requested_a.unwrap_or((W,H))}
            else {(W,H)}
        } else if dynamic {
            if index%2==1 {
                if index<7 {(400usize,240usize)}
                else if state.dynamic_xdg_resize {
                    state.resize_requested_a.ok_or("unconfigured xdg window A")?
                } else {(320usize,192usize)}
            } else if index<8 {(280usize,200usize)}
            else if state.dynamic_xdg_resize {
                state.resize_requested_b.ok_or("unconfigured xdg window B")?
            } else {(220usize,180usize)}
        } else {(W,H)};
        let mut pixels=vec![0u8;client_w*client_h*4];
        let red=(30+(index*17)%200) as u8;
        let green=(40+(index*31)%190) as u8;
        let blue=(25+(index*43)%210) as u8;
        // A second independent proof: B uses actual premultiplied Wayland
        // ARGB8888 at half opacity. The generic renderer must blend ONCE.
        let translucent=dynamic && index%2==0;
        let premul=|component:u8|((u16::from(component)*128+127)/255) as u8;
        for pixel in pixels.chunks_exact_mut(4) {
            if translucent {
                pixel.copy_from_slice(&[
                    premul(blue),premul(green),premul(red),128
                ]);
            } else {
                pixel.copy_from_slice(&[blue,green,red,0]);
            }
        }
        // The same disposable VM runs Stage48 before Stage49. Use a distinct
        // source filename in the reopen scenario: create_new must not
        // overwrite earlier client-owned wl_shm fixtures.
        let path=if state.combined_probe {
            format!("/run/melania-native-qemu/combined-tab-client-{index}.bgra")
        } else if state.tab_probe {
            format!("/run/melania-native-qemu/tab-stack-client-{index}.bgra")
        } else if state.alt_tab_probe {
            format!("/run/melania-native-qemu/alt-tab-client-{index}.bgra")
        } else if managed_role_pixels {
            // Stage81B runs after Stage48 in the SAME QEMU guest: create_new
            // must not collide with earlier two-window fixture filenames.
            format!("/run/melania-native-qemu/managed-role-client-{index}.bgra")
        } else if dynamic {
            format!("/run/melania-native-qemu/continuous-dynamic-client-{index}.bgra")
        } else if std::env::var("MELANIA_QEMU_CONTINUOUS_REOPEN_CLIENT")
            .as_deref()==Ok("YES_QEMU_VM") {
            format!("/run/melania-native-qemu/continuous-reopen-seed-{index}.bgra")
        } else {
            format!("/run/melania-native-qemu/continuous-two-client-{index}.bgra")
        };
        let mut file=OpenOptions::new().create_new(true).read(true).write(true).open(path)?;
        file.write_all(&pixels)?;
        file.sync_all()?;
        let pool=shm.create_pool(file.as_fd(),pixels.len() as i32,&qh,());
        let buffer=pool.create_buffer(
            0,client_w as i32,client_h as i32,
            (client_w*4) as i32,
            if translucent {wl_shm::Format::Argb8888} else {wl_shm::Format::Xrgb8888},
            &qh,()
        );
        state.buffer_released=false;
        let _callback=surface.frame(&qh,());
        surface.attach(Some(&buffer),0,0);
        surface.damage_buffer(0,0,client_w as i32,client_h as i32);
        surface.commit();
        let deadline=Instant::now()+Duration::from_secs(10);
        while !state.buffer_released || state.frame_done_count<index {
            queue.roundtrip(&mut state)?;
            if Instant::now()>=deadline {
                return Err(format!("two-window continuous real KMS frame {index} timed out").into());
            }
            thread::sleep(Duration::from_millis(12));
        }
        if state.frame_done_count!=index {
            return Err("duplicate or premature Wayland callback in two-window mode".into());
        }
        println!("MELANIA_NATIVE_CONTINUOUS_TWO_WINDOW_ACKED={index} window={}",
                 if index%2==1 {"A"} else {"B"});
        if managed_role_pixels {
            // The client callback is delivered only after the actual Rust
            // KMS worker confirms the virtual CRTC framebuffer.
            println!("MELANIA_NATIVE_MANAGED_ROLE_KMS_CALLBACK={index}");
        }
        std::io::stdout().flush()?;
        buffer.destroy();
        pool.destroy();
    }
    // Dedicated tab-stack QEMU scenario keeps both *real* XDG clients
    // mapped after 12 kernel-confirmed frames. It terminates only when a
    // QMP-injected physical USB F8 arrives following real KMS screenshot
    // acceptance. No compositor IPC or synthetic Wayland input is involved.
    if state.tab_probe && !state.combined_probe {
        if managed_role_pixels || dynamic || state.alt_tab_probe {
            return Err("tab QEMU test requires stable normal XDG client pixels".into());
        }
        println!("MELANIA_QEMU_TAB_TWO_WAYLAND_SURFACES_READY=true");
        std::io::stdout().flush()?;
        let deadline=Instant::now()+Duration::from_secs(90);
        while !state.tab_guest_release {
            if Instant::now()>=deadline {
                return Err("QEMU USB tab input/screenshots/release timed out".into());
            }
            queue.roundtrip(&mut state)?;
        }
        if !state.tab_focus_b || !state.tab_focus_a {
            return Err("real wl_keyboard focus B then A not delivered by native tab stacks".into());
        }
        println!("MELANIA_QEMU_TAB_TWO_WL_KEYBOARD_ENTERS_PASS=true");
        std::io::stdout().flush()?;
    }
    // Stage136: two *genuine* XDG surfaces ACK their independent configure
    // serials and submit owned, exact-size wl_shm buffers in one group.
    // Never wait for A's frame callback before submitting B: atomic KMS
    // presentation is legal only after BOTH are ready.
    if state.combined_probe {
        if dynamic || managed_role_pixels || state.alt_tab_probe {
            return Err("combined tab-tiling requires ordinary real XDG clients".into());
        }
        println!("MELANIA_QEMU_COMBINED_TAB_TWO_WAYLAND_SURFACES_READY=true");
        std::io::stdout().flush()?;
        let deadline=Instant::now()+Duration::from_secs(90);
        while !(state.resize_acked_a && state.resize_acked_b) {
            if Instant::now()>=deadline {
                return Err("combined tab-tiling both XDG ACKs timed out".into());
            }
            queue.roundtrip(&mut state)?;
        }
        let desired=(1280usize,800usize);
        if state.resize_requested_a!=Some(desired) ||
           state.resize_requested_b!=Some(desired) {
            return Err(format!("combined tab configure sizes: {:?}, {:?}",
                state.resize_requested_a,state.resize_requested_b).into());
        }
        println!("MELANIA_QEMU_COMBINED_TAB_TWO_REAL_XDG_ACKS=true");
        std::io::stdout().flush()?;
        let mut resources=Vec::new();
        for (id,surface,rgba) in [
            (1u8,&first,[78u8,191,217,0]),
            (2u8,&second,[121u8,222,34,0]),
        ] {
            let mut pixels=vec![0u8;desired.0*desired.1*4];
            for px in pixels.chunks_exact_mut(4) {px.copy_from_slice(&rgba);}
            let path=format!("/run/melania-native-qemu/combined-tab-resize-{id}.bgra");
            let mut file=OpenOptions::new().create_new(true)
                .read(true).write(true).open(path)?;
            file.write_all(&pixels)?;
            file.sync_all()?;
            let pool=shm.create_pool(file.as_fd(),pixels.len() as i32,&qh,());
            let buffer=pool.create_buffer(0,desired.0 as i32,desired.1 as i32,
                (desired.0*4) as i32,wl_shm::Format::Xrgb8888,&qh,());
            let _callback=surface.frame(&qh,());
            surface.attach(Some(&buffer),0,0);
            surface.damage_buffer(0,0,desired.0 as i32,desired.1 as i32);
            surface.commit();
            resources.push((pool,buffer));
        }
        connection.flush()?;
        // The frame callbacks arrive after the single real DRM batch
        // presentation, not after the first client's configure alone.
        let deadline=Instant::now()+Duration::from_secs(55);
        while state.frame_done_count<14 {
            if Instant::now()>=deadline {
                return Err("group XDG owned buffers did not share a real KMS ACK".into());
            }
            queue.roundtrip(&mut state)?;
        }
        println!("MELANIA_QEMU_COMBINED_TAB_TWO_OWNED_SHM_CALLBACKS=true");
        std::io::stdout().flush()?;
        let deadline=Instant::now()+Duration::from_secs(90);
        while !state.tab_guest_release {
            if Instant::now()>=deadline {
                return Err("QMP combined tab screenshots/keyboard not completed".into());
            }
            queue.roundtrip(&mut state)?;
        }
        if !state.tab_focus_b || !state.tab_focus_a {
            return Err("combined tab switching lacked real keyboard Enter B/A".into());
        }
        for (pool,buffer) in resources {
            buffer.destroy();
            pool.destroy();
        }
        println!("MELANIA_QEMU_COMBINED_TAB_TWO_REAL_KEYBOARD_ENTERS_PASS=true");
        std::io::stdout().flush()?;
    }
    // An independent Stage116 physical USB keyboard proves actual Smithay
    // wl_keyboard enter on B then A. The event itself comes only from QMP's
    // private virtual keyboard and the live libinput seat, never IPC.
    if state.alt_tab_probe {
        if managed_role_pixels || dynamic {
            return Err("Alt-Tab QEMU proof requires ordinary stable XDG client pixels".into());
        }
        println!("MELANIA_QEMU_ALT_TAB_TWO_WAYLAND_SURFACES_READY=true");
        std::io::stdout().flush()?;
        let deadline=Instant::now()+Duration::from_secs(70);
        while !state.saw_focus_a {
            if Instant::now()>=deadline {
                return Err("QEMU USB Alt+Tab did not switch real keyboard focus B then A".into());
            }
            queue.roundtrip(&mut state)?;
        }
        println!("MELANIA_NATIVE_ALT_TAB_TWO_REAL_KEYBOARD_ENTERS_PASS=true");
        std::io::stdout().flush()?;
        // Keep the genuine windows mapped for the host's independent QMP
        // CRTC screenshot after the compositor-only KMS frame 13 ACK.
        thread::sleep(Duration::from_secs(15));
        let deadline=Instant::now()+Duration::from_secs(40);
        while !state.saw_alt_f4_close_a {
            if Instant::now()>=deadline {
                return Err("real USB Alt+F4 did not deliver xdg_toplevel.close to focused A".into());
            }
            queue.roundtrip(&mut state)?;
        }
        println!("MELANIA_NATIVE_ALT_F4_REAL_XDG_CLOSE_PASS=true");
        std::io::stdout().flush()?;
    }
    // Optional Stage99 end-to-end experiment: keep both real xdg roles
    // connected, hide A via panel IPC and require a new DRM scanout with
    // its old pixels REMOVED. Then restore A with no new wl_shm commit and
    // require a second real scanout with its *retained* pixels visible.
    if std::env::var("MELANIA_QEMU_MANAGED_MINIMIZE_CLIENT")
        .as_deref()==Ok("YES_QEMU_VM") {
        if !managed_role_pixels {
            return Err("minimize pixel probe requires real managed xdg roles".into());
        }
        let socket=std::env::var("MELANIA_CONTROL_SOCKET")
            .map_err(|_|"managed minimize probe missing panel IPC socket")?;
        if control_request(&socket,"MINIMIZE 1\n")?!="OK\nEND\n" {
            return Err("native compositor rejected real MINIMIZE 1".into());
        }
        let minimized=control_request(&socket,"LIST\n")?;
        if !minimized.lines().any(|line|line.starts_with("WINDOW\t1\t0\t1\t")) {
            return Err("minimized A not reported hidden by native compositor".into());
        }
        await_presented_frame(13)?;
        println!("MELANIA_NATIVE_MANAGED_MINIMIZE_CLIENT_KMS13_PASS=true");
        std::io::stdout().flush()?;
        if control_request(&socket,"ACTIVATE 1\n")?!="OK\nEND\n" {
            return Err("native compositor rejected ACTIVATE of minimized A".into());
        }
        let restored=control_request(&socket,"LIST\n")?;
        if !restored.lines().any(|line|line.starts_with("WINDOW\t1\t1\t0\t")) {
            return Err("restored A not active in native compositor".into());
        }
        await_presented_frame(14)?;
        // Explicitly complete the real xdg configure exchange while
        // the Wayland surface is still alive; the old wl_shm pixels are
        // compositor-owned, not fabricated by the test.
        queue.roundtrip(&mut state)?;
        first.commit();
        connection.flush()?;
        println!("MELANIA_NATIVE_MANAGED_MINIMIZE_CLIENT_KMS14_PASS=true");
        std::io::stdout().flush()?;
    }
    println!("MELANIA_NATIVE_CONTINUOUS_TWO_WINDOW_12_FRAMES_PASS=true");
    if managed_role_pixels {
        if !state.resize_acked_a || state.resize_requested_a!=Some((W,H)) {
            return Err("final xdg unfullscreen restore configure was not ACKed".into());
        }
        println!("MELANIA_NATIVE_MANAGED_ROLE_12_KMS_FRAMES_PASS=true");
    }
    if dynamic {
        println!("MELANIA_NATIVE_DYNAMIC_TWO_SIZES_12_KMS_FRAMES_PASS=true");
        println!("MELANIA_NATIVE_DYNAMIC_PREMULTIPLIED_ARGB_CLIENT_PASS=true");
    }
    std::io::stdout().flush()?;
    if std::env::var("MELANIA_QEMU_DYNAMIC_POINTER_CLIENT").as_deref()==Ok("YES_QEMU_VM") {
        if !dynamic || !state.resize_acked_a || !state.resize_acked_b {
            return Err("real pointer proof requires two xdg resized clients".into());
        }
        // Keep both real Wayland surfaces connected until the QMP tablet
        // moves resized B and the actual virtual CRTC confirms frame13.
        let ack=std::path::Path::new("/run/melania-native-qemu/dynamic-pointer-frame13-acked");
        let deadline=Instant::now()+Duration::from_secs(24);
        while !ack.exists() {
            if Instant::now()>deadline {return Err("dynamic mouse KMS13 ACK absent".into());}
            thread::sleep(Duration::from_millis(25));
        }
        println!("MELANIA_NATIVE_DYNAMIC_POINTER_CLIENT_STAYED_ALIVE=true");
        std::io::stdout().flush()?;
    }
    let reopen=std::env::var("MELANIA_QEMU_CONTINUOUS_REOPEN_CLIENT")
        .as_deref()==Ok("YES_QEMU_VM");
    if reopen {
        // A true compositor-only update: B requests a frame callback, but
        // does NOT attach a new buffer. Destroying A must repaint its old
        // pixels away while keeping B's last real wl_shm pixels visible.
        let _survivor_callback=second.frame(&qh,());
        second.commit();
        first_top.destroy();
        first_xdg.destroy();
        first.destroy();
        connection.flush()?;
        let deadline=Instant::now()+Duration::from_secs(14);
        while state.frame_done_count<13 {
            queue.roundtrip(&mut state)?;
            if Instant::now()>=deadline {
                return Err("KMS close frame 13 never completed survivor callback".into());
            }
            thread::sleep(Duration::from_millis(12));
        }
        if state.frame_done_count!=13 {
            return Err("duplicate or premature continuous close frame callback".into());
        }
        println!("MELANIA_NATIVE_CONTINUOUS_CLOSE_FRAME13_ACKED=true");
        std::io::stdout().flush()?;

        // Create a DIFFERENT xdg_toplevel after the old A is gone. Smithay
        // must recycle stable slot 1 without moving the retained B window.
        state.xdg_acked=false;
        let reopened=compositor.create_surface(&qh,());
        let reopened_xdg=wm.get_xdg_surface(&reopened,&qh,3u8);
        let reopened_top=reopened_xdg.get_toplevel(&qh,3u8);
        reopened_top.set_title("Melania QEMU continuous reopened A".into());
        reopened.commit();
        for _ in 0..5 {
            queue.roundtrip(&mut state)?;
            if state.xdg_acked {break;}
        }
        if !state.xdg_acked {
            return Err("reopened client missing real xdg configure".into());
        }
        let mut pixels=vec![0u8;W*H*4];
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[160,80,190,0]);
        }
        let mut file=OpenOptions::new().create_new(true).read(true).write(true)
            .open("/run/melania-native-qemu/continuous-reopened-client14.bgra")?;
        file.write_all(&pixels)?;
        file.sync_all()?;
        let pool=shm.create_pool(file.as_fd(),pixels.len() as i32,&qh,());
        let buffer=pool.create_buffer(
            0,W as i32,H as i32,(W*4) as i32,wl_shm::Format::Xrgb8888,&qh,()
        );
        state.buffer_released=false;
        let _reopen_callback=reopened.frame(&qh,());
        reopened.attach(Some(&buffer),0,0);
        reopened.damage_buffer(0,0,W as i32,H as i32);
        reopened.commit();
        let deadline=Instant::now()+Duration::from_secs(14);
        while state.frame_done_count<14 || !state.buffer_released {
            queue.roundtrip(&mut state)?;
            if Instant::now()>=deadline {
                return Err("KMS reopen frame 14 never released real wl_shm client".into());
            }
            thread::sleep(Duration::from_millis(12));
        }
        if state.frame_done_count!=14 {
            return Err("duplicate or premature continuous reopened frame callback".into());
        }
        println!("MELANIA_NATIVE_CONTINUOUS_REOPEN_FRAME14_ACKED=true");
        println!("MELANIA_NATIVE_CONTINUOUS_REOPEN_14_FRAMES_PASS=true");
        std::io::stdout().flush()?;
        buffer.destroy();
        pool.destroy();
        reopened_top.destroy();
        reopened_xdg.destroy();
        reopened.destroy();
    } else {
        first_top.destroy();
        first_xdg.destroy();
        first.destroy();
    }
    second_top.destroy();
    second_xdg.destroy();
    second.destroy();
    wm.destroy();
    connection.flush()?;
    Ok(())
}
