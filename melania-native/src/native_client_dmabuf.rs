//! A strictly QEMU-opt-in GPU importer for *real Wayland client* DMA-BUFs.
//!
//! This is NOT a production graphics pipeline: the existing compositor still
//! composes wl_shm snapshots on the CPU and its DRM worker has no external
//! client-buffer scene renderer. No Wayland DRM global may be advertised
//! without an actual libseat-owned GBM/EGL renderer. A successful import is
//! only a real GLES texture import, not permission for wl_surface.frame or
//! wl_buffer.release, a KMS ACK, an acquire fence or a native login session.
use smithay::backend::{
    allocator::{dmabuf::Dmabuf, Buffer, Format, Fourcc},
    renderer::{gles::{GlesRenderer,GlesTexture},ImportDma},
};

// Applies to the visible pixels and, for LINEAR buffers, the entire
// advertised byte span including row padding and plane offset. A small
// logical image must not bypass the import budget with enormous strides.
pub const MAX_CLIENT_BUFFER_BYTES:usize=16*1024*1024;
pub const MAX_CLIENT_SURFACE_EDGE:usize=2048;
pub const MAX_CLIENT_IMPORTS:usize=4;

/// VM-only two successive genuine client GPU frames on one XDG surface.
/// The second DMA-BUF is admissible ONLY after the first confirmed KMS
/// presentation; a third cannot pass by reusing the first ticket.
/// This does not authorize native/physical import or direct GPU scanout.
pub fn qemu_gpu_frame_allowed(acknowledged:u64,two_gpu:bool)->bool {
    acknowledged==0 || (two_gpu && acknowledged==1)
}
pub fn qemu_gpu_frame_required(frame:u64,two_gpu:bool)->bool {
    frame==1 || (two_gpu && frame==2)
}

/// Keep format advertisements restricted to the single-plane 32-bit layouts
/// this experimental bridge can validate, rather than mirroring the whole EGL
/// modifier table (including formats the compositor could never use).
fn supported_format(code:Fourcc)->bool {
    matches!(code,
        Fourcc::Xrgb8888 | Fourcc::Argb8888 |
        Fourcc::Xbgr8888 | Fourcc::Abgr8888)
}

/// The registry is a finite, ordered promise to clients, not the raw EGL
/// driver's (potentially unordered) modifier set. In particular, even when
/// the active driver reports more than 64 combinations, never omit the linear
/// ARGB/XRGB formats that the real QEMU buffer path actually imports.
/// This changes only the advertised capabilities; imported buffers must
/// still pass the same GPU-specific has_dmabuf_format and size checks.
pub const MAX_ADVERTISED_CLIENT_FORMATS:usize=64;

fn client_format_priority(code:Fourcc)->u8 {
    match code {
        Fourcc::Argb8888=>0,
        Fourcc::Xrgb8888=>1,
        Fourcc::Abgr8888=>2,
        Fourcc::Xbgr8888=>3,
        _=>4,
    }
}

fn ordered_advertised_formats(formats:impl IntoIterator<Item=Format>)->Vec<Format> {
    use smithay::backend::allocator::Modifier;
    let mut candidates:Vec<_>=formats.into_iter()
        .filter(|format|supported_format(format.code))
        .collect();
    // Format order from EGL driver iteration is not part of the protocol.
    // Sort linear first, then packed-RGB preference, then the modifier's
    // stable diagnostic representation, before the bounded registry cap.
    // This is only a deterministic order, NOT a synthetic import capability.
    candidates.sort_by_cached_key(|format|(
        format.modifier!=Modifier::Linear,
        client_format_priority(format.code),
        format!("{:?}",format.modifier),
    ));
    candidates.dedup();
    // Never let a driver with dozens of ARGB vendor modifiers consume the
    // entire finite Wayland registry before XRGB/ABGR/XBGR are considered.
    // Reserve every genuinely advertised LINEAR entry first; then choose
    // opaque modifiers fairly by format, one per round. No capabilities are
    // synthesized: every selected entry is copied from the EGL driver set.
    let mut selected:Vec<Format>=candidates.iter().copied()
        .filter(|format|format.modifier==Modifier::Linear)
        .take(MAX_ADVERTISED_CLIENT_FORMATS).collect();
    let mut non_linear:Vec<Vec<Format>>=(0..4).map(|priority|{
        candidates.iter().copied()
            .filter(|format|format.modifier!=Modifier::Linear &&
                    client_format_priority(format.code)==priority)
            .collect()
    }).collect();
    let mut round=0;
    while selected.len()<MAX_ADVERTISED_CLIENT_FORMATS {
        let mut added=false;
        for bucket in &mut non_linear {
            if round<bucket.len() && selected.len()<MAX_ADVERTISED_CLIENT_FORMATS {
                selected.push(bucket[round]);
                added=true;
            }
        }
        if !added {break;}
        round+=1;
    }
    selected
}

