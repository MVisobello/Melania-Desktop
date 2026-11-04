//! QEMU-only native Wayland seat/keyboard/pointer bridge.
 //!
 //! This is a real Smithay wl_compositor + wl_seat endpoint receiving libinput
 //! from libseat in a dedicated VM console, NOT a full graphical compositor.
 //! A client can create a wl_surface and receive keyboard/pointer events, but
 //! KMS scanout of client buffers and native login remain OFF. A separate
//! QEMU-only shell mode checks real xdg_toplevel and layer-surface configure
//! roundtrips without claiming these clients are displayed.
use std::{
    error::Error,
    io::{Read,Write},
    os::unix::net::{UnixListener,UnixStream},
    sync::Arc,
    time::{Duration,Instant},
};
use smithay::{
    backend::{
        input::{InputEvent, ButtonState, KeyboardKeyEvent, PointerButtonEvent, PointerMotionEvent,
                AbsolutePositionEvent, Event},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Session, Event as SessionEvent},
    },
    delegate_compositor, delegate_seat, delegate_xdg_shell, delegate_layer_shell, delegate_shm, delegate_output,
    input::{keyboard::FilterResult, pointer::{ButtonEvent, CursorImageStatus,
           MotionEvent}, Seat, SeatHandler, SeatState},
    reexports::{
        input::{self as libinput, DeviceCapability, Libinput},
        wayland_server::{
            backend::{ClientData,ClientId,DisconnectReason},
            protocol::{wl_buffer,wl_surface::WlSurface,wl_seat,wl_output::WlOutput},
            Client, Display, ListeningSocket,
        },
    },
    utils::{Logical, Point, Serial, Transform, SERIAL_COUNTER},
    output::{Mode, Output, PhysicalProperties, Subpixel},
    wayland::{
        buffer::BufferHandler,
        compositor::{with_states,with_surface_tree_downward,TraversalAction,BufferAssignment,SurfaceAttributes,CompositorClientState,CompositorHandler,CompositorState},
        shm::{ShmState,ShmHandler,with_buffer_contents},
        output::{OutputHandler,OutputManagerState},
        shell::{xdg::{XdgShellState,XdgShellHandler,ToplevelSurface,PopupSurface,PositionerState},
                wlr_layer::{WlrLayerShellState,WlrLayerShellHandler,
                           LayerSurface as WlrLayerSurface,Layer as WlrLayer,
                           Anchor as WlrAnchor,LayerSurfaceCachedState}},
    },
};
use melania_native::layer_surface_layout::{self,Anchors,Insets,LayerRequest,ShellLayer};
use melania_native::gtk_layer_geometry;
use melania_native::scene::Layer as SceneLayer;
use melania_native::{NativeLifecycle,render_scheduler::{FrameTicket,RenderScheduler},continuous_redraw::{ContinuousRedraw,PlannedSceneFrame},continuous_kms_stream::{ContinuousKmsWorker,ContinuousKmsJob},client_commit_ledger::{ClientCommitLedger,MAX_PENDING_CLIENTS},multi_window_scene::MultiWindowScene,dynamic_window_scene::{DynamicWindowScene,WindowPixels},window_slots::WindowSlots,async_kms_stream::{AsyncKmsWorker,FrameJob},window_interaction::{WindowInteraction,WindowRect,InteractionKind},managed_window::{ManagedWindow,WindowDisplayState}};
use smithay::backend::input::Device as InputDevice;
use wayland_protocols::xdg::shell::server::xdg_toplevel;
use crate::native_client_frame;

#[derive(Default)]
struct BridgeClientState {
    compositor: CompositorClientState,
}
impl ClientData for BridgeClientState {
    fn initialized(&self,_:ClientId) {}
    fn disconnected(&self,_:ClientId,_:DisconnectReason) {}
}

struct BridgeState {
    compositor: CompositorState,
    shm:ShmState,
    output:Output,
    output_manager:OutputManagerState,
    pixel_probe:bool,
    pixel_ready:bool,
    live_stream:bool,
    frame_callbacks_probe:bool,
    frame_callback_count:u32,
    render_scheduler:RenderScheduler,
    continuous_redraw:ContinuousRedraw,
    continuous_pending:Option<PlannedSceneFrame>,
    // Opt-in, disposable QEMU continuous renderer; the legacy proof is intact.
    continuous_kms_mode:bool,
    continuous_worker:Option<ContinuousKmsWorker>,
    // Keep one pending wl_shm buffer per real client surface, not globally.
    continuous_inflight:Option<(PlannedSceneFrame,Vec<(WlSurface,wl_buffer::WlBuffer)>)>,
    continuous_client_ledger:ClientCommitLedger<WlSurface,wl_buffer::WlBuffer>,
    continuous_frame_count:u64,
    continuous_frame_target:u64,
    continuous_reopen_mode:bool,
    continuous_close_frame_presented:bool,
    continuous_reopen_frame_presented:bool,
    continuous_two_window_presented:bool,
    multi_window_mode:bool,
    async_kms_mode:bool,
    drag_redraw_mode:bool,
    close_redraw_mode:bool,
    close_frame_presented:bool,
    reopen_redraw_mode:bool,
    reopen_frame_presented:bool,
    presented_layout_generation:u64,
    // Store proof of a successful KMS presentation separately from the
    // current scene: a client may destroy its toplevel after the frame ACK.
    drag_frame_presented:bool,
    two_window_frame_presented:bool,
    async_stream:Option<AsyncKmsWorker>,
    async_pending:Option<PendingAsyncFrame>,
    multi_scene:MultiWindowScene,
    // Stage53 is opt-in within disposable QEMU. The original two fixed
    // window scene and its verified KMS experiments remain unchanged.
    dynamic_window_mode:bool,
    dynamic_scene:DynamicWindowScene,
    // Stage62 only reuses pixels from the last verified CRTC ACK;
    // an in-flight framebuffer is never used as a partial-paint base.
    dynamic_confirmed_frame:Option<Vec<u8>>,
    dynamic_submitted_frame:Option<Vec<u8>>,
    // Explicit, VM-only xdg_toplevel configure/ack resize protocol.
    // Unrelated nested Wayland applications use their existing policy.
    dynamic_xdg_resize_mode:bool,
    // Real QMP tablet drag AFTER xdg resize; needs one compositor-only KMS frame.
    dynamic_pointer_proof_mode:bool,
    dynamic_toplevels:Vec<ToplevelSurface>,
    // Opt-in VM-only bridge of real xdg_toplevel requests to Stage77.
    managed_xdg_mode:bool,
    managed_windows:Vec<(WlSurface,ManagedWindow)>,
    // A configure alone is not a displayed resize: wait for the matching
    // wl_shm client image before changing the retained KMS scene.
    managed_pending_sizes:Vec<(WlSurface,(usize,usize))>,
    window_interaction:WindowInteraction,
    known_window_surfaces:WindowSlots<WlSurface>,
    stream:Option<UnixStream>,
    pixel_frame_count:u8,
    pixel_frame_target:u8,
    xdg_shell: XdgShellState,
    layer_shell: WlrLayerShellState,
    // Real wlr-layer wl_shm is opt-in in a disposable QEMU guest only.
    layer_pixels_mode:bool,
    // Five named GTK surfaces are accepted only in the explicit QEMU opt-in.
    gtk_layer_mode:bool,
    layer_surfaces:Vec<(String,WlrLayerSurface)>,
    xdg_toplevel_count: u32,
    layer_surface_count: u32,
    seats: SeatState<Self>,
    seat: Seat<Self>,
    focused: Option<WlSurface>,
    pointer_position: Point<f64,Logical>,
    lifecycle: NativeLifecycle,
    delivered_keys: u32,
    delivered_buttons: u32,
    delivered_motions: u32,
}

struct PendingAsyncFrame {
    client:Option<(WlSurface,wl_buffer::WlBuffer)>,
    layout_generation:u64,
    ticket:FrameTicket,
    ordinal:u8,
}

/// Send wl_surface.frame completions only after the KMS presenter has
/// acknowledged that client-owned pixels actually reached the VM CRTC.
/// Traverse subsurfaces so a client does not stall an animated surface tree.
// A toplevel role request cannot present old pixels at a new geometry.
// The actual imported wl_shm buffer must satisfy the last configure size.
fn accepts_managed_role_buffer(expected:Option<(usize,usize)>,actual:(usize,usize))->bool {
    expected.is_none_or(|size|size==actual)
}

fn complete_presented_frame(surface:&WlSurface,elapsed_ms:u32)->u32 {
    let mut delivered=0u32;
    with_surface_tree_downward(
        surface,(),
        |_,_,&()|TraversalAction::DoChildren(()),
        |_,attributes,&()|{
            for callback in attributes.cached_state.get::<SurfaceAttributes>()
                .current().frame_callbacks.drain(..) {
                callback.done(elapsed_ms);
                delivered=delivered.saturating_add(1);
            }
        },
        |_,_,&()|true,
    );
    delivered
}

impl BridgeState {
    // Stage43: observe the *actual* scene changes in the QEMU Wayland
    // event loop. The stable ordinal-based KMS presenter remains unchanged;
    // this second, generation-based ledger audits its real ACKs before any
    // wl_surface.frame callback or buffer.release is emitted.
    fn note_scene_change(&mut self) {
        if self.async_kms_mode || self.continuous_kms_mode {
            let generation=if self.dynamic_window_mode {
                self.dynamic_scene.generation()
            } else {
                self.multi_scene.layout_generation()
            };
            self.continuous_redraw.scene_changed(generation);
        }
    }

    // Stage46: in this isolated mode ContinuousRedraw is authoritative.
    // All DRM socket writes and ACK reads happen off the Smithay thread.
    fn submit_continuous_frame(&mut self)->Result<(),Box<dyn Error>> {
        if !self.continuous_kms_mode || self.continuous_inflight.is_some() {
            return Ok(());
        }
        let Some(planned)=self.continuous_redraw.next_frame() else {return Ok(());};
        let generation=if self.dynamic_window_mode {
            self.dynamic_scene.generation()
        } else {
            self.multi_scene.layout_generation()
        };
        if planned.generation!=generation {
            self.continuous_redraw.failed(planned);
            return Err("continuous KMS scene changed during planning".into());
        }
        let composed=if self.dynamic_window_mode {
            if let Some(previous)=self.dynamic_confirmed_frame.as_ref() {
                let mut next=previous.clone();
                self.dynamic_scene.composite_damaged_into(&mut next)
                    .map(|_|next).map_err(|_|"dynamic partial composition failed")
            } else {
                self.dynamic_scene.composite().map_err(|_|"dynamic scene failed")
            }
        } else {
            self.multi_scene.composite().map_err(|_|"fixed scene failed")
        };
        let pixels=match composed {
            Ok(pixels)=>pixels,
            Err(_)=>{
                self.continuous_redraw.failed(planned);
                return Err("continuous native scene composition failed".into());
            }
        };
        let Some(worker)=self.continuous_worker.as_ref() else {
            self.continuous_redraw.failed(planned);
            return Err("continuous QEMU KMS transport unavailable".into());
        };
        // The submitted image is retained separately and only becomes a
        // valid partial-paint baseline after a verified real DRM/KMS ACK.
        let candidate=if self.dynamic_window_mode {Some(pixels.clone())}else{None};
        if worker.submit(ContinuousKmsJob{planned,pixels}).is_err() {
            self.continuous_redraw.failed(planned);
            return Err("continuous QEMU KMS worker rejected frame".into());
        }
        self.dynamic_submitted_frame=candidate;
        self.continuous_inflight=Some((
            planned,self.continuous_client_ledger.take_for_presentation()
        ));
        println!("MELANIA_NATIVE_CONTINUOUS_FRAME_SUBMITTED={} generation={}",
                 planned.ticket.sequence,planned.generation);
        let _=std::io::stdout().flush();
        Ok(())
    }

