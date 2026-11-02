//! Stage 26: one real wl_shm client surface composited by Rust in an isolated
//! QEMU test guest. This produces pixels; it does NOT own the KMS pageflip.
//! The QEMU-only DRM diagnostic later scans out this exact immutable snapshot.
use std::{fs,io::Write, path::Path};
use smithay::{reexports::wayland_server::protocol::wl_shm,
              wayland::shm::BufferData};

pub const OUT_W:usize=1280;
pub const OUT_H:usize=800;
pub const WIN_W:usize=360;
pub const WIN_H:usize=224;
const LEFT:usize=160;
const TOP:usize=120;
pub const FRAME_PATH:&str="/run/melania-native-qemu/client-frame.bgra";

pub fn compose_from_shm(ptr:*const u8,len:usize,meta:BufferData)
    ->Result<Vec<u8>,String>
{
    if meta.format!=wl_shm::Format::Xrgb8888 {
        return Err("only XRGB8888 wl_shm buffers are accepted by the QEMU probe".into());
    }
    if meta.width!=WIN_W as i32 || meta.height!=WIN_H as i32 ||
       meta.stride<(WIN_W*4) as i32 || meta.stride>=(WIN_W*8) as i32 ||
       meta.offset<0
    {
        return Err("unexpected client surface dimensions, stride or offset".into());
    }
    let start=meta.offset as usize;
    let stride=meta.stride as usize;
    let row_size=WIN_W*4;
    let last=start.checked_add((WIN_H-1)*stride)
        .and_then(|p|p.checked_add(row_size))
        .ok_or("client surface address overflow")?;
    if last>len || ptr.is_null() {
        return Err("wl_shm pool does not contain requested client pixels".into());
    }
    let mut frame=vec![0u8;OUT_W*OUT_H*4];
    // Background and frame are compositor-owned; window pixels below originate
    // exclusively in the client's wl_shm pool, never synthesized by the helper.
    for pixel in frame.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[58,33,27,0]); // RGB(27,33,58), dark desktop
    }
    for y in TOP-6..TOP+WIN_H+6 {
        for x in LEFT-6..LEFT+WIN_W+6 {
            let p=(y*OUT_W+x)*4;
            frame[p..p+4].copy_from_slice(&[216,171,65,0]); // border
        }
    }
    for y in 0..WIN_H {
        let in_row=start+y*stride;
        let out_row=((TOP+y)*OUT_W+LEFT)*4;
        for x in 0..WIN_W {
            let p=in_row+x*4;
            let q=out_row+x*4;
            // SAFETY: each individual pointer arithmetic index is bounded by
            // last<=len. Volatile byte loads do not construct shared references
            // into mutable client memory. The wl_shm mapping is valid for the
            // whole Smithay with_buffer_contents callback. A malicious client
            // could still change pixel values concurrently; this is only a
            // bounded test probe, not a production buffer import path.
            for c in 0..3 {
                frame[q+c]=unsafe {std::ptr::read_volatile(ptr.add(p+c))};
            }
            frame[q+3]=0;
        }
    }
    Ok(frame)
}

pub const FRAME2_PATH:&str="/run/melania-native-qemu/client-frame-2.bgra";

pub fn write_frame_index(frame:&[u8],index:u8)
    ->Result<(),Box<dyn std::error::Error>>
{
    if frame.len()!=OUT_W*OUT_H*4 {return Err("invalid composed frame size".into());}
    let name=match index {
        0=>FRAME_PATH,
        1=>FRAME2_PATH,
        _=>return Err("QEMU frame sequence exceeds two buffers".into()),
    };
    let file=Path::new(name);
    let temp=file.with_extension("bgra.part");
    {
        let mut out=fs::OpenOptions::new().create_new(true).write(true).open(&temp)?;
        out.write_all(frame)?;
        out.sync_all()?;
    }
    fs::rename(temp,file)?;
    Ok(())
}

pub fn write_frame(frame:&[u8])->Result<(),Box<dyn std::error::Error>> {
    write_frame_index(frame,0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copies_real_client_pixels_and_keeps_background() {
        let mut source=vec![0u8;WIN_W*WIN_H*4];
        for (index,p) in source.chunks_exact_mut(4).enumerate(){
            let x=index%WIN_W;
            let (r,g,b)=if x<WIN_W/2 {(245,108,34)} else {(16,205,155)};
            p.copy_from_slice(&[b,g,r,0]);
        }
        let meta=BufferData{offset:0,width:WIN_W as i32,height:WIN_H as i32,
            stride:(WIN_W*4) as i32,format:wl_shm::Format::Xrgb8888};
        let f=compose_from_shm(source.as_ptr(),source.len(),meta).unwrap();
        let pixel=|x:usize,y:usize| {
            let p=(y*OUT_W+x)*4;
            [f[p],f[p+1],f[p+2]]
        };
        assert_eq!(pixel(0,0),[58,33,27]);
        assert_eq!(pixel(LEFT-1,TOP),[216,171,65]);
        assert_eq!(pixel(LEFT+WIN_W/4,TOP+WIN_H/2),[34,108,245]);
        assert_eq!(pixel(LEFT+3*WIN_W/4,TOP+WIN_H/2),[155,205,16]);
    }
    #[test]
    fn rejects_truncated_or_mismatched_buffer() {
        let data=[0u8;100];
        let meta=BufferData{offset:0,width:WIN_W as i32,height:WIN_H as i32,
            stride:(WIN_W*4) as i32,format:wl_shm::Format::Xrgb8888};
        assert!(compose_from_shm(data.as_ptr(),data.len(),meta).is_err());
        assert!(compose_from_shm(data.as_ptr(),data.len(),
            BufferData{format:wl_shm::Format::Argb8888,..meta}).is_err());
    }
}