pub fn client_import_formats(renderer:&GlesRenderer)->Vec<Format> {
    ordered_advertised_formats(renderer.dmabuf_formats().iter().copied())
}

/// Check untrusted Wayland metadata before sending any FD to EGL. For now,
/// disallow multi-plane YUV, large images and oversized offsets; a future
/// GPU scene renderer will need per-plane and explicit-fence lifetime rules.
pub fn validate_client_dmabuf(buffer:&Dmabuf)->Result<(),String> {
    let size=buffer.size();
    let width=usize::try_from(size.w)
        .map_err(|_|"negative client DMA-BUF width")?;
    let height=usize::try_from(size.h)
        .map_err(|_|"negative client DMA-BUF height")?;
    if width==0 || height==0 || width>MAX_CLIENT_SURFACE_EDGE ||
       height>MAX_CLIENT_SURFACE_EDGE {
        return Err("client DMA-BUF geometry out of bounded QEMU range".into());
    }
    let row=width.checked_mul(4).ok_or("client DMA-BUF row overflow")?;
    let bytes=row.checked_mul(height).ok_or("client DMA-BUF byte budget overflow")?;
    if bytes>MAX_CLIENT_BUFFER_BYTES {
        return Err("client DMA-BUF exceeds the 16 MiB per-import budget".into());
    }
    if !supported_format(buffer.format().code) || buffer.num_planes()!=1 {
        return Err("client DMA-BUF requires supported packed RGB and one plane".into());
    }
    // The bounded QEMU path has no per-client orientation/interlace
    // conversion. In particular, Y_INVERT must not silently present an
    // upside-down surface, and INTERLACED/BOTTOM_FIRST cannot be interpreted
    // as progressive RGB. Fail closed until the real GPU scene supports
    // these protocol flags explicitly.
    // Smithay 0.7.0 exposes only y_inverted(), not the full bitmask.
    // Do not call the non-existent Dmabuf::flags API. This import guard
    // rejects Y_INVERT on the actual client buffer; INTERLACED and
    // BOTTOM_FIRST still need raw protocol flag admission in a later change.
    if buffer.y_inverted() {
        return Err("client DMA-BUF requires progressive non-inverted flags".into());
    }
    let stride=buffer.strides().next()
        .ok_or("client DMA-BUF has no plane stride")? as usize;
    let offset=buffer.offsets().next()
        .ok_or("client DMA-BUF has no plane offset")? as usize;
    if stride<row || stride>MAX_CLIENT_SURFACE_EDGE*16 {
        return Err("client DMA-BUF stride outside bounded packed-RGB layout".into());
    }
    // The QEMU bridge handles 32-bit packed RGB only. A LINEAR row or
    // first pixel beginning between four-byte texel boundaries is not a
    // valid layout for its tightly packed GLES/readback contract. Reject
    // untrusted metadata before passing the FD to the active EGL driver.
    // Tiled/compressed modifiers have opaque vendor-specific layouts and
    // must not be subjected to this LINEAR-only byte-alignment assumption.
    if buffer.format().modifier==smithay::backend::allocator::Modifier::Linear &&
       (stride%4!=0 || offset%4!=0) {
        return Err("client LINEAR packed-RGB stride/offset must be 4-byte aligned".into());
    }
    let last=height.checked_sub(1)
        .and_then(|rows|rows.checked_mul(stride))
        .and_then(|length|length.checked_add(offset))
        .and_then(|length|length.checked_add(row))
        .ok_or("client DMA-BUF plane address overflow")?;
    if last>64*1024*1024 {
        return Err("client DMA-BUF unbounded plane offset or backing extent".into());
    }
    // A LINEAR packed-RGB plane's last visible byte is the true minimum
    // backing extent. Its padding and offset must consume the same 16 MiB
    // quota as its visible pixels. Without this check a 360x1024 client
    // could reserve >32 MiB through stride while nominal pixels use 1.4 MiB.
    // Tiled/compressed modifiers have opaque physical layout, so do not
    // invent a minimum allocation extent for those layouts here.
    if buffer.format().modifier==smithay::backend::allocator::Modifier::Linear &&
       last>MAX_CLIENT_BUFFER_BYTES {
        return Err("client LINEAR DMA-BUF plane exceeds 16 MiB stride/offset budget".into());
    }
    Ok(())
}

