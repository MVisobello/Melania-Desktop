//! On-demand window-only GPU readback for the local Melania taskbar.
//! Never sample the composed desktop: the framebuffer contains only the
//! requested Wayland surface tree and an opaque background.
use std::{error::Error, fs::OpenOptions, io::Write, path::Path};
use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            Bind, Color32F, ExportMem, Frame, Offscreen, Renderer,
            gles::{GlesRenderer, GlesRenderbuffer},
            element::{surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement}, Kind},
            utils::draw_render_elements,
        },
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Rectangle, Transform},
};

pub const THUMB_W: i32 = 256;
pub const THUMB_H: i32 = 160;

/// Valid BMPv3 BGRA header + bounded 32-bit pixels. No external image decoder
/// or shell commands; GTK supports this format as an ordinary local image.
fn encode_bmp(width: u32, height: u32, bgra: &[u8]) -> Result<Vec<u8>, &'static str> {
    let pixels = width.checked_mul(height)
        .and_then(|n| n.checked_mul(4)).ok_or("Dimensione BMP fuori limite")? as usize;
    if width == 0 || height == 0 || pixels != bgra.len() || pixels > 1024*1024 {
        return Err("Buffer immagine non valido");
    }
    let bytes = 54u32.checked_add(pixels as u32).ok_or("BMP troppo grande")?;
    let mut out = Vec::with_capacity(bytes as usize);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&bytes.to_le_bytes());
    out.extend_from_slice(&[0;4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(pixels as u32).to_le_bytes());
    out.extend_from_slice(&[0;16]);
    out.extend_from_slice(bgra);
    Ok(out)
}

pub fn capture_window(
    renderer: &mut GlesRenderer, surface: &WlSurface,
    source_width: i32, source_height: i32,
    destination: &Path,
) -> Result<(), Box<dyn Error>> {
    if source_width <= 0 || source_height <= 0 ||
        source_width > 16_384 || source_height > 16_384 {
        return Err("Geometria finestra non valida".into());
    }
    let buffer_size = (THUMB_W, THUMB_H).into();
    let physical_size = (THUMB_W, THUMB_H).into();
    let mut offscreen = <GlesRenderer as Offscreen<GlesRenderbuffer>>::create_buffer(
        renderer, Fourcc::Argb8888, buffer_size)?;
    let elements: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = render_elements_from_surface_tree(
        renderer, surface, (0,0), 1.0, 1.0, Kind::Unspecified,
    );
    let mut target = <GlesRenderer as Bind<GlesRenderbuffer>>::bind(renderer, &mut offscreen)?;
    let scale = f64::min(THUMB_W as f64/source_width as f64,
                         THUMB_H as f64/source_height as f64);
    let damage = Rectangle::from_size(physical_size);
    let read_region = Rectangle::from_size(buffer_size);
    {
        let mut frame = renderer.render(&mut target, physical_size, Transform::Normal)?;
        frame.clear(Color32F::new(0.06,0.13,0.22,1.0), &[damage])?;
        draw_render_elements(&mut frame, scale, &elements, &[damage])?;
        frame.finish()?;
    }
    let mapping = renderer.copy_framebuffer(&target,read_region,Fourcc::Argb8888)?;
    let pixels = renderer.map_texture(&mapping)?;
    let bmp = encode_bmp(THUMB_W as u32,THUMB_H as u32,pixels)
        .map_err(|e|std::io::Error::new(std::io::ErrorKind::InvalidData,e))?;
    // The caller chooses a freshly unique filename inside a private 0700
    // directory, with 0600 mode for the image. Never follow an existing file.
    let mut file = OpenOptions::new().write(true).create_new(true).open(destination)?;
    file.write_all(&bmp)?;
    file.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn encoded_bmp_is_bounded_and_has_valid_header() {
        let bytes = encode_bmp(2,2,&[0x11;16]).unwrap();
        assert_eq!(&bytes[..2],b"BM");
        assert_eq!(bytes.len(),70);
        assert_eq!(u32::from_le_bytes(bytes[2..6].try_into().unwrap()),70);
        assert_eq!(u32::from_le_bytes(bytes[10..14].try_into().unwrap()),54);
        assert_eq!(u16::from_le_bytes(bytes[28..30].try_into().unwrap()),32);
    }
    #[test] fn rejects_invalid_sizes_and_pixel_counts() {
        assert!(encode_bmp(0,2,&[]).is_err());
        assert!(encode_bmp(u32::MAX,5,&[]).is_err());
        assert!(encode_bmp(2,2,&[1;12]).is_err());
    }
}