    fn poll_continuous_kms(&mut self)->Result<(),Box<dyn Error>> {
        if !self.continuous_kms_mode {return Ok(());}
        let Some(worker)=self.continuous_worker.as_ref() else {return Ok(());};
        let reply=match worker.try_result() {
            Ok(Some(reply))=>reply,
            Ok(None)=>return Ok(()),
            Err(error)=>return Err(format!("continuous KMS worker disconnected: {error}").into()),
        };
        let (planned,clients)=self.continuous_inflight.take()
            .ok_or("continuous KMS reply with no pending frame")?;
        if reply.planned!=planned {
            self.dynamic_submitted_frame=None;
            self.continuous_redraw.failed(planned);
            return Err("continuous QEMU KMS ACK ticket mismatch".into());
        }
        if let Err(error)=reply.status {
            self.dynamic_submitted_frame=None;
            self.continuous_redraw.failed(planned);
            return Err(format!("continuous KMS presentation rejected: {error}").into());
        }
        // The DRM receiver verifies the actual CRTC FB_ID before MLA1.
        if !self.continuous_redraw.presented(planned) {
            self.dynamic_submitted_frame=None;
            self.dynamic_confirmed_frame=None;
            return Err("continuous KMS stale ACK after seat/output transition".into());
        }
        if self.dynamic_window_mode {
            self.dynamic_confirmed_frame=self.dynamic_submitted_frame.take();
        }
        // Stage59: a damage snapshot is retired only after the real KMS
        // CRTC FB_ID receiver verified this exact frame. A newer client
        // commit or pointer motion keeps its regions dirty for the successor.
        if self.dynamic_window_mode {
            self.dynamic_scene.confirm_presented_damage(planned.generation);
        }
        self.continuous_frame_count=self.continuous_frame_count.saturating_add(1);
        self.presented_layout_generation=planned.generation;
        if self.dynamic_pointer_proof_mode && self.continuous_frame_count==12 {
            // Unique *post-ACK* signal: prior scenarios in this same QEMU
            // guest also present frame 12, so the host must not use a
            // generic ordinal to inject input into the wrong test.
            println!("MELANIA_NATIVE_DYNAMIC_POINTER_READY_AFTER_KMS12=true");
            std::io::stdout().flush()?;
        }
        if self.dynamic_pointer_proof_mode && self.continuous_frame_count==13 {
            let (x,y,width,height)=self.dynamic_scene.geometry(2)
                .ok_or("QMP pointer drag lost resized client B")?;
            if (x-740).abs()>3 || (y-270).abs()>3 ||
               width!=220 || height!=180 || self.dynamic_scene.window_count()!=2 {
                return Err("QMP pointer drag did not preserve resized B geometry".into());
            }
            // Signal only after the real CRTC FB_ID success ACK. Keep the
            // client alive until this proof, otherwise disconnect clears B.
            std::fs::write("/run/melania-native-qemu/dynamic-pointer-frame13-acked",b"real KMS ACK\n")?;
            println!("MELANIA_NATIVE_DYNAMIC_POINTER_KMS13_ACKED=true");
            let _=std::io::stdout().flush();
        }
        if self.dynamic_xdg_resize_mode && self.continuous_frame_count==6 {
            // Before the next client commits, issue REAL xdg_toplevel
            // configure events. Clients must ack xdg_surface.configure
            // before attaching the corresponding smaller wl_shm images.
            if self.dynamic_toplevels.len()!=2 {
                return Err("xdg resize: expected two configured toplevel roles".into());
            }
            for (index,top) in self.dynamic_toplevels.iter().enumerate() {
                let (width,height)=if index==0 {(320,192)} else {(220,180)};
                top.with_pending_state(|state| {
                    state.size=Some((width,height).into());
                    state.states.set(xdg_toplevel::State::Resizing);
                });
                top.send_pending_configure();
            }
            println!("MELANIA_NATIVE_DYNAMIC_XDG_RESIZE_CONFIGURES_SENT=true");
            let _=std::io::stdout().flush();
        }
        // Keep the proof at the verified FB_ID ACK, since the clients may
        // destroy their xdg_toplevels after receiving frame.done.
        let two_visible=if self.dynamic_window_mode {
            self.dynamic_scene.window_count()==2
        } else {
            self.multi_scene.window_count()==2
        };
        if self.multi_window_mode && two_visible &&
           self.known_window_surfaces.live_count()==2 {
            if self.continuous_frame_count==12 {
                self.continuous_two_window_presented=true;
            }
            if self.continuous_reopen_mode && self.continuous_frame_count==14 {
                self.continuous_reopen_frame_presented=true;
            }
        }
        if self.continuous_reopen_mode && self.continuous_frame_count==13 {
            if self.multi_scene.window_count()!=1 ||
               self.known_window_surfaces.get(1).is_some() {
                return Err("continuous close KMS ACK did not remove slot one".into());
            }
            // The surviving B client explicitly requested a frame callback
            // before A was destroyed. Never complete it before KMS FB_ID ACK.
            let survivor=self.known_window_surfaces.get(2).cloned()
                .ok_or("continuous close removed the surviving client")?;
            let delivered=complete_presented_frame(&survivor,13*16);
            if delivered!=1 {
                return Err("continuous close did not complete survivor callback".into());
            }
            self.frame_callback_count=self.frame_callback_count.saturating_add(delivered);
            self.continuous_close_frame_presented=true;
            println!("MELANIA_NATIVE_CONTINUOUS_CLOSE_FRAME13_CALLBACK=true");
        }
        // Each surface is independently eligible for a callback. Newer
        // commits of A must not starve an already presented B.
        for completed in self.continuous_client_ledger.acknowledged(clients) {
            let is_live=(1..=MAX_PENDING_CLIENTS as u64)
                .any(|slot|self.known_window_surfaces.get(slot)==Some(&completed.key));
            if completed.notify_frame && is_live {
                let delivered=complete_presented_frame(
                    &completed.key,(self.continuous_frame_count as u32).saturating_mul(16)
                );
                self.frame_callback_count=self.frame_callback_count.saturating_add(delivered);
            }
            completed.resource.release();
        }
        println!("MELANIA_NATIVE_CONTINUOUS_KMS_ACK={} generation={}",
                 self.continuous_frame_count,planned.generation);
        let _=std::io::stdout().flush();
        // If more damage arrived while KMS was in flight, render the latest.
        self.submit_continuous_frame()?;
        Ok(())
    }

    fn begin_generation_proof(&mut self,expected:u64)->Result<(),Box<dyn Error>> {
        if !self.async_kms_mode { return Ok(()); }
        if self.continuous_pending.is_some() {
            return Err("continuous redraw proof already has an outstanding frame".into());
        }
        let candidate=self.continuous_redraw.next_frame()
            .ok_or("continuous redraw proof has no eligible dirty scene")?;
        if candidate.generation!=expected {
            self.continuous_redraw.failed(candidate);
            return Err("continuous redraw proof disagrees with composed scene generation".into());
        }
        self.continuous_pending=Some(candidate);
        Ok(())
    }

    fn reject_generation_proof(&mut self) {
        if let Some(candidate)=self.continuous_pending.take() {
            self.continuous_redraw.failed(candidate);
        }
    }

    fn confirm_generation_proof(&mut self,actual:u64)->Result<(),Box<dyn Error>> {
        if !self.async_kms_mode { return Ok(()); }
        let candidate=self.continuous_pending.take()
            .ok_or("KMS ACK without a planned continuous scene generation")?;
        if candidate.generation!=actual ||
           !self.continuous_redraw.presented(candidate) {
            return Err("KMS ACK does not match the outstanding scene generation".into());
        }
        println!("MELANIA_NATIVE_SCENE_GENERATION_ACK={}",candidate.generation);
        Ok(())
    }

    /// Resolve QEMU physical output coordinates to a retained client surface.
    fn pointer_target(&self,point:Point<f64,Logical>)
        ->Option<(WlSurface,Point<f64,Logical>)>
    {
        if !self.multi_window_mode {
            return self.focused.clone().map(|s|(s,(0.0,0.0).into()));
        }
        let key=if self.dynamic_window_mode {
            self.dynamic_scene.hit_test(point.x,point.y)?
        } else {
            self.multi_scene.hit_test(point.x,point.y)?
        };
        let surface=self.known_window_surfaces.get(key)?.clone();
        let (x,y)=if self.dynamic_window_mode {
            self.dynamic_scene.position(key)?
        } else {
            self.multi_scene.position(key)?
        };
        Some((surface,(f64::from(x),f64::from(y)).into()))
    }

    /// Changes to model geometry are bounded and require an active real
    /// pointer grab. A compositor-only change is not proof of repaint; the
    /// current KMS stream still presents only on subsequent client commits.
    fn apply_window_motion(&mut self,point:Point<f64,Logical>) {
        if !self.multi_window_mode {return;}
        let Some((key,rect))=self.window_interaction.motion(point.x,point.y) else {
            return;
        };
        match self.window_interaction.active_kind() {
            Some(InteractionKind::Move)=>{
                let moved=if self.dynamic_window_mode {
                    // Avoid issuing redundant GPU damage for a pointer event
                    // that didn't change the client's screen position.
                    self.dynamic_scene.position(key)!=Some((rect.x,rect.y)) &&
                        self.dynamic_scene.move_to(key,rect.x,rect.y).is_ok()
                } else {
                    self.multi_scene.move_to(key,rect.x,rect.y).is_ok()
                };
                if moved {
                    if self.managed_xdg_mode && self.dynamic_window_mode {
                        // Save the real pointer-drag placement for restore.
                        if let Some(surface)=self.known_window_surfaces.get(key) {
                            if let Some((_,model))=self.managed_windows.iter_mut()
                                .find(|(role,_)|role==surface) {
                                model.update_normal_geometry(rect);
                            }
                        }
                    }
                    self.note_scene_change();
                    println!("MELANIA_NATIVE_WINDOW_DRAG_MODEL={} x={} y={}",
                             key,rect.x,rect.y);
                    if self.dynamic_window_mode {
                        // The dynamic scene is presented through the existing
                        // asynchronous real-ACK KMS path, never on mouse
                        // motion without a verified framebuffer result.
                        if self.continuous_frame_count<self.continuous_frame_target {
                            if let Err(error)=self.submit_continuous_frame() {
                                eprintln!("dynamic QEMU drag KMS rejected: {error}");
                            }
                        }
                    } else if let Err(error)=self.submit_drag_frame() {
                        eprintln!("QEMU drag KMS frame rejected: {error}");
                    }
                }
            },
            Some(InteractionKind::ResizeBottomRight)=>{
                // A real xdg.configure + client ACK must precede actual
                // resize/scanout. For now this is a non-destructive preview.
                println!("MELANIA_NATIVE_WINDOW_RESIZE_PREVIEW={} w={} h={}",
                         key,rect.width,rect.height);
            },
            None=>{},
        }
    }

    /// A real libinput pointer movement can repaint the compositor's
    /// retained scene without requiring an extra wl_surface.commit.
    /// Stage38 is intentionally limited to ONE additional frame (ordinal 4).
    fn submit_drag_frame(&mut self) -> Result<(),Box<dyn Error>> {
        if !self.drag_redraw_mode || self.pixel_frame_count!=3 ||
           self.async_pending.is_some() ||
           self.multi_scene.layout_generation()==self.presented_layout_generation {
            return Ok(());
        }
        let frame=self.multi_scene.composite()
            .map_err(|_|"QEMU drag scene compose failed")?;
        if !self.render_scheduler.damage() {
            return Err("QEMU drag damage rejected by suspended session".into());
        }
        let ticket=self.render_scheduler.begin_frame()
            .ok_or("QEMU drag presentation already in progress")?;
        if let Err(error)=self.begin_generation_proof(self.multi_scene.layout_generation()) {
            self.render_scheduler.fail_present(ticket);
            return Err(error);
        }
        let worker=self.async_stream.as_ref()
            .ok_or("QEMU drag KMS worker unavailable")?;
        if worker.submit(FrameJob{ticket,ordinal:4,pixels:frame}).is_err() {
            self.reject_generation_proof();
            self.render_scheduler.fail_present(ticket);
            return Err("QEMU drag KMS queue rejected fourth frame".into());
        }
        self.async_pending=Some(PendingAsyncFrame{
            client:None,ticket,ordinal:4,
            layout_generation:self.multi_scene.layout_generation(),
        });
        println!("MELANIA_NATIVE_DRAG_FRAME_SUBMITTED=4");
        std::io::stdout().flush()?;
        Ok(())
    }

