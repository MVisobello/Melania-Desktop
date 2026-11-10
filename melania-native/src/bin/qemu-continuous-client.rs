//! Stage 46: real Wayland wl_shm updates beyond the old sixth frame.
//! VM-only scenario; every buffer release and callback requires a KMS ACK.
use std::{error::Error,fs::OpenOptions,io::Write,os::fd::{AsFd,OwnedFd},thread,time::{Duration,Instant}};
use melania_native::native_frame_budget::frame_budget;
use wayland_client::{
    Connection,Dispatch,QueueHandle,
    protocol::{wl_registry,wl_compositor,wl_surface,wl_shm,wl_shm_pool,wl_buffer,wl_output,wl_callback},
};
use wayland_protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_dmabuf_v1,zwp_linux_buffer_params_v1,
};
use smithay::{
    backend::{
        allocator::{
            gbm::{GbmDevice,GbmAllocator,GbmBufferFlags},
            dmabuf::DmabufAllocator,
            Allocator,Fourcc,Modifier,
        },
        drm::DrmDeviceFd,
        egl::{EGLContext,EGLDisplay},
        renderer::{gles::GlesRenderer,Bind,Color32F,Frame,Renderer},
    },
    utils::{DeviceFd,Rectangle,Transform},
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
    dmabuf:Option<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1>,
    dmabuf_formats:Vec<(u32,u64)>,
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
                "zwp_linux_dmabuf_v1" if
                    std::env::var("MELANIA_QEMU_CONTINUOUS_CLIENT_DMABUF")
                        .as_deref()==Ok("YES_QEMU_VM") =>
                    state.dmabuf=Some(registry.bind(name,version.min(3),qh,())),
                _=>{}
            }
        }
    }
}
impl Dispatch<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,()> for Client {
    fn event(state:&mut Self,_:&zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
             event:zwp_linux_dmabuf_v1::Event,_:&(),_:&Connection,
             _:&QueueHandle<Self>) {
        match event {
            zwp_linux_dmabuf_v1::Event::Modifier{format,modifier_hi,modifier_lo}=>{
                state.dmabuf_formats.push((
                    format,(u64::from(modifier_hi)<<32)|u64::from(modifier_lo),
                ));
            },
            zwp_linux_dmabuf_v1::Event::Format{..}=>{},
            _=>{},
        }
    }
}
impl Dispatch<zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,()> for Client {
    fn event(_:&mut Self,_:&zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
             _:zwp_linux_buffer_params_v1::Event,_:&(),_:&Connection,
             _:&QueueHandle<Self>){}
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

/// A *different Wayland process* allocates and draws real GBM GPU pixels,
/// sends its live kernel DMA-BUF plane FDs over zwp_linux_dmabuf_v1, then
/// attaches the buffer to a role-less wl_surface. The Smithay compositor
/// must import it via EGL, reject it for KMS presentation (still unimplemented),
/// and send wl_buffer.release WITHOUT producing any wl_surface.frame ACK.
/// This avoids claiming native DMA-BUF scanout before it exists.
fn exercise_real_client_dmabuf(
    state:&mut Client,queue:&mut wayland_client::EventQueue<Client>,
    qh:&QueueHandle<Client>,compositor:&wl_compositor::WlCompositor,
    xdg_surface:&wl_surface::WlSurface,
)->Result<(),Box<dyn Error>> {
    let dmabuf=state.dmabuf.take()
        .ok_or("QEMU Smithay server did not advertise zwp_linux_dmabuf_v1")?;
    let modifier:u64=Modifier::Linear.into();
    let format=Fourcc::Argb8888;
    if !state.dmabuf_formats.contains(&(format as u32,modifier)) {
        return Err("QEMU Wayland compositor did not advertise ARGB8888+LINEAR".into());
    }

    // This disposable QEMU VirtIO GPU allocates BOs through
    // DRM_IOCTL_MODE_CREATE_DUMB, a KMS allocation ioctl which is explicitly
    // NOT permitted on renderD128. The SECOND process opens its own
    // unprivileged primary FD; CREATE_DUMB does not require DRM_MASTER.
    // Never request/set KMS master or perform modesets from the client.
    // Do not silently fall back to the render node: that cannot allocate
    // this VM's dumb-backed GBM buffers.
    let file=OpenOptions::new().read(true).write(true)
        .open("/dev/dri/card0")?;
    let gbm=GbmDevice::new(DrmDeviceFd::new(DeviceFd::from(OwnedFd::from(file))))?;
    let mut allocator=DmabufAllocator(GbmAllocator::new(
        gbm.clone(),GbmBufferFlags::RENDERING,
    ));
    let mut image=allocator.create_buffer(
        W as u32,H as u32,format,&[Modifier::Linear],
    )?;
    // Supply real nontrivial content and wait on the GPU before giving the
    // cross-process FD to the compositor. This is not a synthetic wl_shm pool.
    let display=unsafe {EGLDisplay::new(gbm)}?;
    let context=EGLContext::new(&display)?;
    let mut renderer=unsafe {GlesRenderer::new(context)}?;
    {
        let mut target=renderer.bind(&mut image)?;
        let mut frame=renderer.render(
            &mut target,(W as i32,H as i32).into(),Transform::Normal
        )?;
        frame.clear(
            Color32F::new(0.33,0.18,0.73,1.0),
            &[Rectangle::from_size((W as i32,H as i32).into())],
        )?;
        frame.finish()?.wait()?;
    }

    let params=dmabuf.create_params(qh,());
    let mut plane_count=0usize;
    for ((fd,offset),stride) in image.handles().zip(image.offsets()).zip(image.strides()) {
        params.add(fd,plane_count as u32,offset,stride,
                   (modifier>>32) as u32,modifier as u32);
        plane_count+=1;
    }
    if plane_count!=1 {
        return Err("QEMU client GPU produced a non-packed DMA-BUF layout".into());
    }
    let imported=params.create_immed(
        W as i32,H as i32,format as u32,
        zwp_linux_buffer_params_v1::Flags::empty(),qh,(),
    );
    let gpu_kms=std::env::var("MELANIA_QEMU_CONTINUOUS_CLIENT_DMABUF_KMS")
        .as_deref()==Ok("YES_QEMU_VM");
    // One real XDG client keeps the *same* scene slot for frame 1 GPU and
    // frames 2..12 wl_shm. The negative non-KMS import test continues to
    // use a separate role-less surface, with no fake callback.
    let probe=if gpu_kms {xdg_surface.clone()}
        else {compositor.create_surface(qh,())};
    if gpu_kms {
        // Frame callback is delivered ONLY after the matching real CRTC
        // FB_ID/vblank ACK, never after the compositor's EGL texture import.
        let _callback=probe.frame(qh,());
    }
    state.buffer_released=false;
    probe.attach(Some(&imported),0,0);
    probe.damage_buffer(0,0,W as i32,H as i32);
    probe.commit();
    let deadline=Instant::now()+Duration::from_secs(10);
    while !state.buffer_released ||
          (gpu_kms && state.frame_done_count!=1) {
        queue.roundtrip(state)?;
        if Instant::now()>=deadline {
            return Err("real client DMA-BUF missing verified GPU-to-KMS release or callback".into());
        }
    }
    if gpu_kms {
        if state.frame_done_count!=1 {
            return Err("GPU DMA-BUF callback not paired with verified KMS ACK".into());
        }
        println!("MELANIA_QEMU_EXTERNAL_CLIENT_DMABUF_FRAME1_KMS_ACKED=true");
        println!("MELANIA_NATIVE_CONTINUOUS_CLIENT_ACKED=1");
    } else {
        if state.frame_done_count!=0 {
            return Err("client DMA-BUF received a fabricated KMS frame callback".into());
        }
        println!("MELANIA_QEMU_EXTERNAL_CLIENT_DMABUF_RELEASED_NO_KMS=true");
    }
    std::io::stdout().flush()?;
    imported.destroy();
    if !gpu_kms {probe.destroy();}
    params.destroy();
    dmabuf.destroy();
    Ok(())
}

fn main()->Result<(),Box<dyn Error>> {
    if std::env::var("MELANIA_QEMU_CONTINUOUS_CLIENT").as_deref()!=Ok("YES_QEMU_VM") {
        return Err("QEMU continuous Wayland client opt-in required".into());
    }
    let connection=Connection::connect_to_env()?;
    let mut queue=connection.new_event_queue::<Client>();
    let qh=queue.handle();
    let _registry=connection.display().get_registry(&qh,());
    let mut state=Client::default();
    queue.roundtrip(&mut state)?;
    queue.roundtrip(&mut state)?;
    if !state.output_mode_received {
        return Err("continuous client did not see 1280x800 Wayland output".into());
    }
    let compositor=state.compositor.take().ok_or("wl_compositor missing")?;
    let wm=state.wm.take().ok_or("xdg_wm_base missing")?;
    let shm=state.shm.take().ok_or("wl_shm missing")?;
    let surface=compositor.create_surface(&qh,());
    let xdg=wm.get_xdg_surface(&surface,&qh,());
    let toplevel=xdg.get_toplevel(&qh,());
    toplevel.set_title("Melania QEMU continuous real Wayland client".into());
    surface.commit();
    for _ in 0..5 {
        queue.roundtrip(&mut state)?;
        if state.xdg_acked {break;}
    }
    if !state.xdg_acked {return Err("continuous client xdg configure missing".into());}
    if std::env::var("MELANIA_QEMU_CONTINUOUS_CLIENT_DMABUF")
        .as_deref()==Ok("YES_QEMU_VM") {
        exercise_real_client_dmabuf(&mut state,&mut queue,&qh,&compositor,&surface)?;
    }
    let total=frame_budget(
        std::env::var("MELANIA_QEMU_CONTINUOUS_CLIENT_FRAMES").ok().as_deref(),12
    )? as u32;
    // Only the Stage110 disposable-QEMU proof deliberately suspends client
    // commits and resumes afterwards. No callback may be synthesized in the
    // idle interval, and all resumed commits still require a real KMS ACK.
    let pause_after=std::env::var("MELANIA_QEMU_CONTINUOUS_CLIENT_IDLE_AFTER").ok();
    let pause_for_gpu=match pause_after.as_deref() {
        None=>false,
        Some("24") if total==48 &&
            std::env::var("MELANIA_QEMU_CONTINUOUS_CLIENT").as_deref()==Ok("YES_QEMU_VM")=>
            true,
        Some(_)=>return Err("GPU Wayland idle-resume proof requires exactly 48 QEMU client frames".into()),
    };
    // A real desktop must survive a second wl_surface.commit with pixels
    // equal to the preceding already-presented frame. This opt-in is valid
    // only in the persistent 48-frame GPU experiment with its 15s idle gap.
    let replay_same=match std::env::var("MELANIA_QEMU_CONTINUOUS_CLIENT_IDENTICAL_AFTER").ok().as_deref() {
        None=>false,
        Some("24") if pause_for_gpu=>true,
        Some(_)=>return Err("identical Wayland replay requires persistent QEMU GPU idle proof".into()),
    };
    let gpu_first=std::env::var("MELANIA_QEMU_CONTINUOUS_CLIENT_DMABUF_KMS")
        .as_deref()==Ok("YES_QEMU_VM");
    if gpu_first && (
        std::env::var("MELANIA_QEMU_CONTINUOUS_CLIENT_DMABUF")
            .as_deref()!=Ok("YES_QEMU_VM") || total!=12
    ) {
        return Err("GPU client scanout proof requires exact 12-frame QEMU DMA-BUF mode".into());
    }
    for index in 1..=total {
        // Preserve the historic 1..=total contract for every normal
        // Wayland session. Only the isolated DMA-BUF test has already
        // presented index 1 with a real kernel KMS ACK.
        if gpu_first && index==1 {continue;}
        // Only the opt-in frame 25 repeats frame 24 exactly; all other
        // frames change the client's *real* owned wl_shm buffer.
        let color_index=if replay_same && index==25 {24} else {index};
        let mut pixels=vec![0u8;W*H*4];
        let red=(30+(color_index*17)%200) as u8;
        let green=(40+(color_index*31)%190) as u8;
        let blue=(25+(color_index*43)%210) as u8;
        for pixel in pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[blue,green,red,0]);
        }
        // Each Wayland client invocation must use its own wl_shm backing file.
        // The guest runs sequential legacy and in-process KMS clients in the
        // same /run directory; create_new with a fixed name fails on the
        // second run even if the previous compositor exited cleanly.
        let path=format!("/run/melania-native-qemu/continuous-client-{}-{index}.bgra",
                         std::process::id());
        let mut file=OpenOptions::new().create_new(true).read(true).write(true)
            .open(&path)?;
        file.write_all(&pixels)?;
        file.sync_all()?;
        let pool=shm.create_pool(file.as_fd(),pixels.len() as i32,&qh,());
        // The wl_shm pool retains the FD; the pathname is not needed by
        // either process. Unlink immediately to avoid stale files between
        // consecutive QEMU scenarios and on client disconnect.
        std::fs::remove_file(&path)?;
        let buffer=pool.create_buffer(
            0,W as i32,H as i32,(W*4) as i32,wl_shm::Format::Xrgb8888,&qh,()
        );
        state.buffer_released=false;
        let _callback=surface.frame(&qh,());
        surface.attach(Some(&buffer),0,0);
        surface.damage_buffer(0,0,W as i32,H as i32);
        surface.commit();
        let deadline=Instant::now()+Duration::from_secs(10);
        while !state.buffer_released || state.frame_done_count<index {
            queue.roundtrip(&mut state)?;
            if Instant::now()>=deadline {
                return Err(format!("continuous client frame {index} missing KMS callback").into());
            }
            thread::sleep(Duration::from_millis(12));
        }
        if state.frame_done_count!=index {
            return Err("continuous client received duplicate frame callback".into());
        }
        println!("MELANIA_NATIVE_CONTINUOUS_CLIENT_ACKED={index}");
        if replay_same && index==25 {
            println!("MELANIA_NATIVE_GPU_WAYLAND_IDENTICAL_CLIENT_FRAME25_ACKED=true");
        }
        std::io::stdout().flush()?;
        buffer.destroy();
        pool.destroy();
        if pause_for_gpu && index==24 {
            println!("MELANIA_NATIVE_GPU_WAYLAND_CLIENT_IDLE_AFTER_24=true");
            std::io::stdout().flush()?;
            // 15s exceeds the historical 12s DRM socket deadline. The
            // compositor MUST remain healthy and accept frame 25 afterward.
            thread::sleep(Duration::from_secs(15));
            println!("MELANIA_NATIVE_GPU_WAYLAND_CLIENT_RESUMED_AFTER_IDLE=true");
            std::io::stdout().flush()?;
        }
    }
    if total==12 {
        println!("MELANIA_NATIVE_CONTINUOUS_CLIENT_12_FRAMES_PASS=true");
    } else {
        println!("MELANIA_NATIVE_CONTINUOUS_CLIENT_EXTENDED_FRAMES_PASS={total}");
    }
    std::io::stdout().flush()?;
    toplevel.destroy();
    xdg.destroy();
    surface.destroy();
    wm.destroy();
    connection.flush()?;
    Ok(())
}
