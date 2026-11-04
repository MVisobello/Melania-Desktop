//! QEMU-only *real protocol* proof of all five GTK layer-shell namespaces.
//! Uses real libwayland, but does NOT run GTK, send pixel buffers, or show UI.
use std::{error::Error,io::Write};
use wayland_client::{Connection,Dispatch,QueueHandle,
    protocol::{wl_registry,wl_compositor,wl_surface}};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1,zwlr_layer_surface_v1};

const NAMES:[&str;5]=[
    "melania-panel","melania-start","melania-quick",
    "melania-notifications","melania-calendar",
];
const SIZES:[(u32,u32);5]=[
    (1280,52),(510,565),(350,430),(380,440),(330,315),
];

#[derive(Default)]
struct Probe {
    compositor:Option<wl_compositor::WlCompositor>,
    shell:Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    acknowledged:[bool;5],
    error:Option<String>,
}
impl Dispatch<wl_registry::WlRegistry,()> for Probe {
    fn event(s:&mut Self,r:&wl_registry::WlRegistry,e:wl_registry::Event,
             _:&(),_:&Connection,q:&QueueHandle<Self>) {
        if let wl_registry::Event::Global{name,interface,version}=e {
            match interface.as_str() {
                "wl_compositor"=>s.compositor=Some(r.bind(name,version.min(4),q,())),
                "zwlr_layer_shell_v1"=>s.shell=Some(r.bind(name,version.min(1),q,())),
                _=>{},
            }
        }
    }
}
impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,usize> for Probe {
    fn event(s:&mut Self,role:&zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
             e:zwlr_layer_surface_v1::Event,id:&usize,
             _:&Connection,_:&QueueHandle<Self>) {
        match e {
            zwlr_layer_surface_v1::Event::Configure{serial,width,height}=>{
                if *id>=NAMES.len() {
                    s.error=Some("unknown GTK role index".into());
                    return;
                }
                if (width,height)!=SIZES[*id] {
                    s.error=Some(format!(
                        "GTK role {} configured {}x{}, expected {:?}",
                        NAMES[*id],width,height,SIZES[*id]));
                    return;
                }
                role.ack_configure(serial);
                s.acknowledged[*id]=true;
            }
            zwlr_layer_surface_v1::Event::Closed=>{
                s.error=Some(format!("GTK role {} closed by compositor",NAMES[*id]));
            }
            _=>{},
        }
    }
}
impl Dispatch<wl_compositor::WlCompositor,()> for Probe {
    fn event(_: &mut Self,_:&wl_compositor::WlCompositor,_:wl_compositor::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<wl_surface::WlSurface,()> for Probe {
    fn event(_: &mut Self,_:&wl_surface::WlSurface,_:wl_surface::Event,
             _:&(),_:&Connection,_:&QueueHandle<Self>){}
}
impl Dispatch<zwlr_layer_shell_v1::ZwlrLayerShellV1,()> for Probe {
    fn event(_: &mut Self,_:&zwlr_layer_shell_v1::ZwlrLayerShellV1,
             _:zwlr_layer_shell_v1::Event,_:&(),_:&Connection,_:&QueueHandle<Self>){}
}

fn main()->Result<(),Box<dyn Error>> {
    if std::env::var("MELANIA_QEMU_GTK_FIVE_ROLES").as_deref()!=Ok("YES_QEMU_VM") {
        return Err("explicit disposable QEMU role-only opt-in required".into());
    }
    let conn=Connection::connect_to_env()?;
    let mut queue=conn.new_event_queue::<Probe>();
    let q=queue.handle();
    let _registry=conn.display().get_registry(&q,());
    let mut state=Probe::default();
    queue.roundtrip(&mut state)?;
    let compositor=state.compositor.take().ok_or("wl_compositor missing")?;
    let shell=state.shell.take().ok_or("wlr-layer-shell missing")?;
    let mut owned=Vec::new();
    for (id,name) in NAMES.iter().enumerate() {
        let surface=compositor.create_surface(&q,());
        let layer=if id==0 {zwlr_layer_shell_v1::Layer::Top}
                  else {zwlr_layer_shell_v1::Layer::Overlay};
        let role=shell.get_layer_surface(&surface,None,layer,(*name).to_string(),&q,id);
        let (width,height)=SIZES[id];
        if id==0 {
            // Exactly how the GTK taskbar requests opposing-edge stretch.
            role.set_anchor(zwlr_layer_surface_v1::Anchor::Bottom |
                            zwlr_layer_surface_v1::Anchor::Left |
                            zwlr_layer_surface_v1::Anchor::Right);
            role.set_size(0,height);
            role.set_exclusive_zone(52);
        } else if id==1 {
            role.set_anchor(zwlr_layer_surface_v1::Anchor::Bottom |
                            zwlr_layer_surface_v1::Anchor::Left);
            role.set_size(width,height);
            role.set_margin(0,0,6,8);
            role.set_exclusive_zone(0);
        } else {
            role.set_anchor(zwlr_layer_surface_v1::Anchor::Bottom |
                            zwlr_layer_surface_v1::Anchor::Right);
            role.set_size(width,height);
            let bottom=if id==4 {58} else {65};
            role.set_margin(0,12,bottom,0);
            role.set_exclusive_zone(0);
        }
        surface.commit();
        owned.push((role,surface));
    }
    for _ in 0..12 {
        queue.roundtrip(&mut state)?;
        if let Some(error)=state.error.take(){return Err(error.into());}
        if state.acknowledged.iter().all(|value|*value){break;}
    }
    if !state.acknowledged.iter().all(|value|*value) {
        return Err(format!("missing layer configure ACK: {:?}",state.acknowledged).into());
    }
    // Push actual ACK requests through the wire and observe the server.
    queue.roundtrip(&mut state)?;
    if let Some(error)=state.error.take(){return Err(error.into());}
    for (id,name) in NAMES.iter().enumerate() {
        println!("MELANIA_QEMU_GTK_ROLE_CONFIGURE_ACK={name} size={}x{}",
                 SIZES[id].0,SIZES[id].1);
    }
    println!("MELANIA_QEMU_GTK_FIVE_ROLE_CONFIGURES_PASS=true");
    std::io::stdout().flush()?;
    for (role,surface) in owned {
        role.destroy();
        surface.destroy();
    }
    shell.destroy();
    conn.flush()?;
    Ok(())
}
