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
    fn imported_textures_remain_bounded_even_with_multiple_clients() {
        assert_eq!(MAX_CLIENT_IMPORTS,4);
        assert!(MAX_CLIENT_IMPORTS*MAX_CLIENT_BUFFER_BYTES<=64*1024*1024);
    }
}
