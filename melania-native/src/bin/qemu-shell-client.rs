//! A REAL Wayland protocol client used only inside the disposable QEMU guest.
//! It creates an xdg_toplevel and wlr-layer-shell panel, receives configure
//! events, and acknowledges both. No window buffers or frame are displayed.
use std::{error::Error,io::Write};
use wayland_client::{
    Connection,Dispatch,QueueHandle,Proxy,
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
}

impl Dispatch<wl_registry::WlRegistry,()> for Probe {
    fn event(&mut self,registry:&wl_registry::WlRegistry,
             event:wl_registry::Event,_:&(),_:&Connection,qh:&QueueHandle<Self>) {
        if let wl_registry::Event::Global{name,interface,version}=event {
            match interface.as_str() {
                "wl_compositor"=>{
                    self.compositor=Some(registry.bind(
                        name,version.min(4),qh,()
                    ));
                }
                "xdg_wm_base"=>{
                    self.wm=Some(registry.bind(name,version.min(3),qh,()));
                }
                "zwlr_layer_shell_v1"=>{
                    self.shell=Some(registry.bind(name,version.min(1),qh,()));
                }
                _=>{}
            }
        }
    }
}
impl Dispatch<xdg_wm_base::XdgWmBase,()> for Probe {
    fn event(&mut self,wm:&xdg_wm_base::XdgWmBase,
             event:xdg_wm_base::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_wm_base::Event::Ping{serial}=event {wm.pong(serial);}
    }
}
impl Dispatch<xdg_surface::XdgSurface,()> for Probe {
    fn event(&mut self,surface:&xdg_surface::XdgSurface,
             event:xdg_surface::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        if let xdg_surface::Event::Configure{serial}=event {
            surface.ack_configure(serial);
            self.xdg_acked=true;
        }
    }
}
impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,()> for Probe {
    fn event(&mut self,surface:&zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
             event:zwlr_layer_surface_v1::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        match event {
            zwlr_layer_surface_v1::Event::Configure{serial,..}=>{
                surface.ack_configure(serial);
                self.layer_acked=true;
            }
            zwlr_layer_surface_v1::Event::Closed=>self.server_closed_layer=true,
            _=>{}
        }
    }
}
impl Dispatch<xdg_toplevel::XdgToplevel,()> for Probe {
    fn event(&mut self,_:&xdg_toplevel::XdgToplevel,_:xdg_toplevel::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
// wl_compositor, wl_surface and layer_shell have no incoming events here.
impl Dispatch<wl_compositor::WlCompositor,()> for Probe {
    fn event(&mut self,_:&wl_compositor::WlCompositor,_:wl_compositor::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_surface::WlSurface,()> for Probe {
    fn event(&mut self,_:&wl_surface::WlSurface,_:wl_surface::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1,()> for Probe {
    fn event(&mut self,_:&zwlr_layer_shell_v1::ZwlrLayerShellV1,
             _:zwlr_layer_shell_v1::Event,_:&(),_:&Connection,_:&QueueHandle<Self>){}
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
    compositor.destroy();
    connection.flush()?;
    Ok(())
}