    /// QEMU-only fifth presentation: remove the destroyed client's old
    /// pixels from the *actual* KMS framebuffer while its peer stays alive.
    /// The fourth-frame ticket must have been acknowledged first.
    fn submit_close_frame(&mut self) -> Result<(),Box<dyn Error>> {
        if !self.close_redraw_mode || self.pixel_frame_count!=4 ||
           self.async_pending.is_some() ||
           self.multi_scene.window_count()!=1 ||
           !self.render_scheduler.needs_redraw() {
            return Err("QEMU close frame not ready: missing live survivor or damage".into());
        }
        let frame=self.multi_scene.composite()
            .map_err(|_|"QEMU close scene compose failed")?;
        let ticket=self.render_scheduler.begin_frame()
            .ok_or("QEMU close frame could not acquire next KMS ticket")?;
        if let Err(error)=self.begin_generation_proof(self.multi_scene.layout_generation()) {
            self.render_scheduler.fail_present(ticket);
            return Err(error);
        }
        let worker=self.async_stream.as_ref()
            .ok_or("QEMU close frame KMS worker unavailable")?;
        if worker.submit(FrameJob{ticket,ordinal:5,pixels:frame}).is_err() {
            self.reject_generation_proof();
            self.render_scheduler.fail_present(ticket);
            return Err("QEMU close frame was not accepted by KMS worker".into());
        }
        self.async_pending=Some(PendingAsyncFrame{
            client:None,ticket,ordinal:5,
            layout_generation:self.multi_scene.layout_generation(),
        });
        println!("MELANIA_NATIVE_CLOSE_FRAME_SUBMITTED=5");
        std::io::stdout().flush()?;
        Ok(())
    }

    /// Submit exactly one sixth-frame update from the NEW real wl_shm buffer.
    /// The old first wl_surface must already be destroyed and slot 1 freed.
    /// This is deliberately opt-in for the disposable QEMU guest only.
    fn submit_reopen_frame(&mut self,surface:&WlSurface,buffer:wl_buffer::WlBuffer)
        ->Result<(),Box<dyn Error>>
    {
        if !self.reopen_redraw_mode || self.pixel_frame_count!=5 ||
           !self.close_frame_presented || self.async_pending.is_some() ||
           self.known_window_surfaces.get(1).is_some() ||
           self.multi_scene.window_count()!=1 {
            return Err("QEMU sixth frame requires one closed and one live window".into());
        }
        let imported=with_buffer_contents(&buffer,|ptr,len,metadata|{
            native_client_frame::copy_client_shm(ptr,len,metadata)
        });
        let pixels=match imported {
            Ok(Ok(pixels))=>pixels,
            _=>return Err("QEMU reopened Wayland client did not provide valid wl_shm pixels".into()),
        };
        let key=self.known_window_surfaces.register(surface.clone())
            .map_err(|_|"QEMU reopening exceeds stable slot limit")?;
        if key!=1 { return Err("QEMU reopened surface failed to reuse the freed slot".into()); }
        self.multi_scene.upsert(key,pixels)
            .map_err(|_|"QEMU reopened window invalid scene image")?;
        self.note_scene_change();
        let frame=self.multi_scene.composite()
            .map_err(|_|"QEMU reopened window scene cannot be composed")?;
        if !self.render_scheduler.damage() {
            return Err("QEMU reopen damage rejected by seat scheduler".into());
        }
        let ticket=self.render_scheduler.begin_frame()
            .ok_or("QEMU reopen frame could not acquire KMS ticket")?;
        if let Err(error)=self.begin_generation_proof(self.multi_scene.layout_generation()) {
            self.render_scheduler.fail_present(ticket);
            return Err(error);
        }
        let worker=self.async_stream.as_ref()
            .ok_or("QEMU reopen frame KMS worker unavailable")?;
        if worker.submit(FrameJob{ticket,ordinal:6,pixels:frame}).is_err() {
            self.reject_generation_proof();
            self.render_scheduler.fail_present(ticket);
            return Err("QEMU reopening sixth frame rejected by KMS worker".into());
        }
        self.async_pending=Some(PendingAsyncFrame{
            client:Some((surface.clone(),buffer)),
            ticket,ordinal:6,
            layout_generation:self.multi_scene.layout_generation(),
        });
        println!("MELANIA_NATIVE_REOPEN_WINDOW_SLOT={key}");
        println!("MELANIA_NATIVE_CLIENT_REOPENED_XDG_COMMITTED=true");
        println!("MELANIA_NATIVE_REOPEN_FRAME_SUBMITTED=6");
        std::io::stdout().flush()?;
        Ok(())
    }

    /// Poll the worker without blocking the Smithay/calloop dispatcher.
    /// No callback, buffer.release(), or frame counter is issued before the
    /// worker has read the genuine KMS ACK from the VM Rust DRM process.
    fn poll_async_kms(&mut self) -> Result<(),Box<dyn Error>> {
        if !self.async_kms_mode {return Ok(());}
        let worker=self.async_stream.as_ref().ok_or("async KMS worker not connected")?;
        let reply=match worker.try_result() {
            Ok(Some(result))=>result,
            Ok(None)=>return Ok(()),
            Err(error)=>return Err(format!("async KMS worker disconnected: {error}").into()),
        };
        let pending=self.async_pending.take()
            .ok_or("KMS worker returned an ACK without a pending Wayland frame")?;
        if pending.ticket!=reply.ticket || pending.ordinal!=reply.ordinal {
            return Err("async KMS ticket or frame ordinal mismatch".into());
        }
        if let Err(error)=reply.status {
            self.reject_generation_proof();
            self.render_scheduler.fail_present(pending.ticket);
            return Err(format!("async Rust KMS presentation failed: {error}").into());
        }
        if !self.render_scheduler.complete_present(pending.ticket) {
            self.reject_generation_proof();
            return Err("async KMS ACK invalidated by seat pause or GPU change".into());
        }
        // The native scene generation is checked only after the verified
        // CRTC FB_ID ACK and before clients are notified of presentation.
        self.confirm_generation_proof(pending.layout_generation)?;
        self.pixel_frame_count=pending.ordinal;
        self.pixel_ready=self.pixel_frame_count==self.pixel_frame_target;
        self.presented_layout_generation=pending.layout_generation;
        if self.multi_window_mode && pending.ordinal==3 &&
           self.multi_scene.window_count()==2 {
            self.two_window_frame_presented=true;
        }
        if self.drag_redraw_mode && pending.ordinal==4 {
            self.drag_frame_presented=true;
        }
        if self.close_redraw_mode && pending.ordinal==5 {
            self.close_frame_presented=true;
        }
        if self.reopen_redraw_mode && pending.ordinal==6 &&
           self.multi_scene.window_count()==2 &&
           self.known_window_surfaces.get(1).is_some() &&
           self.known_window_surfaces.get(2).is_some() {
            // Capture the state at the actual KMS ACK. The test client is
            // allowed to destroy its window after receiving frame.done.
            self.reopen_frame_presented=true;
        }
        println!("MELANIA_NATIVE_LIVE_KMS_FRAME_ACK={}",pending.ordinal);
        println!("MELANIA_NATIVE_SCHEDULED_FRAME_ACK={}",pending.ordinal);
        println!("MELANIA_NATIVE_ASYNC_FRAME_COMPLETED={}",pending.ordinal);
        if let Some((surface,buffer))=pending.client {
            println!("MELANIA_NATIVE_CLIENT_SHM_IMPORTED=true");
            println!("MELANIA_NATIVE_CLIENT_FRAME_INDEX={}",pending.ordinal);
            println!("MELANIA_NATIVE_CLIENT_FRAME_COMPOSED=true");
            if self.frame_callbacks_probe {
                let delivered=complete_presented_frame(
                    &surface,pending.ordinal as u32*16
                );
                if self.reopen_redraw_mode && pending.ordinal==6 && delivered!=1 {
                    return Err("reopened client did not request a real sixth-frame Wayland callback".into());
                }
                self.frame_callback_count=self.frame_callback_count.saturating_add(delivered);
                if self.reopen_redraw_mode && pending.ordinal==6 {
                    println!("MELANIA_NATIVE_REOPEN_FRAME_CALLBACK_DONE=true");
                }
                println!("MELANIA_NATIVE_FRAME_CALLBACK_DONE={}",pending.ordinal);
                println!("MELANIA_NATIVE_FRAME_CALLBACK_COUNT={}",self.frame_callback_count);
            }
            buffer.release();
            if self.reopen_redraw_mode && pending.ordinal==6 {
                println!("MELANIA_NATIVE_REOPEN_FRAME_ACKED=6");
            }
        } else {
            match pending.ordinal {
                4 if self.drag_redraw_mode=>
                    println!("MELANIA_NATIVE_DRAG_FRAME_ACKED=4"),
                5 if self.close_redraw_mode=>{
                    // A surviving real wl_surface requested this callback
                    // before A was destroyed. Only the fifth DRM FB_ID ACK
                    // may complete it, not display.sync or an idle timer.
                    let survivor=self.known_window_surfaces.get(2).cloned()
                        .ok_or("QEMU surviving Wayland window unavailable after KMS ACK")?;
                    let delivered=complete_presented_frame(&survivor,5*16);
                    if delivered!=1 {
                        return Err("QEMU close KMS presentation did not complete survivor callback".into());
                    }
                    self.frame_callback_count=self.frame_callback_count.saturating_add(delivered);
                    println!("MELANIA_NATIVE_FRAME_CALLBACK_COUNT={}",self.frame_callback_count);
                    println!("MELANIA_NATIVE_CLOSE_FRAME_CALLBACK_DONE=true");
                    println!("MELANIA_NATIVE_CLOSE_FRAME_ACKED=5");
                },
                _=>return Err("unexpected compositor-only KMS frame".into()),
            }
        }
        if self.close_redraw_mode && pending.ordinal==4 {
            self.submit_close_frame()?;
        }
        std::io::stdout().flush()?;
        Ok(())
    }
}

impl BridgeState {
    /// Apply actual xdg_toplevel requests to the QEMU compositor state and
    /// configure the client. A new client commit is still required for the
    /// maximized-sized wl_shm content to become visible at its full size.
    fn request_managed_maximize(&mut self,top:ToplevelSurface,enabled:bool) {
        if !self.managed_xdg_mode {
            top.send_configure();
            return;
        }
        let surface=top.wl_surface().clone();
        let Some((_,window))=self.managed_windows.iter_mut()
            .find(|(role,_)|role==&surface) else {
            top.send_configure();
            return;
        };
        let changed=if enabled {window.maximize()} else {
            // Do not reset unrelated minimized/fullscreen states on an
            // unmaximize request. They need their own role transition.
            if window.state()==WindowDisplayState::Maximized {
                window.restore()
            } else {false}
        };
        let rectangle=window.geometry();
        top.with_pending_state(|pending| {
            if enabled {
                pending.states.set(xdg_toplevel::State::Maximized);
                pending.states.unset(xdg_toplevel::State::Fullscreen);
            } else {pending.states.unset(xdg_toplevel::State::Maximized);}
            if let Some(rect)=rectangle {
                pending.size=Some((rect.width,rect.height).into());
            }
        });
        top.send_pending_configure();
        if changed {
            if let Some(rect)=rectangle {
                // The configure is only a request. A previous wl_shm image
                // must NOT be moved or submitted to KMS before the matching
                // client buffer has been committed and validated.
                self.managed_pending_sizes.retain(|(role,_)|role!=&surface);
                self.managed_pending_sizes.push((
                    surface.clone(),(rect.width as usize,rect.height as usize)
                ));
            }
        }
        println!("MELANIA_NATIVE_MANAGED_XDG_MAXIMIZED={enabled}");
    }

