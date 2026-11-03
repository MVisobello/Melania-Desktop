//! A REAL Wayland protocol client used only inside the disposable QEMU guest.
//! It creates an xdg_toplevel and wlr-layer-shell panel, receives configure
//! events, and acknowledges both. No window buffers or frame are displayed.
use std::{error::Error,io::Write};
use wayland_client::{
    Connection,Dispatch,QueueHandle,
    protocol::{wl_registry,wl_compositor,wl_surface},
};
use wayland_protocols::xdg::shell::client::{
    xdg_wm_base,xdg_surface,xdg_toplevel,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,zwlr_layer_surface_v1,
};

#[derive(Default)]
struct Probe {
    compositor:Option<wl_compositor::WlCompositor>,
    wm:Option<xdg_wm_base::XdgWmBase>,
    shell:Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    xdg_acked:bool,
    layer_acked:bool,
    server_closed_layer:bool,
    role_size:Option<(i32,i32)>,
    role_states:Vec<u32>,
}

impl Dispatch<wl_registry::WlRegistry,()> for Probe {
    fn event(state:&mut Self,registry:&wl_registry::WlRegistry,
             event:wl_registry::Event,_:&(),_:&Connection,qh:&QueueHandle<Self>) {
        if let wl_registry::Event::Global{name,interface,version}=event {
            match interface.as_str() {
                "wl_compositor"=>{
                    state.compositor=Some(registry.bind(
                        name,version.min(4),qh,()
                    ));
                }
                "xdg_wm_base"=>{
                    state.wm=Some(registry.bind(name,version.min(3),qh,()));
                }
                "zwlr_layer_shell_v1"=>{
                    state.shell=Some(registry.bind(name,version.min(1),qh,()));
                }
                _=>{}
            }
        }
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase,()> for Probe {
    fn event(state:&mut Self,wm:&xdg_wm_base::XdgWmBase,
             event:xdg_wm_base::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping{serial}=event {wm.pong(serial);}
    }
}
impl Dispatch<xdg_surface::XdgSurface,()> for Probe {
    fn event(state:&mut Self,surface:&xdg_surface::XdgSurface,
             event:xdg_surface::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_surface::Event::Configure{serial}=event {
            surface.ack_configure(serial);
            state.xdg_acked=true;
        }
    }
}
impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,()> for Probe {
    fn event(state:&mut Self,surface:&zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
             event:zwlr_layer_surface_v1::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        match event {
            zwlr_layer_surface_v1::Event::Configure{serial,..}=>{
                surface.ack_configure(serial);
                state.layer_acked=true;
            }
            zwlr_layer_surface_v1::Event::Closed=>state.server_closed_layer=true,
            _=>{}
        }
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel,()> for Probe {
    fn event(state:&mut Self,_:&xdg_toplevel::XdgToplevel,event:xdg_toplevel::Event,
              _:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_toplevel::Event::Configure{width,height,states}=event {
            state.role_size=Some((width,height));
            state.role_states=states.chunks_exact(4)
                .map(|bytes|u32::from_ne_bytes([bytes[0],bytes[1],bytes[2],bytes[3]]))
                .collect();
        }
    }
}
// wl_compositor, wl_surface and layer_shell have no incoming events here.
impl Dispatch<wl_compositor::WlCompositor,()> for Probe {
    fn event(state:&mut Self,_:&wl_compositor::WlCompositor,_:wl_compositor::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_surface::WlSurface,()> for Probe {
    fn event(state:&mut Self,_:&wl_surface::WlSurface,_:wl_surface::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1,()> for Probe {
    fn event(state:&mut Self,_:&zwlr_layer_shell_v1::ZwlrLayerShellV1,
             _:zwlr_layer_shell_v1::Event,_:&(),_:&Connection,_:&QueueHandle<Self>){}
}
fn expect_managed_role(
    events:&mut wayland_client::EventQueue<Probe>,
    state:&mut Probe,
    size:(i32,i32),
    required:Option<xdg_toplevel::State>,
    forbidden:Option<xdg_toplevel::State>,
)->Result<(),Box<dyn Error>> {
    state.xdg_acked=false;
    for _ in 0..8 {
        events.roundtrip(state)?;
        if state.xdg_acked {break;}
    }
    if !state.xdg_acked {return Err("managed xdg configure was not ACKed".into());}
    if state.role_size!=Some(size) {
        return Err(format!("managed xdg configure size {:?}, expected {:?}",
                           state.role_size,size).into());
    }
    if required.is_some_and(|v|!state.role_states.contains(&(v as u32))) {
        return Err("managed xdg configure missing required window state".into());
    }
    if forbidden.is_some_and(|v|state.role_states.contains(&(v as u32))) {
        return Err("managed xdg configure advertised incompatible window state".into());
    }
    Ok(())
}

fn main()->Result<(),Box<dyn Error>> {
    let connection=Connection::connect_to_env()?;
    let mut events=connection.new_event_queue::<Probe>();
    let qh=events.handle();
    let display=connection.display();
    let _registry=display.get_registry(&qh,());
    let mut state=Probe::default();
    events.roundtrip(&mut state)?;
    let compositor=state.compositor.take().ok_or("wl_compositor missing")?;
    let wm=state.wm.take().ok_or("xdg_wm_base missing")?;
    let shell=state.shell.take().ok_or("zwlr_layer_shell_v1 missing")?;

    let window_surface=compositor.create_surface(&qh,());
    let xdg=wm.get_xdg_surface(&window_surface,&qh,());
    let top=xdg.get_toplevel(&qh,());
    top.set_title("Melania Native QEMU xdg-shell protocol probe".into());
    window_surface.commit();

    let panel_surface=compositor.create_surface(&qh,());
    let panel=shell.get_layer_surface(
        &panel_surface,None,zwlr_layer_shell_v1::Layer::Top,
        "melania-qemu-layer-probe".into(),&qh,()
    );
    panel.set_size(640,36);
    panel.set_anchor(zwlr_layer_surface_v1::Anchor::Top);
    panel_surface.commit();
    for _ in 0..6 {
        events.roundtrip(&mut state)?;
        if state.xdg_acked && state.layer_acked {break;}
    }
    if !state.xdg_acked || !state.layer_acked || state.server_closed_layer {
        return Err("xdg/layer configure ack roundtrip failed".into());
    }
    // Flush both ack_configure requests and check the server accepts them.
    window_surface.commit();
    panel_surface.commit();
    events.roundtrip(&mut state)?;
    if std::env::var("MELANIA_NATIVE_MANAGED_XDG").as_deref()==Ok("YES_QEMU_VM") {
        // A real client, not a synthetic reducer-only test.
        top.set_maximized();
        expect_managed_role(&mut events,&mut state,(1280,800),
            Some(xdg_toplevel::State::Maximized),Some(xdg_toplevel::State::Fullscreen))?;
        window_surface.commit();
        top.unset_maximized();
        expect_managed_role(&mut events,&mut state,(360,224),
            None,Some(xdg_toplevel::State::Maximized))?;
        window_surface.commit();
        top.set_fullscreen(None);
        expect_managed_role(&mut events,&mut state,(1280,800),
            Some(xdg_toplevel::State::Fullscreen),Some(xdg_toplevel::State::Maximized))?;
        window_surface.commit();
        top.unset_fullscreen();
        expect_managed_role(&mut events,&mut state,(360,224),
            None,Some(xdg_toplevel::State::Fullscreen))?;
        window_surface.commit();
        // xdg_toplevel has no unminimize request; restoring a hidden
        // window is the shell/taskbar's responsibility in a later stage.
        top.set_minimized();
        events.roundtrip(&mut state)?;
        println!("MELANIA_NATIVE_XDG_MINIMIZE_REQUEST_SENT=true");
        println!("MELANIA_NATIVE_XDG_ROLE_ROUNDTRIP_PASS=true");
    }
    println!("MELANIA_NATIVE_XDG_CONFIGURE_ACK=true");
    println!("MELANIA_NATIVE_LAYER_CONFIGURE_ACK=true");
    println!("MELANIA_NATIVE_SHELL_PROTOCOL_CLIENT_PASS=true");
    std::io::stdout().flush()?;

    // Destroy roles before surfaces in protocol-correct order.
    panel.destroy();
    panel_surface.destroy();
    top.destroy();
    xdg.destroy();
    window_surface.destroy();
    shell.destroy();
    wm.destroy();
    drop(compositor);
    connection.flush()?;
    Ok(())
}