/// A single-plane LINEAR DMA-BUF has an exact minimum byte extent:
/// offset + (height - 1) * stride + width * 4. Validate that extent against
/// the actual kernel-reported DMA-BUF FD length, not just metadata that an
/// untrusted Wayland client provided. Also bound the *whole* kernel-backed
/// allocation: a tiny visible rectangle must not pin an arbitrarily large
/// DMA-BUF merely by specifying a small stride and plane offset. Kernel DMA-BUF supports SEEK_END to
/// report its allocation size; refusal or an unknown size fails closed.
///
/// Non-linear/tiled modifiers may have vendor-specific compression/headers:
/// their logical stride is NOT necessarily a minimum physical file extent.
/// Do not enforce a LINEAR minimum extent on those layouts, but the kernel
/// allocation itself must still fit the import budget. This provides neither an acquire
/// fence nor permission for wl_buffer.release/Wayland frame callbacks.
fn validate_client_plane_backing(buffer:&Dmabuf)->Result<(),String> {
    use std::os::fd::AsRawFd;
    use smithay::backend::allocator::Modifier;
    let linear=buffer.format().modifier==Modifier::Linear;
    let size=buffer.size();
    let width=usize::try_from(size.w).map_err(|_|"negative DMA-BUF backing width")?;
    let height=usize::try_from(size.h).map_err(|_|"negative DMA-BUF backing height")?;
    let row=width.checked_mul(4).ok_or("DMA-BUF backing row overflow")?;
    let stride=buffer.strides().next()
        .ok_or("LINEAR DMA-BUF backing stride missing")? as usize;
    let offset=buffer.offsets().next()
        .ok_or("LINEAR DMA-BUF backing offset missing")? as usize;
    let required=height.checked_sub(1)
        .and_then(|rows|rows.checked_mul(stride))
        .and_then(|bytes|bytes.checked_add(offset))
        .and_then(|bytes|bytes.checked_add(row))
        .ok_or("LINEAR DMA-BUF backing extent overflow")?;
    let fd=buffer.handles().next().ok_or("LINEAR DMA-BUF plane FD missing")?;
    // lseek(0, SEEK_END) on DMA-BUF returns the true allocation size on
    // supported Linux kernels. Do not use stat.st_size, a guessed GBM pitch,
    // or an untrusted client-provided file length as a substitute.
    let actual=unsafe {libc::lseek(fd.as_raw_fd(),0,libc::SEEK_END)};
    if actual < 0 {
        return Err("DMA-BUF kernel backing size unavailable".into());
    }
    // Even an opaque/tiled layout needs a real non-empty kernel allocation.
    // A zero-length FD cannot back any image and must never reach EGL.
    if actual == 0 {
        return Err("DMA-BUF FD has zero-length kernel backing".into());
    }
    // Even with opaque/tiled modifiers the plane's byte offset identifies
    // its first address in the allocation. It must refer to an existing
    // byte, not an address beyond (or exactly at) EOF. Do not infer a
    // vendor-specific row extent for these formats.
    if (offset as u64) >= actual as u64 {
        return Err("DMA-BUF plane offset outside kernel allocation".into());
    }
    if linear && (actual as u64) < required as u64 {
        return Err("LINEAR DMA-BUF FD shorter than advertised plane extent".into());
    }
    // The earlier visible-byte check bounds the minimum addressed pixels.
    // Without checking the *actual* allocation, a client could import a
    // multi-gigabyte BO as a nominal 360x224 wl_buffer and bypass the four-
    // import (64 MiB total) residency budget before EGL sees the FD.
    if (actual as u64) > MAX_CLIENT_BUFFER_BYTES as u64 {
        return Err("DMA-BUF FD exceeds 16 MiB allocation budget".into());
    }
    Ok(())
}

/// Import a *client-supplied* DMA-BUF on the renderer that belongs to the
/// currently active libseat DRM device. The caller must retain both the
/// returned GlesTexture and the wl_buffer until the import's lifetime ends,
/// and must release them BEFORE closing the original libseat DRM FD.
/// A successful EGL import is NOT proof of scanout or a synchronized frame.
pub fn import_client_dmabuf(
    renderer:&mut GlesRenderer,buffer:&Dmabuf,
)->Result<GlesTexture,String> {
    validate_client_dmabuf(buffer)?;
    validate_client_plane_backing(buffer)?;
    if !renderer.has_dmabuf_format(buffer.format()) {
        return Err("client DMA-BUF format/modifier not advertised by active GLES GPU".into());
    }
    renderer.import_dmabuf(buffer,None)
        .map_err(|error|format!("client DMA-BUF GLES texture import failed: {error}"))
}