    /// The disposable QEMU output is a single 1280x800 display. The optional
    /// xdg fullscreen output hint is therefore ignored, not mistaken for a
    /// real multi-output KMS policy. The client still owns its pixels.
    fn request_managed_fullscreen(&mut self,top:ToplevelSurface,enabled:bool) {
        if !self.managed_xdg_mode {
            top.send_configure();
            return;
        }
        let surface=top.wl_surface().clone();
        let Some((_,window))=self.managed_windows.iter_mut()
            .find(|(role,_)|role==&surface) else {
            top.send_configure();
            return;
        };
        let changed=if enabled {window.fullscreen()} else {
            if window.state()==WindowDisplayState::Fullscreen {
                window.restore()
            } else {false}
        };
        let rectangle=window.geometry();
        top.with_pending_state(|pending| {
            if enabled {
                pending.states.set(xdg_toplevel::State::Fullscreen);
                pending.states.unset(xdg_toplevel::State::Maximized);
            } else {
                pending.states.unset(xdg_toplevel::State::Fullscreen);
            }
            if let Some(rect)=rectangle {
                pending.size=Some((rect.width,rect.height).into());
            }
        });
        top.send_pending_configure();
        if changed {
            if let Some(rect)=rectangle {
                // The configure is only a request. A previous wl_shm image
                // must NOT be moved or submitted to KMS before the matching
                // client buffer has been committed and validated.
                self.managed_pending_sizes.retain(|(role,_)|role!=&surface);
                self.managed_pending_sizes.push((
                    surface.clone(),(rect.width as usize,rect.height as usize)
                ));
            }
        }
        println!("MELANIA_NATIVE_MANAGED_XDG_FULLSCREEN={enabled}");
    }
}

