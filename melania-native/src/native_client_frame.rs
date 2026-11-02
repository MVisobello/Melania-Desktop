//! Stage 26: one real wl_shm client surface composited by Rust in an isolated
//! QEMU test guest. This produces pixels; it does NOT own the KMS pageflip.
//! The QEMU-only DRM diagnostic later scans out this exact immutable snapshot.
use std::{fs,io::Write, path::Path};
use melania_native::scene::{self,Layer,Pixels,Surface};
use smithay::{reexports::wayland_server::protocol::wl_shm,
              wayland::shm::BufferData};

pub const OUT_W:usize=1280;
pub const OUT_H:usize=800;
pub const WIN_W:usize=360;
pub const WIN_H:usize=224;
const LEFT:usize=160;
const TOP:usize=120;
pub const FRAME_PATH:&str="/run/melania-native-qemu/client-frame.bgra";

/// Extract exactly one validated wl_shm snapshot into owned XRGB pixels.
/// The pointer is never saved past the Smithay buffer access callback.
pub fn copy_client_shm(ptr:*const u8,len:usize,meta:BufferData)
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
    // Copy untrusted wl_shm bytes into owned memory while Smithay holds the
    // mapping. Never retain a borrowed client pointer across a redraw.
    let mut client=vec![0u8;WIN_W*WIN_H*4];
    for y in 0..WIN_H {
        let in_row=start+y*stride;
        let out_row=y*WIN_W*4;
        for x in 0..WIN_W {
            let p=in_row+x*4;
            let q=out_row+x*4;
            // SAFETY: all reads are inside the validated wl_shm range, and
            // the mapping remains valid for the entire callback. Volatile
            // reads avoid assuming the mutable client buffer is immutable.
            for c in 0..3 {
                client[q+c]=unsafe {std::ptr::read_volatile(ptr.add(p+c))};
            }
            client[q+3]=0;
        }
    }
    Ok(client)
}

/// VM-only dimension-aware copy for *genuine* resized client wl_shm.
/// This does not guess client image dimensions or retain the mapped pointer.
pub fn copy_client_shm_sized(ptr:*const u8,len:usize,meta:BufferData)
    ->Result<Vec<u8>,String>
{
    if meta.format!=wl_shm::Format::Xrgb8888 ||
       !(96..=600).contains(&meta.width) ||
       !(72..=420).contains(&meta.height) ||
       meta.offset<0 {
        return Err("invalid resized client buffer format or dimensions".into());
    }
    let width=meta.width as usize;
    let height=meta.height as usize;
    let stride=usize::try_from(meta.stride)
        .map_err(|_|"negative wl_shm stride")?;
    if stride<width*4 || stride>width*8 || ptr.is_null() {
        return Err("invalid resized client buffer stride or mapping".into());
    }
    let start=meta.offset as usize;
    let last=start.checked_add((height-1)*stride)
        .and_then(|pos|pos.checked_add(width*4))
        .ok_or("resized wl_shm offset overflow")?;
    if last>len {return Err("resized wl_shm outside verified mapping".into());}
    let mut owned=vec![0u8;width*height*4];
    for y in 0..height {
        for x in 0..width {
            let source=start+y*stride+x*4;
            let target=(y*width+x)*4;
            // SAFETY: the complete last row has been checked against
            // Smithay's mapped wl_shm length; the callback owns the borrow.
            for channel in 0..3 {
                owned[target+channel]=unsafe {
                    std::ptr::read_volatile(ptr.add(source+channel))
                };
            }
        }
    }
    Ok(owned)
}

pub fn compose_from_shm(ptr:*const u8,len:usize,meta:BufferData)
    ->Result<Vec<u8>,String>
{
    let client=copy_client_shm(ptr,len,meta)?;
    // The disposable VM now exercises the same position-aware scene
    // composition primitive intended for multiple windows and shell layers.
    // Stage31 still has ONE real Wayland client, not a native desktop session.
    let surfaces=[
        Surface{
            x:(LEFT-6) as i32,y:(TOP-6) as i32,
            width:WIN_W+12,height:WIN_H+12,layer:Layer::Bottom,
            pixels:Pixels::Solid([216,171,65,0]),
        },
        Surface{
            x:LEFT as i32,y:TOP as i32,
            width:WIN_W,height:WIN_H,layer:Layer::Normal,
            pixels:Pixels::OpaqueBgra{data:&client,stride:WIN_W*4},
        },
    ];
    let frame=scene::compose(OUT_W,OUT_H,[58,33,27,0],&surfaces)
        .map_err(|err|format!("native scene composition failed: {err:?}"))?;
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
