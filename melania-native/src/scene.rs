//! CPU scene compositor for the isolated native/QEMU probe.
//!
//! The stage-31 renderer is deliberately deterministic and memory bounded.
//! It accepts multiple positioned surfaces with explicit layer ordering and
//! clips them to the output. It is NOT a GBM/EGL renderer, a KMS scheduler,
//! or authorization to use a physical display.
use std::convert::TryFrom;

const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layer {
    Background,
    Bottom,
    Normal,
    Top,
    Overlay,
}

pub enum Pixels<'a> {
    /// XRGB8888 (BGRA memory order); high byte is ignored.
    OpaqueBgra { data: &'a [u8], stride: usize },
    /// Unpremultiplied BGRA8 with alpha in the fourth byte.
    StraightAlphaBgra { data: &'a [u8], stride: usize },
    /// Wayland wl_shm ARGB8888, whose RGB channels are PREMULTIPLIED.
    /// Never blend these pixels as if the RGB channels were straight alpha.
    PremultipliedAlphaBgra { data: &'a [u8], stride: usize },
    Solid([u8; 4]),
}

pub struct Surface<'a> {
    pub x: i32,
    pub y: i32,
    pub width: usize,
    pub height: usize,
    pub layer: Layer,
    pub pixels: Pixels<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SceneError {
    InvalidDimensions,
    InvalidSurfaceBuffer,
}

fn verify(surface: &Surface<'_>) -> Result<(), SceneError> {
    if surface.width == 0 || surface.height == 0 {
        return Ok(());
    }
    // Coordinates are i32, but a malicious surface size may be usize::MAX.
    i64::try_from(surface.width).map_err(|_| SceneError::InvalidDimensions)?;
    i64::try_from(surface.height).map_err(|_| SceneError::InvalidDimensions)?;
    if let Pixels::OpaqueBgra { data, stride }
         | Pixels::StraightAlphaBgra { data, stride }
         | Pixels::PremultipliedAlphaBgra { data, stride } = &surface.pixels {
        let row = surface.width.checked_mul(4)
            .ok_or(SceneError::InvalidSurfaceBuffer)?;
        if *stride < row {
            return Err(SceneError::InvalidSurfaceBuffer);
        }
        let required = (surface.height - 1).checked_mul(*stride)
            .and_then(|n| n.checked_add(row))
            .ok_or(SceneError::InvalidSurfaceBuffer)?;
        if required > data.len() {
            return Err(SceneError::InvalidSurfaceBuffer);
        }
    }
    Ok(())
}

/// Compose a single output, in BGRA/XRGB memory order. Stable ordering within
/// a layer follows the supplied surface slice; higher layers are rendered last.
/// Validate every surface BEFORE producing output (also off-screen surfaces).
pub fn compose(width: usize, height: usize, background: [u8; 4],
               surfaces: &[Surface<'_>]) -> Result<Vec<u8>, SceneError> {
    let bytes = width.checked_mul(height).and_then(|p| p.checked_mul(4))
        .ok_or(SceneError::InvalidDimensions)?;
    if width == 0 || height == 0 || bytes > MAX_FRAME_BYTES {
        return Err(SceneError::InvalidDimensions);
    }
    for surface in surfaces { verify(surface)?; }

    let mut output = vec![0u8; bytes];
    for p in output.chunks_exact_mut(4) {
        p[..3].copy_from_slice(&background[..3]);
    }
    for layer in [Layer::Background, Layer::Bottom, Layer::Normal,
                  Layer::Top, Layer::Overlay] {
        for surface in surfaces.iter().filter(|s| s.layer == layer) {
            let left = i64::from(surface.x);
            let top = i64::from(surface.y);
            let right = left + surface.width as i64;
            let bottom = top + surface.height as i64;
            let sx = left.max(0).min(width as i64) as usize;
            let sy = top.max(0).min(height as i64) as usize;
            let ex = right.max(0).min(width as i64) as usize;
            let ey = bottom.max(0).min(height as i64) as usize;
            if sx >= ex || sy >= ey { continue; }
            for y in sy..ey {
                for x in sx..ex {
                    let p = (y * width + x) * 4;
                    let (color, alpha, premultiplied) = match &surface.pixels {
                        Pixels::Solid(color) => (*color, 255u8, false),
                        Pixels::OpaqueBgra { data, stride } => {
                            let src = (y as i64 - top) as usize * *stride
                                + (x as i64 - left) as usize * 4;
                            ([data[src], data[src + 1], data[src + 2], 0], 255, false)
                        }
                        Pixels::StraightAlphaBgra { data, stride } => {
                            let src = (y as i64 - top) as usize * *stride
                                + (x as i64 - left) as usize * 4;
                            ([data[src], data[src + 1], data[src + 2], 0],
                             data[src + 3], false)
                        }
                        Pixels::PremultipliedAlphaBgra { data, stride } => {
                            let src = (y as i64 - top) as usize * *stride
                                + (x as i64 - left) as usize * 4;
                            ([data[src], data[src + 1], data[src + 2], 0],
                             data[src + 3], true)
                        }
                    };
                    if alpha == 255 {
                        output[p..p + 3].copy_from_slice(&color[..3]);
                    } else if alpha > 0 {
                        for c in 0..3 {
                            let a = u32::from(alpha);
                            let background=u32::from(output[p+c]);
                            output[p+c]=if premultiplied {
                                // Untrusted client RGB can violate its claimed
                                // premultiplied alpha; clamp it to alpha.
                                (u32::from(color[c]).min(a)
                                  + (background*(255-a)+127)/255).min(255) as u8
                            } else {
                                ((u32::from(color[c])*a
                                   + background*(255-a)+127)/255) as u8
                            };
                        }
                    }
                }
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pixel(out: &[u8], w: usize, x: usize, y: usize) -> [u8; 3] {
        let i = (y * w + x) * 4;
        [out[i], out[i + 1], out[i + 2]]
    }

    #[test]
    fn honors_layers_and_stable_order() {
        let input = [
            Surface { x: 0, y: 0, width: 2, height: 2,
                layer: Layer::Normal, pixels: Pixels::Solid([10, 20, 30, 0]) },
            Surface { x: 0, y: 0, width: 2, height: 2,
                layer: Layer::Bottom, pixels: Pixels::Solid([80, 80, 80, 0]) },
            Surface { x: 1, y: 1, width: 1, height: 1,
                layer: Layer::Normal, pixels: Pixels::Solid([40, 50, 60, 0]) },
        ];
        let out = compose(3, 3, [1, 2, 3, 0], &input).unwrap();
        assert_eq!(pixel(&out, 3, 0, 0), [10, 20, 30]);
        assert_eq!(pixel(&out, 3, 1, 1), [40, 50, 60]);
        assert_eq!(pixel(&out, 3, 2, 2), [1, 2, 3]);
    }

    #[test]
    fn clips_negative_origin_and_right_bottom_edges() {
        let input = [Surface { x: -1, y: -1, width: 4, height: 4,
            layer: Layer::Normal, pixels: Pixels::Solid([8, 9, 10, 0]) }];
        let out = compose(2, 2, [0; 4], &input).unwrap();
        assert_eq!(pixel(&out, 2, 0, 0), [8, 9, 10]);
        assert_eq!(pixel(&out, 2, 1, 1), [8, 9, 10]);
    }

    #[test]
    fn reads_correct_rows_when_clipped_and_stride_padded() {
        let data = [1, 2, 3, 0, 9, 9, 9, 9,
                    4, 5, 6, 0, 8, 8, 8, 8];
        let input = [Surface { x: -1, y: 0, width: 1, height: 2,
            layer: Layer::Top,
            pixels: Pixels::OpaqueBgra { data: &data, stride: 8 } }];
        let out = compose(2, 2, [7, 7, 7, 0], &input).unwrap();
        assert_eq!(pixel(&out, 2, 0, 0), [7, 7, 7]);
        // Same source shown when moved on-screen.
        let input2 = [Surface { x: 1, y: 0, width: 1, height: 2,
            layer: Layer::Top,
            pixels: Pixels::OpaqueBgra { data: &data, stride: 8 } }];
        let out2 = compose(2, 2, [7, 7, 7, 0], &input2).unwrap();
        assert_eq!(pixel(&out2, 2, 1, 0), [1, 2, 3]);
        assert_eq!(pixel(&out2, 2, 1, 1), [4, 5, 6]);
    }

    #[test]
    fn blends_straight_alpha_without_changing_xrgb_high_byte() {
        let data = [200, 100, 0, 128];
        let input = [Surface { x: 0, y: 0, width: 1, height: 1,
            layer: Layer::Overlay,
            pixels: Pixels::StraightAlphaBgra { data: &data, stride: 4 } }];
        let out = compose(1, 1, [0, 0, 100, 255], &input).unwrap();
        assert_eq!(&out[..4], &[100, 50, 50, 0]);
    }

    #[test]
    fn blends_wayland_premultiplied_argb_without_double_alpha() {
        // Two semitransparent channels are already multiplied by 128/255.
        // The resulting pixel MUST be src_premul + dst*(1-alpha).
        let data=[20u8,30,40,128];
        let layers=[Surface {x:0,y:0,width:1,height:1,
            layer:Layer::Normal,
            pixels:Pixels::PremultipliedAlphaBgra {data:&data,stride:4}}];
        let output=compose(1,1,[10,20,30,0],&layers).unwrap();
        assert_eq!(output,[25,40,55,0]);
        // A malicious ARGB buffer claiming alpha=0 must never leak RGB.
        let zero=[254u8,200,100,0];
        let layers=[Surface {x:0,y:0,width:1,height:1,
            layer:Layer::Normal,
            pixels:Pixels::PremultipliedAlphaBgra {data:&zero,stride:4}}];
        assert_eq!(compose(1,1,[10,20,30,0],&layers).unwrap(),
                   vec![10,20,30,0]);
    }

    #[test]
    fn rejects_truncation_malformed_stride_and_huge_output() {
        let data = [1, 2, 3, 4];
        let invalid = [Surface { x: -20, y: -20, width: 2, height: 2,
            layer: Layer::Normal,
            pixels: Pixels::OpaqueBgra { data: &data, stride: 4 } }];
        assert_eq!(compose(4, 4, [0; 4], &invalid),
                   Err(SceneError::InvalidSurfaceBuffer));
        assert_eq!(compose(usize::MAX, 5, [0; 4], &[]),
                   Err(SceneError::InvalidDimensions));
    }
}