impl BufferHandler for BridgeState {
    fn buffer_destroyed(&mut self,_:&wl_buffer::WlBuffer) {}
}
impl CompositorHandler for BridgeState {
    fn compositor_state(&mut self)->&mut CompositorState {&mut self.compositor}
    fn client_compositor_state<'a>(&self,client:&'a Client)->&'a CompositorClientState {
        &client.get_data::<BridgeClientState>().expect("registered client").compositor
    }
    fn commit(&mut self,surface:&WlSurface) {
        if self.layer_pixels_mode {
            if let Some(role)=self.layer_surfaces.iter().find(|(_,r)|r.wl_surface()==surface)
                .map(|(_,r)|r.clone()) {
                // A protocol configure is NOT pixel evidence. Only a client
                // wl_shm buffer copied out of Smithay's live mapping enters
                // the native retained scene and the Rust KMS worker.
                let assignment=with_states(surface,|states|{
                    states.cached_state.get::<SurfaceAttributes>().current().buffer.take()
                });
                if let Some(BufferAssignment::NewBuffer(buffer))=assignment {
                    let imported=with_buffer_contents(&buffer,|ptr,len,metadata|{
                        melania_native::owned_shm::copy_bounded_client_shm(ptr,len,metadata)
                    });
                    let pixels=match imported {
                        Ok(Ok(image))=>match WindowPixels::from_owned_shm(image) {
                            Ok(image)=>image,
                            Err(_)=>{
                                eprintln!("MELANIA_NATIVE_LAYER_SHM_INVALID=true");
                                buffer.release();
                                return;
                            }
                        },
                        _=>{
                            eprintln!("MELANIA_NATIVE_LAYER_SHM_INVALID=true");
                            buffer.release();
                            return;
                        }
                    };
                    let attrs=with_states(surface,|states|{
                        *states.cached_state.get::<LayerSurfaceCachedState>().current()
                    });
                    let shell_layer=match attrs.layer {
                        WlrLayer::Background=>ShellLayer::Background,
                        WlrLayer::Bottom=>ShellLayer::Bottom,
                        WlrLayer::Top=>ShellLayer::Top,
                        WlrLayer::Overlay=>ShellLayer::Overlay,
                    };
                    let requested=LayerRequest {
                        layer:shell_layer,
                        anchors:Anchors{
                            top:attrs.anchor.contains(WlrAnchor::TOP),
                            bottom:attrs.anchor.contains(WlrAnchor::BOTTOM),
                            left:attrs.anchor.contains(WlrAnchor::LEFT),
                            right:attrs.anchor.contains(WlrAnchor::RIGHT),
                        },
                        width:attrs.size.w.max(0) as u32,
                        height:attrs.size.h.max(0) as u32,
                        margins:Insets{top:attrs.margin.top,right:attrs.margin.right,
                                       bottom:attrs.margin.bottom,left:attrs.margin.left},
                        exclusive_zone:i32::from(attrs.exclusive_zone),
                    };
                    let placement=match layer_surface_layout::place(requested,1280,800) {
                        Ok(v)=>v,
                        Err(error)=>{
                            eprintln!("MELANIA_NATIVE_LAYER_LAYOUT_REJECTED={error:?}");
                            buffer.release();
                            return;
                        }
                    };
                    if pixels.width()!=placement.rect.width as usize ||
                       pixels.height()!=placement.rect.height as usize {
                        eprintln!("MELANIA_NATIVE_LAYER_CONFIGURE_BUFFER_MISMATCH=true");
                        buffer.release();
                        return;
                    }
                    // Restrict allocation to the dynamic QEMU scene, and
                    // retain the layer's z-index for render and pointer hits.
                    let key=match self.known_window_surfaces.register(surface.clone()) {
                        Ok(key)=>key,
                        Err(_)=>{
                            buffer.release();
                            return;
                        }
                    };
                    let role_layer:SceneLayer=placement.layer.scene_layer();
                    if self.dynamic_scene.upsert_in_layer(
                        key,placement.rect.x,placement.rect.y,role_layer,pixels
                    ).is_err() {
                        eprintln!("MELANIA_NATIVE_LAYER_SCENE_REJECTED=true");
                        buffer.release();
                        return;
                    }
                    match self.continuous_client_ledger.stage(surface.clone(),buffer) {
                        Ok(Some(previous))=>previous.release(),
                        Ok(None)=>{},
                        Err((_,rejected))=>{
                            rejected.release();
                            eprintln!("MELANIA_NATIVE_LAYER_KMS_QUEUE_FULL=true");
                            return;
                        }
                    }
                    self.note_scene_change();
                    if let Err(error)=self.submit_continuous_frame() {
                        eprintln!("MELANIA_NATIVE_LAYER_KMS_SUBMIT_FAILED={error}");
                    } else {
                        println!("MELANIA_NATIVE_LAYER_OWNED_SHM_QUEUED={}x{} layer={role_layer:?}",
                                 placement.rect.width,placement.rect.height);
                    }
                }
                // Never autofocus noninteractive taskbars on wl_surface.commit.
                return;
            }
        }
        // A role-only GTK handshake must never focus hidden/noninteractive
        // layers; it does not attach a wl_shm image or simulate a GTK frame.
        if self.gtk_layer_mode && !self.layer_pixels_mode &&
           self.layer_surfaces.iter().any(|(_,r)|r.wl_surface()==surface) {
            return;
        }
        if self.continuous_kms_mode {
            let assignment=with_states(surface,|states|{
                states.cached_state.get::<SurfaceAttributes>().current().buffer.take()
            });
            if let Some(BufferAssignment::NewBuffer(buffer))=assignment {
                // The importer owns the snapshot before the Smithay pool
                // access callback returns. Fixed QEMU uses its original
                // 360x224 checks; the separate dynamic VM path uses Stage51.
                if self.dynamic_window_mode {
                    let imported=with_buffer_contents(&buffer,|ptr,len,metadata|{
                        melania_native::owned_shm::copy_bounded_client_shm(ptr,len,metadata)
                    });
                    let pixels=match imported {
                        Ok(Ok(image))=>match WindowPixels::from_owned_shm(image) {
                            Ok(pixels)=>pixels,
                            Err(_)=>{
                                eprintln!("dynamic QEMU client has invalid image");
                                buffer.release();
                                return;
                            }
                        },
                        _=>{
                            eprintln!("dynamic QEMU client submitted invalid wl_shm");
                            buffer.release();
                            return;
                        }
                    };
                    let key=match self.known_window_surfaces.register(surface.clone()) {
                        Ok(key)=>key,
                        Err(_)=>{
                            eprintln!("dynamic QEMU client has no scene slot");
                            buffer.release();
                            return;
                        }
                    };
                    let Some(default_pos)=melania_native::window_placement::default_position(key,1280,800) else {
                        eprintln!("dynamic QEMU client outside bounded placement policy");
                        buffer.release();
                        return;
                    };
                    // Refuse stale-size buffers during a managed role switch.
                    // A successful client ACK alone did not provide pixels.
                    if self.managed_xdg_mode {
                        let expected=self.managed_pending_sizes.iter()
                            .find(|(role,_)|role==surface).map(|(_,size)|*size);
                        if !accepts_managed_role_buffer(expected,(pixels.width(),pixels.height())) {
                            eprintln!("MELANIA_NATIVE_MANAGED_XDG_STALE_BUFFER_REJECTED=true");
                            buffer.release();
                            return;
                        }
                    }
                    // Keep the current position across later wl_shm commits.
                    let (x,y)=if self.managed_xdg_mode {
                        self.managed_windows.iter().find(|(role,_)|role==surface)
                            .and_then(|(_,window)|window.geometry())
                            .map(|rect|(rect.x,rect.y))
                            .unwrap_or(default_pos)
                    } else {
                        self.dynamic_scene.position(key).unwrap_or(default_pos)
                    };
                    let width=pixels.width() as i32;
                    let height=pixels.height() as i32;
                    if self.dynamic_scene.upsert(key,x,y,pixels).is_err() {
                        eprintln!("dynamic QEMU client image rejected");
                        buffer.release();
                        return;
                    }
                    if self.managed_xdg_mode {
                        // Change the remembered normal geometry only after
                        // the owned client pixels actually entered the scene.
                        if let Some((_,window))=self.managed_windows.iter_mut()
                            .find(|(role,_)|role==surface) {
                            window.update_normal_geometry(WindowRect{x,y,width,height});
                        }
                        self.managed_pending_sizes.retain(|(role,_)|role!=surface);
                        println!("MELANIA_NATIVE_MANAGED_XDG_CLIENT_BUFFER_ACCEPTED={}x{}",width,height);
                    }
                } else {
                    let imported=with_buffer_contents(&buffer,|ptr,len,metadata|{
                        native_client_frame::copy_client_shm(ptr,len,metadata)
                    });
                    let pixels=match imported {
                        Ok(Ok(pixels))=>pixels,
                        _=>{
                            eprintln!("continuous QEMU client submitted invalid wl_shm");
                            return;
                        }
                    };
                    let key=match self.known_window_surfaces.register(surface.clone()) {
                        Ok(key)=>key,
                        Err(_)=>{
                            eprintln!("continuous QEMU client has no free scene slot");
                            return;
                        }
                    };
                    if self.multi_scene.upsert(key,pixels).is_err() {
                        eprintln!("continuous QEMU scene pixels rejected");
                        return;
                    }
                }
                // Retain at most one not-yet-submitted client buffer. Its
                // pixels were copied into the bounded retained scene.
                match self.continuous_client_ledger.stage(surface.clone(),buffer) {
                    Ok(Some(superseded))=>superseded.release(),
                    Ok(None)=>{},
                    Err((_,rejected))=>{
                        rejected.release();
                        eprintln!("continuous Wayland client receipt capacity exceeded");
                        return;
                    }
                }
                self.note_scene_change();
                if let Err(error)=self.submit_continuous_frame() {
                    eprintln!("continuous QEMU KMS scheduling failed: {error}");
                }
            }
            return;
        }
        if self.reopen_redraw_mode && self.pixel_frame_count==5 {
            let assignment=with_states(surface,|states|{
                states.cached_state.get::<SurfaceAttributes>().current().buffer.take()
            });
            if let Some(BufferAssignment::NewBuffer(buffer))=assignment {
                if let Err(error)=self.submit_reopen_frame(surface,buffer) {
                    eprintln!("QEMU real Wayland reopen buffer rejected: {error}");
                }
                return;
            }
        }
        if self.pixel_probe && self.pixel_frame_count<self.pixel_frame_target &&
           self.pixel_frame_count<3 && self.xdg_toplevel_count>0 {
            let assignment=with_states(surface,|states|{
                states.cached_state.get::<SurfaceAttributes>().current().buffer.take()
            });
            if let Some(BufferAssignment::NewBuffer(buffer))=assignment {
                // In the two-window VM test each COMMIT carries pixel data
                // from a distinct wl_surface. Keep the previous window's
                // last owned snapshot for the next full-scene composition.
                let imported=with_buffer_contents(&buffer,|ptr,len,metadata|{
                    if self.multi_window_mode {
                        native_client_frame::copy_client_shm(ptr,len,metadata)
                    } else {
                        native_client_frame::compose_from_shm(ptr,len,metadata)
                    }
                });
                match imported {
                    Ok(Ok(imported))=>{
                        let frame=if self.multi_window_mode {
                            let key=match self.known_window_surfaces.register(surface.clone()) {
                                Ok(key)=>key,
                                Err(_)=>{
                                    eprintln!("QEMU two-window scene exceeded bound");
                                    return;
                                },
                            };
                            if self.multi_scene.upsert(key,imported).is_err() {
                                eprintln!("QEMU retained window rejected invalid wl_shm image");
                                return;
                            }
                            self.note_scene_change();
                            match self.multi_scene.composite() {
                                Ok(frame)=>frame,
                                Err(error)=>{
                                    eprintln!("QEMU multi-window composition failed: {error:?}");
                                    return;
                                }
                            }
                        } else {imported};
                        let next=self.pixel_frame_count+1;
                        // This still uses a bounded, synchronous QEMU stream.
                        // Stage32 adds verified frame identity and coalesced
                        // damage semantics, NOT an asynchronous renderer yet.
                        let scheduled:Option<FrameTicket>=if self.live_stream {
                            if !self.render_scheduler.damage() {
                                eprintln!("native render scheduler rejected damage during seat pause");
                                return;
                            }
                            match self.render_scheduler.begin_frame() {
                                Some(ticket)=>Some(ticket),
                                None=>{
                                    eprintln!("native frame already outstanding or seat inactive");
                                    return;
                                }
                            }
                        } else {None};
                        if self.async_kms_mode {
                            let ticket=scheduled.ok_or("scheduled KMS ticket missing");
                            let ticket=match ticket {
                                Ok(ticket)=>ticket,
                                Err(error)=>{eprintln!("{error}");return;}
                            };
                            if self.async_pending.is_some() {
                                eprintln!("second frame while asynchronous KMS is presenting");
                                self.render_scheduler.fail_present(ticket);
                                return;
                            }
                            if let Err(error)=self.begin_generation_proof(self.multi_scene.layout_generation()) {
                                eprintln!("QEMU generation audit refused native frame: {error}");
                                self.render_scheduler.fail_present(ticket);
                                return;
                            }
                            let worker=match self.async_stream.as_ref() {
                                Some(worker)=>worker,
                                None=>{
                                    eprintln!("async KMS transport not connected");
                                    self.reject_generation_proof();
                                    self.render_scheduler.fail_present(ticket);
                                    return;
                                }
                            };
                            if worker.submit(FrameJob{ticket,ordinal:next,pixels:frame}).is_err() {
                                eprintln!("async KMS transport full or disconnected");
                                self.reject_generation_proof();
                                self.render_scheduler.fail_present(ticket);
                                return;
                            }
                            self.async_pending=Some(PendingAsyncFrame {
                                client:Some((surface.clone(),buffer)),ticket,ordinal:next,
                                layout_generation:self.multi_scene.layout_generation(),
                            });
                            println!("MELANIA_NATIVE_ASYNC_FRAME_SUBMITTED={next}");
                            let _=std::io::stdout().flush();
                            // Return to Smithay NOW. No KMS socket I/O or
                            // waiting for a page-flip acknowledgement here.
                            return;
                        }
                        let handoff:Result<(),Box<dyn Error>>=if self.live_stream {
                            (||->Result<(),Box<dyn Error>>{
                                let stream=self.stream.as_mut()
                                    .ok_or("native DRM stream not connected")?;
                                let length=u32::try_from(frame.len())?;
                                stream.write_all(&length.to_le_bytes())?;
                                stream.write_all(&frame)?;
                                stream.flush()?;
                                let mut ack=[0u8;1];
                                stream.read_exact(&mut ack)?;
                                if ack[0]!=next {
                                    return Err("Rust KMS did not acknowledge the displayed frame".into());
                                }
                                println!("MELANIA_NATIVE_LIVE_KMS_FRAME_ACK={next}");
                                Ok(())
                            })()
                        } else {
                            native_client_frame::write_frame_index(&frame,self.pixel_frame_count)
                        };
                        match handoff {
                            Ok(())=>{
                                if let Some(ticket)=scheduled {
                                    // Do not send a wl_surface.frame completion
                                    // for an invalidated or duplicated ticket.
                                    if !self.render_scheduler.complete_present(ticket) {
                                        eprintln!("native KMS ACK rejected: stale frame ticket");
                                        return;
                                    }
                                    println!("MELANIA_NATIVE_SCHEDULED_FRAME_ACK={next}");
                                }
                                self.pixel_frame_count=next;
                                self.pixel_ready=self.pixel_frame_count==self.pixel_frame_target;
                                println!("MELANIA_NATIVE_CLIENT_SHM_IMPORTED=true");
                                println!("MELANIA_NATIVE_CLIENT_FRAME_INDEX={}",self.pixel_frame_count);
                                println!("MELANIA_NATIVE_CLIENT_FRAME_COMPOSED=true");
                                if self.live_stream && self.frame_callbacks_probe {
                                    // ACK follows successful FB_ID check in drm-rs.
                                    // A frame callback before that point would
                                    // falsely tell the client rendering completed.
                                    let count=complete_presented_frame(
                                        surface,self.pixel_frame_count as u32*16
                                    );
                                    self.frame_callback_count+=count;
                                    println!("MELANIA_NATIVE_FRAME_CALLBACK_DONE={}",self.pixel_frame_count);
                                    println!("MELANIA_NATIVE_FRAME_CALLBACK_COUNT={}",self.frame_callback_count);
                                }
                                let _=std::io::stdout().flush();
                            },
                            Err(error)=>{
                                if let Some(ticket)=scheduled {
                                    self.render_scheduler.fail_present(ticket);
                                }
                                eprintln!("Melania Wayland frame transfer failed: {error}");
                                // Do not acknowledge/release an unpresented wl_buffer.
                                return;
                            },
                        }
                    },
                    Ok(Err(error))=>eprintln!("Melania invalid client pixels: {error}"),
                    Err(error)=>eprintln!("Melania wl_shm access error: {error:?}"),
                }
                buffer.release();
            }
        }
        if self.focused.is_some() {return;}
        self.focused=Some(surface.clone());
        let key=self.seat.get_keyboard().expect("keyboard");
        key.set_focus(self,Some(surface.clone()),SERIAL_COUNTER.next_serial());
        let pointer=self.seat.get_pointer().expect("pointer");
        pointer.motion(self,Some((surface.clone(),(0.0,0.0).into())),
            &MotionEvent{location:(0.0,0.0).into(),
                         serial:SERIAL_COUNTER.next_serial(),time:0});
        pointer.frame(self);
        println!("MELANIA_NATIVE_WAYLAND_SURFACE_FOCUSED=true");
        let _=std::io::stdout().flush();
    }
}
impl OutputHandler for BridgeState {}
impl ShmHandler for BridgeState {
    fn shm_state(&self)->&ShmState {&self.shm}
}
impl SeatHandler for BridgeState {
    type KeyboardFocus=WlSurface;
    type PointerFocus=WlSurface;
    type TouchFocus=WlSurface;
    fn seat_state(&mut self)->&mut SeatState<Self> {&mut self.seats}
    fn focus_changed(&mut self,_:&Seat<Self>,_:Option<&WlSurface>){}
    fn cursor_image(&mut self,_:&Seat<Self>,_:CursorImageStatus){}
}
// Match the protocol implementations of the nested compositor, but keep
// this guest-specific backend headless: configuring a role is NOT rendering.
impl XdgShellHandler for BridgeState {
    fn xdg_shell_state(&mut self)->&mut XdgShellState {&mut self.xdg_shell}
    fn new_toplevel(&mut self,top:ToplevelSurface) {
        self.xdg_toplevel_count=self.xdg_toplevel_count.saturating_add(1);
        if self.managed_xdg_mode {
            // XDG configure requests may precede the first buffer commit.
            // Reserve its stable role slot without pretending pixels exist.
            if let Ok(key)=self.known_window_surfaces.register(top.wl_surface().clone()) {
                if let Some((x,y))=melania_native::window_placement::default_position(key,1280,800) {
                    let rect=WindowRect{x,y,width:360,height:224};
                    if let Some(window)=ManagedWindow::new(rect,1280,800) {
                        self.managed_windows.push((top.wl_surface().clone(),window));
                    }
                }
            }
        }
        if self.dynamic_xdg_resize_mode {
            // Preserve original role order for A/B even before either has
            // attached pixels. The Stage54 QEMU client labels them explicitly.
            self.dynamic_toplevels.push(top.clone());
        }
        top.send_configure();
        println!("MELANIA_NATIVE_XDG_TOPLEVEL_CONFIGURE_SENT=true");
    }
    fn new_popup(&mut self,popup:PopupSurface,_:PositionerState) {
        let _=popup.send_configure();
    }
    fn move_request(&mut self,_:ToplevelSurface,_:wl_seat::WlSeat,_:Serial){}
    fn resize_request(&mut self,_:ToplevelSurface,_:wl_seat::WlSeat,
                      _:Serial,_:xdg_toplevel::ResizeEdge){}
    fn maximize_request(&mut self,top:ToplevelSurface) {
        self.request_managed_maximize(top,true);
    }
    fn unmaximize_request(&mut self,top:ToplevelSurface) {
        self.request_managed_maximize(top,false);
    }
    fn fullscreen_request(&mut self,top:ToplevelSurface,_:Option<WlOutput>) {
        self.request_managed_fullscreen(top,true);
    }
    fn unfullscreen_request(&mut self,top:ToplevelSurface) {
        self.request_managed_fullscreen(top,false);
    }
    fn minimize_request(&mut self,_:ToplevelSurface){}
    fn toplevel_destroyed(&mut self,top:ToplevelSurface){
        let surface=top.wl_surface().clone();
        self.managed_windows.retain(|(role,_)|role!=&surface);
        self.managed_pending_sizes.retain(|(role,_)|role!=&surface);
        if self.dynamic_xdg_resize_mode {
            self.dynamic_toplevels.retain(|existing|existing.wl_surface()!=&surface);
        }
        if let Some(key)=self.known_window_surfaces.remove(&surface) {
            // The retained pixel copy and stable slot belong to this client.
            // Never leave a closed window occupying the limited QEMU scene.
            let removed=if self.dynamic_window_mode {
                self.dynamic_scene.remove(key)
            } else {
                self.multi_scene.remove(key)
            };
            if removed { self.note_scene_change(); }
            if removed && self.continuous_kms_mode &&
               self.continuous_frame_count<self.continuous_frame_target {
                // A destroyed xdg surface changes retained pixels even if
                // no surviving client submits a new wl_shm buffer.
                if let Err(error)=self.submit_continuous_frame() {
                    eprintln!("continuous close KMS frame not scheduled: {error}");
                }
            }
            if removed && self.close_redraw_mode &&
               self.multi_scene.window_count()==1 && self.pixel_frame_count>=3 {
                // Defer the redraw until the outstanding fourth KMS frame
                // receives its ACK. The scheduler coalesces this damage.
                if !self.render_scheduler.damage() {
                    eprintln!("QEMU native close damage ignored on paused seat");
                } else {
                    println!("MELANIA_NATIVE_CLOSE_DAMAGE_QUEUED=true");
                }
            }
            if self.window_interaction.active_key()==Some(key) {
                self.window_interaction.cancel();
            }
            println!("MELANIA_NATIVE_WINDOW_REMOVED={key}");
            if self.close_redraw_mode && key==1 {
                println!("MELANIA_NATIVE_CLIENT_FIRST_WINDOW_DESTROYED=true");
            }
            let _=std::io::stdout().flush();
        }
        if self.focused.as_ref()==Some(&surface) {
            // Do not forward keys or pointer events to a destroyed Wayland
            // role. Where possible retarget the remaining window beneath the
            // last pointer position; otherwise clear keyboard/pointer focus.
            self.focused=None;
            let target=self.pointer_target(self.pointer_position);
            let replacement=target.as_ref().map(|(next,_)|next.clone());
            let keyboard=self.seat.get_keyboard().expect("keyboard");
            keyboard.set_focus(self,replacement.clone(),SERIAL_COUNTER.next_serial());
            self.focused=replacement;
            let pointer=self.seat.get_pointer().expect("pointer");
            pointer.motion(self,target,&MotionEvent{
                location:self.pointer_position,
                serial:SERIAL_COUNTER.next_serial(),time:0,
            });
            pointer.frame(self);
        }
    }
    fn grab(&mut self,_:PopupSurface,_:wl_seat::WlSeat,_:Serial){}
    fn reposition_request(&mut self,popup:PopupSurface,_:PositionerState,token:u32){
        popup.send_repositioned(token);
    }
}
impl WlrLayerShellHandler for BridgeState {
    fn shell_state(&mut self)->&mut WlrLayerShellState {&mut self.layer_shell}
    fn new_layer_surface(&mut self,surface:WlrLayerSurface,
                         _:Option<WlOutput>,layer:WlrLayer,namespace:String) {
        if self.layer_pixels_mode || self.gtk_layer_mode {
            // Keep Stage82/83A's original 2-surface QEMU behavior unless
            // explicitly exercising the real five-namespace GTK shell.
            let configured=if self.gtk_layer_mode {
                let requested=match layer {
                    WlrLayer::Background=>ShellLayer::Background,
                    WlrLayer::Bottom=>ShellLayer::Bottom,
                    WlrLayer::Top=>ShellLayer::Top,
                    WlrLayer::Overlay=>ShellLayer::Overlay,
                };
                let Some(spec)=gtk_layer_geometry::permit(
                    &namespace,requested,
                    self.layer_surfaces.iter().map(|(name,_)|name.as_str())
                ) else {
                    eprintln!("MELANIA_NATIVE_GTK_LAYER_ROLE_REJECTED={namespace}");
                    surface.send_close();
                    return;
                };
                (spec.configured_width as i32,spec.configured_height as i32)
            } else {
                if self.layer_surfaces.len()>=2 {
                    surface.send_close();
                    return;
                }
                (1280,52)
            };
            // Configure based on the real GTK namespace, not one uniform
            // 1280x52 buffer. Each accepted role retains its own identity.
            surface.with_pending_state(|pending|{
                pending.size=Some(configured.into());
            });
            self.layer_surfaces.push((namespace.clone(),surface.clone()));
            if self.gtk_layer_mode {
                println!("MELANIA_NATIVE_GTK_LAYER_CONFIGURED={namespace} size={}x{}",
                         configured.0,configured.1);
            }
        }
        self.layer_surface_count=self.layer_surface_count.saturating_add(1);
        surface.send_configure();
        println!("MELANIA_NATIVE_LAYER_SURFACE_CONFIGURE_SENT=true");
    }
    fn layer_destroyed(&mut self,role:WlrLayerSurface){
        if !self.layer_pixels_mode && !self.gtk_layer_mode {return;}
        let surface=role.wl_surface().clone();
        self.layer_surfaces.retain(|(_,existing)|existing.wl_surface()!=&surface);
        if let Some(key)=self.known_window_surfaces.remove(&surface) {
            if self.dynamic_scene.remove(key) {
                self.note_scene_change();
                // The layer's old pixels must be invalidated and replaced
                // by a subsequently ACKed native KMS frame.
                if self.continuous_frame_count<self.continuous_frame_target {
                    let _=self.submit_continuous_frame();
                }
            }
        }
    }
}
delegate_compositor!(BridgeState);
delegate_seat!(BridgeState);
delegate_xdg_shell!(BridgeState);
delegate_layer_shell!(BridgeState);
delegate_shm!(BridgeState);
delegate_output!(BridgeState);