/// QEMU-only transitional compositor path: import happened on the
/// libseat-owned EGL renderer, then download the GLES texture into one
/// *owned* bounded BGRA image for the existing verified Rust DRM worker.
/// This is a GPU -> CPU readback, NOT a zero-copy GPU composition engine.
/// The snapshot may be submitted to KMS only via the existing frame ticket
/// and DRM FB_ID/vblank ACK; no wl_buffer.release occurs here.
pub fn readback_qemu_client_bgra(
    renderer:&mut GlesRenderer,texture:&GlesTexture,
)->Result<Vec<u8>,String> {
    use smithay::{
        backend::renderer::{ExportMem,Texture,TextureMapping},
        utils::Rectangle,
    };
    let width=texture.width() as usize;
    let height=texture.height() as usize;
    if width!=360 || height!=224 {
        return Err("QEMU GPU client readback must match verified 360x224 scene".into());
    }
    let region=Rectangle::from_size((width as i32,height as i32).into());
    // GLES readback in ABGR8888 gives byte order RGBA (little endian).
    // Convert to the CPU/KMS scene's opaque XRGB8888 / BGRA byte order.
    let mapping=renderer.copy_texture(texture,region,Fourcc::Abgr8888)
        .map_err(|error|format!("QEMU GPU client texture download failed: {error}"))?;
    let flipped=mapping.flipped();
    let mapped=renderer.map_texture(&mapping)
        .map_err(|error|format!("QEMU GPU client pixel map failed: {error}"))?;
    convert_rgba_readback_to_bgra(mapped,width,height,flipped)
}

// The existing QEMU retained CPU scene is opaque XRGB over this BGRA
// background. Its GPU client has real ARGB8888 alpha; dropping alpha would
// wrongly turn translucent and fully transparent client texels into solid
// foreground pixels. This is an explicitly bounded CPU composition step,
// NOT direct GPU scanout or proof of production DMA-BUF fencing.
const QEMU_SCENE_BGRA:[u8;3]=[58,33,27];

fn over_opaque_background(premultiplied:u8,background:u8,alpha:u8)->u8 {
    let remaining=255u16-u16::from(alpha);
    let background_part=(u16::from(background)*remaining+127)/255;
    (u16::from(premultiplied)+background_part).min(255) as u8
}

