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

pub fn client_import_formats(renderer:&GlesRenderer)->Vec<Format> {
    renderer.dmabuf_formats().iter().copied()
        .filter(|format|supported_format(format.code))
        .take(64).collect()
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
    let stride=buffer.strides().next()
        .ok_or("client DMA-BUF has no plane stride")? as usize;
    let offset=buffer.offsets().next()
        .ok_or("client DMA-BUF has no plane offset")? as usize;
    if stride<row || stride>MAX_CLIENT_SURFACE_EDGE*16 {
        return Err("client DMA-BUF stride outside bounded packed-RGB layout".into());
    }
    let last=height.checked_sub(1)
        .and_then(|rows|rows.checked_mul(stride))
        .and_then(|length|length.checked_add(offset))
        .and_then(|length|length.checked_add(row))
        .ok_or("client DMA-BUF plane address overflow")?;
    if last>64*1024*1024 {
        return Err("client DMA-BUF unbounded plane offset or backing extent".into());
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