fn register(state:&mut BridgeState,device:libinput::Device) {
    let caps=melania_native::input_registry::InputCapabilities{
        keyboard:device.has_capability(DeviceCapability::Keyboard),
        pointer:device.has_capability(DeviceCapability::Pointer),
        touch:device.has_capability(DeviceCapability::Touch),
    };
    if caps.any() {
        let _=state.lifecycle.input_registry_mut().device_added(InputDevice::id(&device),caps);
    }
}

/// Relays only actual libinput events belonging to currently registered
/// devices on the ACTIVE libseat generation. No host-wide synthetic key feed.
fn handle_input(state:&mut BridgeState,event:InputEvent<LibinputInputBackend>) {
    match event {
        InputEvent::DeviceAdded {device} =>register(state,device),
        InputEvent::DeviceRemoved {device} =>
            state.lifecycle.input_registry_mut().device_removed(&InputDevice::id(&device)),
        InputEvent::Keyboard {event} => {
            if state.focused.is_none() ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let keyboard=state.seat.get_keyboard().expect("keyboard");
            keyboard.input::<(),_>(state,event.key_code(),event.state(),
                SERIAL_COUNTER.next_serial(),event.time_msec(),
                |_,_,_|FilterResult::Forward);
            state.delivered_keys+=1;
        }
        InputEvent::PointerMotion {event} => {
            if (state.focused.is_none() && !state.multi_window_mode) ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let point:Point<f64,Logical>=(
                (state.pointer_position.x+event.delta_x())
                    .clamp(0.0,if state.multi_window_mode {1279.0} else {639.0}),
                (state.pointer_position.y+event.delta_y())
                    .clamp(0.0,if state.multi_window_mode {799.0} else {479.0})
            ).into();
            state.pointer_position=point;
            state.apply_window_motion(point);
            if state.multi_window_mode {
                println!("MELANIA_NATIVE_WINDOW_POINTER_REL=({:.1},{:.1}) target={:?}",
                    point.x,point.y,state.multi_scene.hit_test(point.x,point.y));
                let _=std::io::stdout().flush();
            }
            let pointer=state.seat.get_pointer().expect("pointer");
            let target=state.pointer_target(point);
            pointer.motion(state,target,
                &MotionEvent{location:point,serial:SERIAL_COUNTER.next_serial(),
                             time:event.time_msec()});
            pointer.frame(state);
            state.delivered_motions+=1;
        }
        InputEvent::PointerMotionAbsolute {event} => {
            if (state.focused.is_none() && !state.multi_window_mode) ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let point=if state.multi_window_mode {
                event.position_transformed((1280,800).into())
            } else {
                event.position_transformed((640,480).into())
            };
            state.pointer_position=point;
            state.apply_window_motion(point);
            if state.multi_window_mode {
                println!("MELANIA_NATIVE_WINDOW_POINTER_ABS=({:.1},{:.1}) target={:?}",
                    point.x,point.y,state.multi_scene.hit_test(point.x,point.y));
                let _=std::io::stdout().flush();
            }
            let pointer=state.seat.get_pointer().expect("pointer");
            let target=state.pointer_target(point);
            pointer.motion(state,target,
                &MotionEvent{location:point,serial:SERIAL_COUNTER.next_serial(),
                             time:event.time_msec()});
            pointer.frame(state);
            state.delivered_motions+=1;
        }
        InputEvent::PointerButton {event} => {
            if (state.focused.is_none() && !state.multi_window_mode) ||
               state.lifecycle.input_registry_mut().event(&InputDevice::id(&event.device())).is_err() {
                return;
            }
            let point=state.pointer_position;
            if state.multi_window_mode {
                let target_key=if state.dynamic_window_mode {
                    state.dynamic_scene.hit_test(point.x,point.y)
                } else {
                    state.multi_scene.hit_test(point.x,point.y)
                };
                println!("MELANIA_NATIVE_WINDOW_BUTTON=({:.1},{:.1}) target={:?} state={:?}",
                    point.x,point.y,target_key,event.state());
                let _=std::io::stdout().flush();
            }
            let pointer=state.seat.get_pointer().expect("pointer");
            if state.multi_window_mode {
                let target=state.pointer_target(point);
                pointer.motion(state,target.clone(),
                    &MotionEvent{location:point,
                        serial:SERIAL_COUNTER.next_serial(),time:event.time_msec()});
                // A click, unlike motion, raises a client and grants
                // keyboard focus to its actual wl_surface.
                if event.state()==ButtonState::Pressed {
                    if let Some((surface,_))=target {
                        let id=if state.dynamic_window_mode {
                            state.dynamic_scene.hit_test(point.x,point.y)
                        } else {
                            state.multi_scene.hit_test(point.x,point.y)
                        };
                        if let Some(id)=id {
                            if state.dynamic_window_mode {
                                if state.dynamic_scene.raise(id).unwrap_or(false) {
                                    state.note_scene_change();
                                    if state.continuous_frame_count<state.continuous_frame_target {
                                        if let Err(error)=state.submit_continuous_frame() {
                                            eprintln!("dynamic QEMU raise KMS rejected: {error}");
                                        }
                                    }
                                }
                            } else {
                                state.multi_scene.raise(id);
                            }
                            if event.button_code()==0x110 {
                                let geometry=if state.dynamic_window_mode {
                                    state.dynamic_scene.geometry(id)
                                        .map(|(x,y,w,h)|(x,y,w as i32,h as i32))
                                } else {
                                    state.multi_scene.position(id)
                                        .map(|(x,y)|(x,y,360,224))
                                };
                                if let Some((x,y,width,height))=geometry {
                                    let right=f64::from(x)+f64::from(width);
                                    let bottom=f64::from(y)+f64::from(height);
                                    let kind=if point.x>=right-16.0 &&
                                                point.y>=bottom-16.0 {
                                        Some(InteractionKind::ResizeBottomRight)
                                    } else if point.y>=f64::from(y) &&
                                              point.y<f64::from(y+26) {
                                        Some(InteractionKind::Move)
                                    } else {None};
                                    if let Some(kind)=kind {
                                        let original=WindowRect {x,y,width,height};
                                        if state.window_interaction.begin(
                                            id,original,point.x,point.y,kind
                                        ) {
                                            println!("MELANIA_NATIVE_WINDOW_INTERACTION_BEGIN={id} kind={kind:?}");
                                        }
                                    }
                                }
                            }
                            let keyboard=state.seat.get_keyboard().expect("keyboard");
                            keyboard.set_focus(state,Some(surface.clone()),
                                SERIAL_COUNTER.next_serial());
                            state.focused=Some(surface);
                            println!("MELANIA_NATIVE_WINDOW_POINTER_FOCUS={id}");
                            // The QEMU injector observes this event via
                            // the redirected guest serial log. Flush NOW,
                            // not after the next multi-second DRM ACK.
                            let _=std::io::stdout().flush();
                        }
                    }
                }
            }
            pointer.button(state,&ButtonEvent{
                button:event.button_code(),state:event.state(),
                serial:SERIAL_COUNTER.next_serial(),time:event.time_msec(),
            });
            pointer.frame(state);
            if state.multi_window_mode && event.button_code()==0x110 &&
               event.state()==ButtonState::Released {
                if let Some(id)=state.window_interaction.end() {
                    println!("MELANIA_NATIVE_WINDOW_INTERACTION_END={id}");
                }
            }
            state.delivered_buttons+=1;
        }
        _=>{}
    }
}