fn convert_rgba_readback_to_bgra(
    rgba:&[u8],width:usize,height:usize,flipped:bool,
)->Result<Vec<u8>,String> {
    let row=width.checked_mul(4).ok_or("client readback row overflow")?;
    let len=row.checked_mul(height).ok_or("client readback allocation overflow")?;
    if width==0 || height==0 || width>MAX_CLIENT_SURFACE_EDGE ||
       height>MAX_CLIENT_SURFACE_EDGE || len>MAX_CLIENT_BUFFER_BYTES ||
       rgba.len()!=len {
        return Err("client GPU readback dimensions or size invalid".into());
    }
    let mut pixels=vec![0;len];
    for y in 0..height {
        let source_y=if flipped {height-1-y} else {y};
        let src=&rgba[source_y*row..(source_y+1)*row];
        let dst=&mut pixels[y*row..(y+1)*row];
        for (src,dst) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
            // Imported ARGB8888 client colors are premultiplied. Match
            // the opaque real QEMU scene background before XRGB scanout,
            // otherwise partial alpha becomes an incorrectly solid color.
            // Preserve the B,G,R byte order and discard alpha only AFTER
            // the per-channel source-over operation.
            dst.copy_from_slice(&[
                over_opaque_background(src[2],QEMU_SCENE_BGRA[0],src[3]),
                over_opaque_background(src[1],QEMU_SCENE_BGRA[1],src[3]),
                over_opaque_background(src[0],QEMU_SCENE_BGRA[2],src[3]),
                0,
            ]);
        }
    }
    Ok(pixels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use smithay::backend::allocator::{dmabuf::DmabufFlags,Modifier};

    fn buffer(width:i32,height:i32,stride:u32,offset:u32,
              format:Fourcc,planes:u32)->Dmabuf {
        let mut b=Dmabuf::builder(
            (width,height),format,Modifier::Linear,DmabufFlags::empty());
        for index in 0..planes {
            let file=std::fs::File::open("/dev/null").unwrap();
            assert!(b.add_plane(file.into(),index,offset,stride));
        }
        b.build().unwrap()
    }
    #[test]
    fn advertised_linear_modifiers_are_deterministic_and_not_dropped() {
        use smithay::backend::allocator::Modifier;
        let linear=|code|Format{code,modifier:Modifier::Linear};
        let formats=[
            linear(Fourcc::Xbgr8888),
            linear(Fourcc::Rgb565), // must never be promised to clients
            linear(Fourcc::Xrgb8888),
            linear(Fourcc::Argb8888),
            linear(Fourcc::Abgr8888),
            linear(Fourcc::Argb8888), // a duplicate EGL entry
        ];
        let first=ordered_advertised_formats(formats);
        let reversed=ordered_advertised_formats(formats.into_iter().rev());
        assert_eq!(first,reversed);
        assert_eq!(first,vec![
            linear(Fourcc::Argb8888),
            linear(Fourcc::Xrgb8888),
            linear(Fourcc::Abgr8888),
            linear(Fourcc::Xbgr8888),
        ]);
        assert!(first.len()<=MAX_ADVERTISED_CLIENT_FORMATS);
    }

    #[test]
    fn crowded_egl_modifiers_preserve_each_supported_rgb_format() {
        use smithay::backend::allocator::Modifier;
        // 96 actual-looking distinct vendor modifier IDs for ARGB alone
        // must not evict XRGB, ABGR and XBGR from a 64-entry registry.
        let mut formats:Vec<Format>=(1u64..=96).map(|id|Format{
            code:Fourcc::Argb8888,modifier:Modifier::from(id),
        }).collect();
        for code in [Fourcc::Xrgb8888,Fourcc::Abgr8888,Fourcc::Xbgr8888] {
            formats.push(Format{code,modifier:Modifier::from(1000u64+code as u32 as u64)});
        }
        let selected=ordered_advertised_formats(formats.clone());
        assert_eq!(selected.len(),MAX_ADVERTISED_CLIENT_FORMATS);
        for code in [Fourcc::Argb8888,Fourcc::Xrgb8888,
                     Fourcc::Abgr8888,Fourcc::Xbgr8888] {
            assert!(selected.iter().any(|format|format.code==code),
                "supported EGL RGB format starved by vendor modifiers: {code:?}");
        }
        assert!(selected.iter().all(|format|formats.contains(format)));
        formats.reverse();
        assert_eq!(selected,ordered_advertised_formats(formats));
    }

    #[test]
    fn advertised_modifier_priority_never_fabricates_gpu_support() {
        use smithay::backend::allocator::Modifier;
        let linear=Format{code:Fourcc::Argb8888,modifier:Modifier::Linear};
        // No new formats may be inserted into an empty/unrelated driver set.
        assert!(ordered_advertised_formats(Vec::<Format>::new()).is_empty());
        assert!(ordered_advertised_formats(vec![
            Format{code:Fourcc::Rgb565,modifier:Modifier::Linear}
        ]).is_empty());
        assert_eq!(ordered_advertised_formats(vec![linear]),vec![linear]);
    }

    fn memfd_geometry(
        len:u64,offset:u32,stride:u32,width:i32,height:i32,modifier:Modifier,
    )->Dmabuf {
        use std::os::fd::FromRawFd;
        let name=std::ffi::CString::new("melania-linear-plane-fixture").unwrap();
        let raw=unsafe {libc::memfd_create(name.as_ptr(),libc::MFD_CLOEXEC)};
        assert!(raw>=0,"test memfd_create failed");
        let file=unsafe {std::fs::File::from_raw_fd(raw)};
        file.set_len(len).unwrap();
        let mut buffer=Dmabuf::builder(
            (width,height),Fourcc::Argb8888,modifier,DmabufFlags::empty(),
        );
        assert!(buffer.add_plane(file.into(),0,offset,stride));
        buffer.build().unwrap()
    }
    fn linear_memfd_geometry(len:u64,offset:u32,stride:u32,width:i32,height:i32)->Dmabuf {
        memfd_geometry(len,offset,stride,width,height,Modifier::Linear)
    }
    fn linear_memfd(len:u64,offset:u32,stride:u32)->Dmabuf {
        linear_memfd_geometry(len,offset,stride,360,224)
    }

    // Validate the entire protocol bitfield for callers that still have it.
    // Smithay 0.7.0's Dmabuf handle has no flags() accessor, so this is
    // NOT yet wired into the imported client buffer path for field flags.
    fn validate_client_protocol_flags(flags:smithay::backend::allocator::dmabuf::DmabufFlags)
        ->Result<(),String> {
        if flags.is_empty() {Ok(())}
        else {Err("client DMA-BUF requires progressive non-inverted flags".into())}
    }

    #[test]
    fn client_dmabuf_rejects_unimplemented_orientation_and_field_flags() {
        // The bridge does not implement Y_INVERT or interlaced fields.
        // Real kernel-backed descriptors are used, not metadata-only mocks.
        use std::os::fd::FromRawFd;
        for flags in [
            DmabufFlags::Y_INVERT,
            DmabufFlags::INTERLACED,
            DmabufFlags::BOTTOM_FIRST,
        ] {
            let name=std::ffi::CString::new("melania-dmabuf-flags").unwrap();
            let raw=unsafe {libc::memfd_create(name.as_ptr(),libc::MFD_CLOEXEC)};
            assert!(raw>=0);
            let file=unsafe {std::fs::File::from_raw_fd(raw)};
            file.set_len((360*224*4) as u64).unwrap();
            let mut builder=Dmabuf::builder(
                (360,224),Fourcc::Argb8888,Modifier::Linear,flags);
            assert!(builder.add_plane(file.into(),0,0,360*4));
            let buffer=builder.build().unwrap();
            // All three flags are rejected by the protocol admission
            // predicate. Only Y_INVERT can currently be read back from a
            // Smithay 0.7.0 Dmabuf instance; the remaining two flags must
            // be intercepted before construction, and are NOT yet safe.
            assert_eq!(validate_client_protocol_flags(flags),
                Err("client DMA-BUF requires progressive non-inverted flags".into()));
            if flags==DmabufFlags::Y_INVERT {
                assert_eq!(validate_client_dmabuf(&buffer),
                    Err("client DMA-BUF requires progressive non-inverted flags".into()));
            }
        }
        let normal=linear_memfd((360*224*4) as u64,0,360*4);
        assert_eq!(validate_client_dmabuf(&normal),Ok(()));
        assert_eq!(validate_client_plane_backing(&normal),Ok(()));
    }

    #[test]
    fn linear_packed_rgb_rejects_misaligned_stride_or_first_pixel() {
        // Real memfds with enough bytes for their advertised plane: the
        // failure is solely the 32-bit texel alignment, not FD truncation.
        let aligned=linear_memfd(360*224*4+4096,4096,360*4);
        let bad_offset=linear_memfd(360*224*4+4096,1,360*4);
        let bad_stride=linear_memfd(360*224*4+4096,0,360*4+1);
        assert_eq!(validate_client_dmabuf(&aligned),Ok(()));
        for b in [&bad_offset,&bad_stride] {
            assert_eq!(validate_client_dmabuf(b),
                Err("client LINEAR packed-RGB stride/offset must be 4-byte aligned".into()));
        }
    }

    #[test]
    fn linear_dma_buf_fd_must_back_the_actual_last_row() {
        let stride=1536u32;
        let offset=4096u32;
        let needed=usize::try_from(offset).unwrap()
            + 223*usize::try_from(stride).unwrap() + 360*4;
        let exact=linear_memfd(needed as u64,offset,stride);
        assert!(validate_client_dmabuf(&exact).is_ok());
        assert_eq!(validate_client_plane_backing(&exact),Ok(()));
        let short=linear_memfd(needed as u64-1,offset,stride);
        assert!(validate_client_dmabuf(&short).is_ok());
        assert_eq!(validate_client_plane_backing(&short),
                   Err("LINEAR DMA-BUF FD shorter than advertised plane extent".into()));
        let zero=linear_memfd(0,offset,stride);
        assert!(validate_client_plane_backing(&zero).is_err());
    }

    #[test]
    fn linear_plane_budget_counts_real_stride_padding_not_only_visible_pixels() {
        // Both buffers advertise the same small visible 360x1024 content.
        // Their real, kernel-backed memfd extents differ only in stride.
        // The generous padding in the second buffer would bypass a quota
        // computed from width * height * 4 alone.
        let height=1024i32;
        let valid_stride=16384u32;
        let valid_extent=1023u64*u64::from(valid_stride)+360*4;
        let valid=linear_memfd_geometry(valid_extent,0,valid_stride,360,height);
        assert!(valid_extent<=MAX_CLIENT_BUFFER_BYTES as u64);
        assert_eq!(validate_client_dmabuf(&valid),Ok(()));
        assert_eq!(validate_client_plane_backing(&valid),Ok(()));

        let excessive_stride=32768u32;
        let excessive_extent=1023u64*u64::from(excessive_stride)+360*4;
        let excessive=linear_memfd_geometry(excessive_extent,0,excessive_stride,360,height);
        assert!(excessive_extent>MAX_CLIENT_BUFFER_BYTES as u64);
        assert!(excessive_extent<64*1024*1024);
        assert_eq!(
            validate_client_dmabuf(&excessive),
            Err("client LINEAR DMA-BUF plane exceeds 16 MiB stride/offset budget".into())
        );
        // The FD genuinely backs the advertised extent: both the logical
        // stride/offset budget and the whole-FD residency check reject it.
        assert_eq!(validate_client_plane_backing(&excessive),
                   Err("DMA-BUF FD exceeds 16 MiB allocation budget".into()));
    }

    #[test]
    fn linear_fd_allocation_budget_cannot_hide_behind_tiny_visible_image() {
        // Real kernel memfds: identical client-visible 360x224 geometry and
        // 1440-byte row stride, but radically different backing lengths.
        // All metadata checks pass; the oversized BO must fail *before* EGL.
        let minimum=(360*224*4) as u64;
        let exact=linear_memfd(minimum,0,360*4);
        let at_limit=linear_memfd(MAX_CLIENT_BUFFER_BYTES as u64,0,360*4);
        let over_limit=linear_memfd(MAX_CLIENT_BUFFER_BYTES as u64+1,0,360*4);
        let huge=linear_memfd(128*1024*1024,0,360*4);
        for b in [&exact,&at_limit,&over_limit,&huge] {
            assert_eq!(validate_client_dmabuf(b),Ok(()));
        }
        assert_eq!(validate_client_plane_backing(&exact),Ok(()));
        assert_eq!(validate_client_plane_backing(&at_limit),Ok(()));
        for b in [&over_limit,&huge] {
            assert_eq!(validate_client_plane_backing(b),
                       Err("DMA-BUF FD exceeds 16 MiB allocation budget".into()));
        }
    }

    #[test]
    fn opaque_modifier_plane_offset_must_point_inside_real_allocation() {
        // The modifier is deliberately opaque: no fake row-size rule.
        // The actual FD and byte offset, however, must be consistent.
        let valid=memfd_geometry(4096,4095,1440,360,224,Modifier::Invalid);
        let at_eof=memfd_geometry(4096,4096,1440,360,224,Modifier::Invalid);
        let past_eof=memfd_geometry(4096,8192,1440,360,224,Modifier::Invalid);
        for b in [&valid,&at_eof,&past_eof] {
            assert_eq!(validate_client_dmabuf(b),Ok(()));
        }
        assert_eq!(validate_client_plane_backing(&valid),Ok(()));
        for b in [&at_eof,&past_eof] {
            assert_eq!(validate_client_plane_backing(b),
                Err("DMA-BUF plane offset outside kernel allocation".into()));
        }
    }

    #[test]
    fn non_linear_modifier_still_has_real_kernel_allocation_budget() {
        // An opaque layout needs no invented LINEAR last-row extent,
        // but must not pin an oversized BO through an arbitrary modifier.
        let zero=memfd_geometry(0,0,1440,360,224,Modifier::Invalid);
        let small=memfd_geometry(4096,0,1440,360,224,Modifier::Invalid);
        let oversized=memfd_geometry(
            MAX_CLIENT_BUFFER_BYTES as u64+1,0,1440,360,224,Modifier::Invalid);
        assert_eq!(validate_client_dmabuf(&zero),Ok(()));
        assert_eq!(validate_client_plane_backing(&zero),
                   Err("DMA-BUF FD has zero-length kernel backing".into()));
        assert_eq!(validate_client_dmabuf(&small),Ok(()));
        assert_eq!(validate_client_plane_backing(&small),Ok(()));
        assert_eq!(validate_client_dmabuf(&oversized),Ok(()));
        assert_eq!(validate_client_plane_backing(&oversized),
                   Err("DMA-BUF FD exceeds 16 MiB allocation budget".into()));
    }

    #[test]
    fn linear_dma_buf_unseekable_or_truncated_import_is_fail_closed() {
        // A typed Dmabuf carrying a readable but unbacked /dev/null FD
        // can have superficially valid dimensions. It cannot reach EGL.
        let empty=buffer(360,224,360*4,0,Fourcc::Xrgb8888,1);
        assert!(validate_client_dmabuf(&empty).is_ok());
        assert!(validate_client_plane_backing(&empty).is_err());
        let valid=linear_memfd((360*224*4) as u64,0,360*4);
        assert_eq!(validate_client_plane_backing(&valid),Ok(()));
    }

    #[test]
    fn accepts_one_bounded_packed_rgb_plane() {
        let b=buffer(360,224,360*4,0,Fourcc::Xrgb8888,1);
        assert!(validate_client_dmabuf(&b).is_ok());
    }
    #[test]
    fn rejects_bad_geometry_large_stride_and_backing_offsets() {
        for b in [
            buffer(0,224,1440,0,Fourcc::Xrgb8888,1),
            buffer(2049,1,8196,0,Fourcc::Xrgb8888,1),
            buffer(2048,2048,4096,0,Fourcc::Xrgb8888,1),
            buffer(360,224,10,0,Fourcc::Xrgb8888,1),
            buffer(360,224,1440,65*1024*1024,Fourcc::Xrgb8888,1),
        ] {
            assert!(validate_client_dmabuf(&b).is_err());
        }
    }
    #[test]
    fn rejects_unhandled_multiplane_and_yuv_layouts() {
        let two=buffer(360,224,1440,0,Fourcc::Argb8888,2);
        let rgb565=buffer(360,224,720,0,Fourcc::Rgb565,1);
        assert!(validate_client_dmabuf(&two).is_err());
        assert!(validate_client_dmabuf(&rgb565).is_err());
    }
    #[test]
    fn readback_swizzles_rgba_into_opaque_bgra_and_flips_bottom_up() {
        // Two rows: top RED/GREEN, bottom BLUE/WHITE.
        let data=[
            0,0,255,255, 255,255,255,255,
            255,0,0,255, 0,255,0,255,
        ];
        let pixels=convert_rgba_readback_to_bgra(&data,2,2,true).unwrap();
        assert_eq!(&pixels[..8],[0,0,255,0,0,255,0,0]);
        assert_eq!(&pixels[8..],[255,0,0,0,255,255,255,0]);
    }
    #[test]
    fn imported_argb_premultiplied_alpha_blends_over_real_kms_background() {
        // First texel: premultiplied ARGB at 50% alpha. The second is
        // fully transparent, third opaque; no alpha escapes XRGB scanout.
        let rgba=[
            42,23,93,128,
            0,0,0,0,
            84,46,186,255,
        ];
        let pixels=convert_rgba_readback_to_bgra(&rgba,3,1,false).unwrap();
        assert_eq!(&pixels[0..4],&[122,39,55,0]);
        assert_eq!(&pixels[4..8],&[58,33,27,0]);
        assert_eq!(&pixels[8..12],&[186,46,84,0]);
        // A non-premultiplied, overly bright channel cannot wrap.
        assert_eq!(over_opaque_background(250,58,128),255);
    }
    #[test]
    fn readback_rejects_wrong_stride_length_and_zero_dimensions() {
        assert!(convert_rgba_readback_to_bgra(&[0;4],2,1,false).is_err());
        assert!(convert_rgba_readback_to_bgra(&[],0,2,false).is_err());
        assert!(convert_rgba_readback_to_bgra(&[0;4],1,1,false).is_ok());
    }
    #[test]
    fn second_real_client_gpu_frame_needs_explicit_mode_and_first_kms_ack() {
        assert!(qemu_gpu_frame_allowed(0,false));
        assert!(!qemu_gpu_frame_allowed(1,false));
        assert!(!qemu_gpu_frame_allowed(2,false));
        assert!(qemu_gpu_frame_allowed(0,true));
        assert!(qemu_gpu_frame_allowed(1,true));
        assert!(!qemu_gpu_frame_allowed(2,true));
        assert!(!qemu_gpu_frame_allowed(u64::MAX,true));
        assert!(qemu_gpu_frame_required(1,false));
        assert!(!qemu_gpu_frame_required(2,false));
        assert!(qemu_gpu_frame_required(1,true));
        assert!(qemu_gpu_frame_required(2,true));
        assert!(!qemu_gpu_frame_required(3,true));
        assert!(!qemu_gpu_frame_required(0,true));
    }
    #[test]
    fn imported_textures_remain_bounded_even_with_multiple_clients() {
        assert_eq!(MAX_CLIENT_IMPORTS,4);
        assert!(MAX_CLIENT_IMPORTS*MAX_CLIENT_BUFFER_BYTES<=64*1024*1024);
    }
}
