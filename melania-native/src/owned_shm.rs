//! Owned, dimension-aware Wayland wl_shm pixel importer for a future
//! general-purpose native compositor.
//!
//! The current QEMU DRM presentation path still uses 360x224 client windows.
//! This import primitive accepts bounded arbitrary dimensions and padded rows,
//! but does NOT change KMS modes, establish native login or advertise output.
//! No borrowed client pointer or pool mapping escapes this function.
use smithay::{
    reexports::wayland_server::protocol::wl_shm,
    wayland::shm::BufferData,
};

pub const MAX_CLIENT_DIMENSION: usize = 8192;
pub const MAX_CLIENT_COPY_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaType {
    Opaque,
    // wl_shm ARGB8888 client pixels use premultiplied alpha, NOT straight.
    Premultiplied,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedClientPixels {
    pub width: usize,
    pub height: usize,
    pub alpha: AlphaType,
    /// Tightly packed BGRA, 4 bytes/pixel; padding and pool offset removed.
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShmImportError {
    UnsupportedFormat,
    InvalidGeometry,
    InvalidStride,
    InvalidPoolRange,
    TooLarge,
}

/// The caller must hold the Smithay wl_shm access callback's live mapping.
/// Perform all bounds checks before the first allocation or pointer access.
/// The output owns ALL copied pixels, so Wayland buffers may be released
/// according to presentation lifetime rather than source mmap lifetime.
pub fn copy_bounded_client_shm(
    ptr: *const u8, pool_len: usize, meta: BufferData
) -> Result<OwnedClientPixels, ShmImportError> {
    let alpha=match meta.format {
        wl_shm::Format::Xrgb8888=>AlphaType::Opaque,
        wl_shm::Format::Argb8888=>AlphaType::Premultiplied,
        _=>return Err(ShmImportError::UnsupportedFormat),
    };
    let width=usize::try_from(meta.width)
        .map_err(|_|ShmImportError::InvalidGeometry)?;
    let height=usize::try_from(meta.height)
        .map_err(|_|ShmImportError::InvalidGeometry)?;
    if width==0 || height==0 ||
       width>MAX_CLIENT_DIMENSION || height>MAX_CLIENT_DIMENSION {
        return Err(ShmImportError::InvalidGeometry);
    }
    let row=width.checked_mul(4).ok_or(ShmImportError::TooLarge)?;
    let size=row.checked_mul(height).ok_or(ShmImportError::TooLarge)?;
    if size>MAX_CLIENT_COPY_BYTES { return Err(ShmImportError::TooLarge); }
    let stride=usize::try_from(meta.stride)
        .map_err(|_|ShmImportError::InvalidStride)?;
    if stride<row || stride>MAX_CLIENT_DIMENSION*16 {
        return Err(ShmImportError::InvalidStride);
    }
    let start=usize::try_from(meta.offset)
        .map_err(|_|ShmImportError::InvalidPoolRange)?;
    let end=start.checked_add(
        (height-1).checked_mul(stride).ok_or(ShmImportError::InvalidPoolRange)?
    ).and_then(|pos|pos.checked_add(row))
        .ok_or(ShmImportError::InvalidPoolRange)?;
    if ptr.is_null() || end>pool_len {
        return Err(ShmImportError::InvalidPoolRange);
    }
    let mut data=vec![0u8;size];
    for y in 0..height {
        let source=start+y*stride;
        let dest=y*row;
        for x in 0..width {
            let input=source+x*4;
            let output=dest+x*4;
            // SAFETY: the mapped pool remains accessible for the duration
            // of the caller's Smithay callback. end<=pool_len, source-row
            // calculations are checked above, and only byte reads occur.
            for component in 0..3 {
                data[output+component]=unsafe {
                    std::ptr::read_volatile(ptr.add(input+component))
                };
            }
            data[output+3]=if alpha==AlphaType::Opaque {0} else {
                unsafe {std::ptr::read_volatile(ptr.add(input+3))}
            };
        }
    }
    Ok(OwnedClientPixels {width,height,alpha,data})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(width:i32,height:i32,stride:i32,offset:i32,
            format:wl_shm::Format)->BufferData {
        BufferData {width,height,stride,offset,format}
    }

    #[test]
    fn copies_variable_dimensions_and_padded_rows_without_aliasing_pool() {
        let bytes=[
            99,99,99,99,       // source offset
            4,5,6,123, 7,8,9,222, 55,55,55,55, // row0 + padding
            10,11,12,255, 13,14,15,255,        // row1
        ];
        let f=copy_bounded_client_shm(
            bytes.as_ptr(),bytes.len(),meta(2,2,12,4,wl_shm::Format::Xrgb8888)
        ).unwrap();
        assert_eq!((f.width,f.height,f.alpha),(2,2,AlphaType::Opaque));
        assert_eq!(f.data,vec![4,5,6,0,7,8,9,0,10,11,12,0,13,14,15,0]);
        assert_eq!(bytes[7],123); // never change client memory
    }

    #[test]
    fn preserves_premultiplied_alpha_without_unpremultiplying() {
        let bytes=[30,20,10,128, 60,50,40,255];
        let f=copy_bounded_client_shm(
            bytes.as_ptr(),bytes.len(),meta(2,1,8,0,wl_shm::Format::Argb8888)
        ).unwrap();
        assert_eq!(f.alpha,AlphaType::Premultiplied);
        assert_eq!(f.data,bytes);
    }

    #[test]
    fn fails_closed_on_invalid_stride_offset_or_truncated_mapping() {
        let bytes=[0u8;32];
        let call=|w,h,stride,offset,available|{
            copy_bounded_client_shm(
                bytes.as_ptr(),available,meta(w,h,stride,offset,wl_shm::Format::Xrgb8888)
            )
        };
        assert_eq!(call(2,2,7,0,32),Err(ShmImportError::InvalidStride));
        assert_eq!(call(2,2,-1,0,32),Err(ShmImportError::InvalidStride));
        assert_eq!(call(2,2,8,-1,32),Err(ShmImportError::InvalidPoolRange));
        assert_eq!(call(2,2,8,0,15),Err(ShmImportError::InvalidPoolRange));
        assert_eq!(call(0,2,8,0,32),Err(ShmImportError::InvalidGeometry));
        assert_eq!(call(-1,2,8,0,32),Err(ShmImportError::InvalidGeometry));
        assert_eq!(call(8193,1,32772,0,32),Err(ShmImportError::InvalidGeometry));
        assert_eq!(
            copy_bounded_client_shm(std::ptr::null(),32,
                meta(2,2,8,0,wl_shm::Format::Xrgb8888)),
            Err(ShmImportError::InvalidPoolRange)
        );
    }

    #[test]
    fn rejects_oversized_and_unsupported_buffers_without_allocation() {
        let byte=0u8;
        assert_eq!(
            copy_bounded_client_shm(&byte,1,
                meta(8192,8192,32768,0,wl_shm::Format::Xrgb8888)),
            Err(ShmImportError::TooLarge)
        );
        assert_eq!(
            copy_bounded_client_shm(&byte,1,
                meta(1,1,4,0,wl_shm::Format::Rgb565)),
            Err(ShmImportError::UnsupportedFormat)
        );
    }
}