pub fn run() -> Result<(),Box<dyn Error>> {
    // Main has already required explicit consent, QEMU DMI and dedicated VT.
    let (session,notifier)=LibSeatSession::new()?;
    let seat_name=session.seat();
    let mut context=Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
        session.clone().into()
    );
    context.udev_assign_seat(&seat_name)
        .map_err(|_|"Cannot bind libinput to QEMU libseat")?;
    let mut display=Display::<BridgeState>::new()?;
    let mut handle=display.handle();
    let output=Output::new("MELANIA-QEMU-1".into(),PhysicalProperties{
        size:(0,0).into(),subpixel:Subpixel::Unknown,
        make:"Melania".into(),model:"Virtual display (QEMU only)".into(),
    });
    // Protocol-advertised virtual output, not proof of a live KMS renderer.
    // Mode matches the VM-only 1280x800 scanout snapshot tested in Stage 26.
    let virtual_mode=Mode{size:(1280,800).into(),refresh:60_000};
    output.create_global::<BridgeState>(&handle);
    output.change_current_state(Some(virtual_mode),Some(Transform::Normal),
                                None,Some((0,0).into()));
    output.set_preferred(virtual_mode);
    let mut seats=SeatState::new();
    let mut seat=seats.new_wl_seat(&handle,"melania-native-seat0");
    seat.add_keyboard(Default::default(),250,40)?;
    seat.add_pointer();
    let mut state=BridgeState{
        compositor:CompositorState::new::<BridgeState>(&handle),
        shm:ShmState::new::<BridgeState>(&handle,vec![]),
        output_manager:OutputManagerState::new_with_xdg_output::<BridgeState>(&handle),
        output,
        pixel_probe:std::env::var("MELANIA_NATIVE_CLIENT_PIXELS")
            .as_deref()==Ok("YES_QEMU_VM"),pixel_ready:false,
        pixel_frame_count:0,
        pixel_frame_target:if std::env::var("MELANIA_NATIVE_REOPEN_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM") {6} else if std::env::var("MELANIA_NATIVE_CLOSE_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM") {5} else if std::env::var("MELANIA_NATIVE_DRAG_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM") {4} else if std::env::var("MELANIA_NATIVE_LIVE_STREAM")
            .as_deref()==Ok("YES_QEMU_VM") {3} else if std::env::var("MELANIA_NATIVE_CLIENT_UPDATES")
            .as_deref()==Ok("YES_QEMU_VM") {2} else {1},
        live_stream:std::env::var("MELANIA_NATIVE_LIVE_STREAM")
            .as_deref()==Ok("YES_QEMU_VM"),
        frame_callbacks_probe:std::env::var("MELANIA_NATIVE_PRESENT_FRAME_CALLBACKS")
            .as_deref()==Ok("YES_QEMU_VM"),
        frame_callback_count:0,
        render_scheduler:RenderScheduler::default(),
        continuous_redraw:ContinuousRedraw::default(),continuous_pending:None,
        continuous_kms_mode:std::env::var("MELANIA_NATIVE_CONTINUOUS_KMS")
            .as_deref()==Ok("YES_QEMU_VM"),
        continuous_worker:None,continuous_inflight:None,
        continuous_client_ledger:if std::env::var("MELANIA_NATIVE_DYNAMIC_SCENE")
            .as_deref()==Ok("YES_QEMU_VM") {
                ClientCommitLedger::with_capacity(16).expect("bounded QEMU receipts")
            } else {ClientCommitLedger::new()}, 
        continuous_frame_count:0,
        continuous_frame_target:if std::env::var("MELANIA_NATIVE_CONTINUOUS_REOPEN")
            .as_deref()==Ok("YES_QEMU_VM") {14} else if std::env::var("MELANIA_NATIVE_DYNAMIC_POINTER_PROOF")
            .as_deref()==Ok("YES_QEMU_VM") {13} else {12},
        continuous_reopen_mode:std::env::var("MELANIA_NATIVE_CONTINUOUS_REOPEN")
            .as_deref()==Ok("YES_QEMU_VM"),
        continuous_close_frame_presented:false,
        continuous_reopen_frame_presented:false,
        continuous_two_window_presented:false,
        multi_window_mode:std::env::var("MELANIA_NATIVE_TWO_WINDOWS")
            .as_deref()==Ok("YES_QEMU_VM"),
        async_kms_mode:std::env::var("MELANIA_NATIVE_ASYNC_KMS")
            .as_deref()==Ok("YES_QEMU_VM"),
        drag_redraw_mode:std::env::var("MELANIA_NATIVE_DRAG_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM"),
        close_redraw_mode:std::env::var("MELANIA_NATIVE_CLOSE_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM"),close_frame_presented:false,
        reopen_redraw_mode:std::env::var("MELANIA_NATIVE_REOPEN_REDRAW")
            .as_deref()==Ok("YES_QEMU_VM"),reopen_frame_presented:false,
        presented_layout_generation:0,
        drag_frame_presented:false,two_window_frame_presented:false,
        async_stream:None,async_pending:None,
        multi_scene:MultiWindowScene::default(),
        dynamic_window_mode:std::env::var("MELANIA_NATIVE_DYNAMIC_SCENE")
            .as_deref()==Ok("YES_QEMU_VM"),
        dynamic_scene:DynamicWindowScene::new(1280,800,[58,33,27,0])
            .expect("bounded disposable QEMU output"),
        dynamic_confirmed_frame:None,
        dynamic_submitted_frame:None,
        dynamic_xdg_resize_mode:std::env::var("MELANIA_NATIVE_XDG_RESIZE")
            .as_deref()==Ok("YES_QEMU_VM"),
        dynamic_pointer_proof_mode:std::env::var("MELANIA_NATIVE_DYNAMIC_POINTER_PROOF")
            .as_deref()==Ok("YES_QEMU_VM"),
        dynamic_toplevels:Vec::new(),
        managed_xdg_mode:std::env::var("MELANIA_NATIVE_MANAGED_XDG")
            .as_deref()==Ok("YES_QEMU_VM"),
        managed_windows:Vec::new(),
        managed_pending_sizes:Vec::new(),
        window_interaction:WindowInteraction::default(),
        known_window_surfaces:if std::env::var("MELANIA_NATIVE_DYNAMIC_SCENE")
            .as_deref()==Ok("YES_QEMU_VM") {
                WindowSlots::with_capacity(16).expect("bounded QEMU window registry")
            } else {WindowSlots::default()}, 
        stream:None,
        xdg_shell:XdgShellState::new::<BridgeState>(&handle),
        layer_shell:WlrLayerShellState::new::<BridgeState>(&handle),
        layer_pixels_mode:std::env::var("MELANIA_NATIVE_LAYER_PIXELS")
            .as_deref()==Ok("YES_QEMU_VM"),
        gtk_layer_mode:std::env::var("MELANIA_NATIVE_GTK_MULTISURFACE")
            .as_deref()==Ok("YES_QEMU_VM"),
        layer_surfaces:Vec::new(),
        xdg_toplevel_count:0,layer_surface_count:0,
        seats,seat,focused:None,pointer_position:(0.0,0.0).into(),
        lifecycle:NativeLifecycle::default(),delivered_keys:0,
        delivered_buttons:0,delivered_motions:0,
    };
    if session.is_active() {
        state.lifecycle.on_session(SessionEvent::ActivateSession);
        state.render_scheduler.set_seat_active(true);
        state.continuous_redraw.set_seat_active(true);
    }
    context.dispatch()?;
    for event in &mut context {
        if let libinput::Event::Device(libinput::event::DeviceEvent::Added(added))=event {
            register(&mut state,libinput::event::EventTrait::device(&added));
        }
    }
    let backend=LibinputInputBackend::new(context.clone());
    let mut events=calloop::EventLoop::<BridgeState>::try_new()?;
    events.handle().insert_source(backend,|event,_,state|handle_input(state,event))?;
    events.handle().insert_source(notifier,move |event,_,state|{
        match event {
            SessionEvent::PauseSession=>context.suspend(),
            SessionEvent::ActivateSession=>{
                if context.resume().is_err() {
                    state.lifecycle.on_session(SessionEvent::PauseSession);
                    state.render_scheduler.set_seat_active(false);
                    state.continuous_redraw.set_seat_active(false);
                    state.continuous_pending=None;
                    return;
                }
            }
        }
        let active=matches!(event,SessionEvent::ActivateSession);
        state.lifecycle.on_session(event);
        state.render_scheduler.set_seat_active(active);
        state.continuous_redraw.set_seat_active(active);
        if !active {
            state.continuous_pending=None;
            state.window_interaction.cancel();
        }
    })?;

    let live_stream=std::env::var("MELANIA_NATIVE_LIVE_STREAM")
        .as_deref()==Ok("YES_QEMU_VM");
    if live_stream && !state.pixel_probe {
        return Err("live KMS stream needs explicit QEMU client-pixel probe".into());
    }
    if state.frame_callbacks_probe && !live_stream {
        return Err("frame presentation probe needs live VM Rust DRM stream".into());
    }
    if state.continuous_kms_mode &&
       (!live_stream || !state.frame_callbacks_probe || state.async_kms_mode) {
        return Err("continuous KMS requires a dedicated QEMU probe".into());
    }
    let continuous_two_windows=std::env::var("MELANIA_NATIVE_CONTINUOUS_TWO_WINDOWS")
        .as_deref()==Ok("YES_QEMU_VM");
    if state.continuous_kms_mode &&
       (state.multi_window_mode != continuous_two_windows) {
        return Err("continuous QEMU window layout flags must agree".into());
    }
    if state.continuous_reopen_mode &&
       (!state.continuous_kms_mode || !state.multi_window_mode) {
        return Err("continuous reopen requires two real windows in disposable QEMU".into());
    }
    if state.dynamic_window_mode &&
       (!state.continuous_kms_mode || !state.multi_window_mode ||
        state.continuous_reopen_mode || !continuous_two_windows) {
        return Err("dynamic KMS requires dedicated two-window QEMU proof".into());
    }
    let gtk_role_protocol_only=std::env::var("MELANIA_NATIVE_SHELL_PROBE")
        .as_deref()==Ok("YES_QEMU_VM");
    if state.gtk_layer_mode && !state.layer_pixels_mode && !gtk_role_protocol_only {
        return Err("native GTK multisurface requires QEMU layer pixels or role-only smoke".into());
    }
    if state.layer_pixels_mode &&
       (!state.dynamic_window_mode || !state.continuous_kms_mode ||
        !state.multi_window_mode || !state.frame_callbacks_probe ||
        state.continuous_reopen_mode) {
        return Err("native layer pixels require isolated QEMU dynamic multi-client KMS".into());
    }
    if state.managed_xdg_mode && !state.dynamic_window_mode &&
       std::env::var("MELANIA_NATIVE_SHELL_PROBE").as_deref()!=Ok("YES_QEMU_VM") {
        return Err("managed xdg requires QEMU dynamic KMS or role-only shell probe".into());
    }
    if state.dynamic_xdg_resize_mode && !state.dynamic_window_mode {
        return Err("xdg resize configure is only permitted in disposable QEMU dynamic mode".into());
    }
    if state.dynamic_pointer_proof_mode &&
       (!state.dynamic_xdg_resize_mode || !state.dynamic_window_mode ||
        !state.continuous_kms_mode || state.continuous_reopen_mode) {
        return Err("dynamic pointer frame13 requires real xdg resize and opt-in VM KMS".into());
    }
    if state.multi_window_mode && !state.continuous_kms_mode &&
       (!live_stream || !state.frame_callbacks_probe ||
        state.pixel_frame_target!=if state.reopen_redraw_mode {6} else if state.close_redraw_mode {5} else if state.drag_redraw_mode {4} else {3}) {
        return Err("two real windows require bounded QEMU streaming and callbacks".into());
    }
    if state.async_kms_mode && !state.multi_window_mode {
        return Err("nonblocking KMS probe only enabled for QEMU two-window mode".into());
    }
    if state.drag_redraw_mode &&
       (!state.multi_window_mode || !state.async_kms_mode) {
        return Err("native layout redraw only supported with QEMU async two-window mode".into());
    }
    if state.close_redraw_mode &&
       (!state.drag_redraw_mode || !state.async_kms_mode || !state.live_stream) {
        return Err("close redraw needs QEMU two-window async KMS drag mode".into());
    }
    if state.reopen_redraw_mode &&
       (!state.close_redraw_mode || !state.multi_window_mode) {
        return Err("reopen redraw requires an isolated QEMU close-redraw test".into());
    }
    let frame_listener=if live_stream {
        let path=if state.continuous_kms_mode {
            "/run/melania-native-qemu/continuous-frame-stream.sock"
        } else {"/run/melania-native-qemu/frame-stream.sock"};
        let listener=UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;
        println!("MELANIA_NATIVE_FRAME_STREAM_SOCKET_READY=true");
        Some(listener)
    } else {None};
    let listener=ListeningSocket::bind_auto("melania-native-seat",1..=8)?;
    let socket=listener.socket_name().ok_or("Missing native Wayland socket name")?;
    println!("MELANIA_NATIVE_WAYLAND_SOCKET_READY={}",socket.to_string_lossy());
    println!("MELANIA_NATIVE_WAYLAND_RENDER_READY=false");
    println!("MELANIA_NATIVE_WL_OUTPUT_ADVERTISED=1280x800");
    std::io::stdout().flush()?;
    // Only a disposable QEMU VM may select shell protocol handshake mode.
    // The parent process enforces the opt-in, DMI and Linux VT gates.
    let shell_probe=std::env::var("MELANIA_NATIVE_SHELL_PROBE")
        .as_deref()==Ok("YES_QEMU_VM");
    let deadline=Instant::now()+Duration::from_secs(
        if state.continuous_kms_mode {70} else if state.reopen_redraw_mode {68} else if state.close_redraw_mode {55} else if state.drag_redraw_mode {45} else {27}
    );
    let mut both_shell_roles_seen_at=None;
    let mut pixel_ready_since=None;
    let mut continuous_ready_since=None;
    while Instant::now()<deadline {
        // Accept the Rust DRM consumer before processing client commits;
        // the socket is private to this disposable VM, not the host.
        if state.live_stream && state.stream.is_none() && state.async_stream.is_none() && state.continuous_worker.is_none() {
            if let Some(listener)=&frame_listener {
                match listener.accept() {
                    Ok((stream,_))=>{
                        stream.set_write_timeout(Some(Duration::from_secs(7)))?;
                        stream.set_read_timeout(Some(Duration::from_secs(8)))?;
                        if state.continuous_kms_mode {
                            state.continuous_worker=Some(ContinuousKmsWorker::spawn(stream));
                            println!("MELANIA_NATIVE_CONTINUOUS_KMS_WORKER_STARTED=true");
                        } else if state.async_kms_mode {
                            state.async_stream=Some(AsyncKmsWorker::spawn(stream));
                            println!("MELANIA_NATIVE_ASYNC_KMS_WORKER_STARTED=true");
                        } else {
                            state.stream=Some(stream);
                        }
                        println!("MELANIA_NATIVE_FRAME_STREAM_CONNECTED=true");
                        let _=std::io::stdout().flush();
                    }
                    Err(error) if error.kind()==std::io::ErrorKind::WouldBlock=>{},
                    Err(error)=>return Err(error.into()),
                }
            }
        }
        events.dispatch(Some(Duration::from_millis(12)),&mut state)?;
        while let Some(stream)=listener.accept()? {
            handle.insert_client(stream,Arc::new(BridgeClientState::default()))?;
        }
        display.dispatch_clients(&mut state)?;
        if state.async_kms_mode && state.async_stream.is_some() {
            state.poll_async_kms()?;
        }
        if state.continuous_kms_mode && state.continuous_worker.is_some() {
            state.poll_continuous_kms()?;
        }
        display.flush_clients()?;
        if state.continuous_kms_mode &&
           state.continuous_frame_count==state.continuous_frame_target {
            // Frame 13 in the optional tablet proof is compositor-owned
            // damage: NO new client commit and therefore NO callback.
            let expected_callbacks=if state.dynamic_pointer_proof_mode {12}
                else {state.continuous_frame_target};
            if u64::from(state.frame_callback_count)!=expected_callbacks {
                return Err("Wayland callbacks not completed after continuous KMS".into());
            }
            if state.multi_window_mode && !state.continuous_two_window_presented {
                return Err("continuous KMS did not present both retained windows".into());
            }
            if state.continuous_reopen_mode &&
               (!state.continuous_close_frame_presented ||
                !state.continuous_reopen_frame_presented) {
                return Err("continuous close and reopen never reached the real CRTC".into());
            }
            // Keep dispatching display.sync for the client after the last
            // callback, rather than shutting down its socket prematurely.
            let since=continuous_ready_since.get_or_insert_with(Instant::now);
            if since.elapsed()>=Duration::from_secs(2) {
                println!("MELANIA_NATIVE_CONTINUOUS_WAYLAND_KMS_FRAMES_PASS={}",
                         state.continuous_frame_target);
                if state.continuous_reopen_mode {
                    println!("MELANIA_NATIVE_CONTINUOUS_REOPEN_14_FRAMES_PASS=true");
                }
                if state.dynamic_pointer_proof_mode {
                    println!("MELANIA_NATIVE_DYNAMIC_POINTER_FRAME13_PASS=true");
                }
                println!("MELANIA_NATIVE_SESSION_READY=false");
                return Ok(());
            }
        }
        if state.pixel_probe && state.pixel_ready {
            let since=pixel_ready_since.get_or_insert_with(Instant::now);
            if since.elapsed()>=Duration::from_secs(
                if state.multi_window_mode {6} else {3}
            ) {
                println!("MELANIA_NATIVE_CLIENT_FRAME_SNAPSHOT_READY=true");
                println!("MELANIA_NATIVE_CLIENT_FRAME_COUNT={}",state.pixel_frame_count.min(3));
                if state.drag_redraw_mode {
                    // Destruction dirties the next scene, but cannot undo the
                    // fourth frame which KMS already acknowledged.
                    if state.pixel_frame_count<4 || !state.drag_frame_presented {
                        return Err("QEMU drag frame not acknowledged by KMS".into());
                    }
                    println!("MELANIA_NATIVE_DRAG_FRAME_VISIBLE=true");
                }
                if state.close_redraw_mode {
                    if state.pixel_frame_count<5 || !state.close_frame_presented {
                        return Err("QEMU close redraw not acknowledged by KMS".into());
                    }
                    println!("MELANIA_NATIVE_CLOSE_FRAME_VISIBLE=true");
                }
                if state.reopen_redraw_mode {
                    if state.pixel_frame_count!=6 || !state.reopen_frame_presented {
                        return Err("QEMU sixth KMS frame never presented both window surfaces".into());
                    }
                    println!("MELANIA_NATIVE_REOPEN_FRAME_VISIBLE=true");
                }
                if state.live_stream {
                    println!("MELANIA_NATIVE_KMS_STREAM_THREE_FRAMES=true");
                }
                if state.multi_window_mode {
                    if !state.two_window_frame_presented {
                        return Err("QEMU native scene never presented both client windows".into());
                    }
                    println!("MELANIA_NATIVE_TWO_REAL_WINDOWS_COMPOSITED=true");
                }
                if state.frame_callbacks_probe {
                    if state.frame_callback_count!=if state.reopen_redraw_mode {5} else if state.close_redraw_mode {4} else if state.drag_redraw_mode {3} else {state.pixel_frame_target as u32} {
                        return Err("not all Wayland frame callbacks completed after KMS".into());
                    }
                    println!("MELANIA_NATIVE_FRAME_CALLBACKS_PRESENTED_PASS=true");
                }
                println!("MELANIA_NATIVE_LIVE_KMS_RENDERING=false");
                println!("MELANIA_NATIVE_SESSION_READY=false");
                return Ok(());
            }
        }
        if shell_probe && (if state.gtk_layer_mode {
            state.layer_surface_count>=gtk_layer_geometry::MAX_GTK_LAYER_SURFACES as u32
        } else {state.xdg_toplevel_count>0 && state.layer_surface_count>0}) {
            let since=both_shell_roles_seen_at.get_or_insert_with(Instant::now);
            // Let the real client receive, ack and finish its configure
            // roundtrips before closing the Wayland server.
            if since.elapsed()>=Duration::from_secs(3) {
                println!("MELANIA_NATIVE_XDG_TOPLEVEL_COUNT={}",state.xdg_toplevel_count);
                println!("MELANIA_NATIVE_LAYER_SURFACE_COUNT={}",state.layer_surface_count);
                println!("MELANIA_NATIVE_SHELL_PROTOCOLS_READY=true");
                println!("MELANIA_NATIVE_SHELL_RENDERED=false");
                println!("MELANIA_NATIVE_SESSION_READY=false");
                return Ok(());
            }
        }
        if !shell_probe && !state.pixel_probe && state.delivered_keys>=2 && state.delivered_buttons>=2 && state.focused.is_some() {
            println!("MELANIA_NATIVE_WAYLAND_KEYBOARD_EVENTS={}",state.delivered_keys);
            println!("MELANIA_NATIVE_WAYLAND_POINTER_BUTTON_EVENTS={}",state.delivered_buttons);
            println!("MELANIA_NATIVE_WAYLAND_INPUT_FORWARDED=true");
            println!("MELANIA_NATIVE_SESSION_READY=false");
            return Ok(());
        }
    }
    Err(format!(
        "QEMU Wayland input bridge incomplete: focused={} keys={} buttons={}",
        state.focused.is_some(),state.delivered_keys,state.delivered_buttons
    ).into())
}

#[cfg(test)]
mod managed_role_commit_tests {
    use super::accepts_managed_role_buffer;

    #[test]
    fn role_resize_refuses_stale_client_images_until_matching_wl_shm_arrives() {
        assert!(accepts_managed_role_buffer(None,(360,224)));
        assert!(accepts_managed_role_buffer(None,(400,240)));
        assert!(!accepts_managed_role_buffer(Some((1280,800)),(360,224)));
        assert!(!accepts_managed_role_buffer(Some((1280,800)),(1280,799)));
        assert!(accepts_managed_role_buffer(Some((1280,800)),(1280,800)));
        assert!(!accepts_managed_role_buffer(Some((360,224)),(1280,800)));
        assert!(accepts_managed_role_buffer(Some((360,224)),(360,224)));
    }
}
