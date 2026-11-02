//! Bounded retained scene of client-owned wl_shm pixel snapshots.
//!
//! The compositor owns COPIES of client pixels; it never keeps a mapped
//! wl_shm pointer after Smithay returns. This scene is strictly the QEMU
//! demonstration: two fixed-size windows and deterministic positions, not
//! an unbounded native window manager or a replacement for xdg-shell policy.
use crate::scene::{self, Layer, Pixels, Surface};

pub const WIN_W: usize = 360;
pub const WIN_H: usize = 224;
pub const MAX_WINDOWS: usize = 2;
const WINDOW_BYTES: usize = WIN_W * WIN_H * 4;
const OUTPUT_W: usize = 1280;
const OUTPUT_H: usize = 800;
const POSITIONS: [(i32, i32); MAX_WINDOWS] = [(160, 120), (650, 190)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiSceneError {
    InvalidPixelBuffer,
    TooManyWindows,
    UnknownWindow,
    CompositionFailed,
}

#[derive(Debug)]
struct Entry {
    key: u64,
    pixels: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct MultiWindowScene {
    windows: Vec<Entry>,
}

impl MultiWindowScene {
    pub fn window_count(&self) -> usize { self.windows.len() }

    /// Keys are compositor-local, never client-provided filenames or paths.
    /// A first commit creates a slot; subsequent commits update that slot.
    pub fn upsert(&mut self, key: u64, pixels: Vec<u8>) -> Result<(), MultiSceneError> {
        if pixels.len() != WINDOW_BYTES {
            return Err(MultiSceneError::InvalidPixelBuffer);
        }
        if let Some(window) = self.windows.iter_mut().find(|w| w.key == key) {
            window.pixels = pixels;
            return Ok(());
        }
        if self.windows.len() == MAX_WINDOWS {
            return Err(MultiSceneError::TooManyWindows);
        }
        self.windows.push(Entry { key, pixels });
        Ok(())
    }

    pub fn remove(&mut self, key: u64) -> bool {
        let previous = self.windows.len();
        self.windows.retain(|w| w.key != key);
        self.windows.len() != previous
    }

    /// The scene compositor paints BOTH retained windows on every update:
    /// window 1's pixels survive window 2's independent wl_surface commit.
    pub fn composite(&self) -> Result<Vec<u8>, MultiSceneError> {
        let mut surfaces=Vec::with_capacity(self.windows.len()*2);
        for (index,window) in self.windows.iter().enumerate() {
            let (x,y)=POSITIONS[index];
            surfaces.push(Surface {
                x:x-6,y:y-6,width:WIN_W+12,height:WIN_H+12,
                layer:Layer::Bottom,
                pixels:Pixels::Solid([216,171,65,0]),
            });
            surfaces.push(Surface {
                x,y,width:WIN_W,height:WIN_H,
                layer:Layer::Normal,
                pixels:Pixels::OpaqueBgra{data:&window.pixels,stride:WIN_W*4},
            });
        }
        scene::compose(OUTPUT_W,OUTPUT_H,[58,33,27,0],&surfaces)
            .map_err(|_|MultiSceneError::CompositionFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn solid(r:u8,g:u8,b:u8)->Vec<u8> {
        let mut v=vec![0;WINDOW_BYTES];
        for p in v.chunks_exact_mut(4) {p.copy_from_slice(&[b,g,r,0]);}
        v
    }
    fn sample(frame:&[u8],x:usize,y:usize)->[u8;3] {
        let p=(y*OUTPUT_W+x)*4;
        [frame[p+2],frame[p+1],frame[p]]
    }

    #[test]
    fn two_independent_client_surfaces_remain_visible_after_redraw() {
        let mut graph=MultiWindowScene::default();
        graph.upsert(11,solid(245,108,34)).unwrap();
        let first=graph.composite().unwrap();
        assert_eq!(sample(&first,200,200),[245,108,34]);
        assert_eq!(sample(&first,700,250),[27,33,58]);
        graph.upsert(12,solid(73,126,237)).unwrap();
        let second=graph.composite().unwrap();
        assert_eq!(sample(&second,200,200),[245,108,34]);
        assert_eq!(sample(&second,700,250),[73,126,237]);
        graph.upsert(11,solid(222,64,118)).unwrap();
        let third=graph.composite().unwrap();
        assert_eq!(sample(&third,200,200),[222,64,118]);
        assert_eq!(sample(&third,700,250),[73,126,237]);
        assert_eq!(graph.window_count(),2);
    }

    #[test]
    fn rejects_overflow_and_does_not_mutate_existing_image() {
        let mut graph=MultiWindowScene::default();
        graph.upsert(1,solid(1,2,3)).unwrap();
        assert_eq!(graph.upsert(1,vec![1,2,3]),Err(MultiSceneError::InvalidPixelBuffer));
        assert_eq!(sample(&graph.composite().unwrap(),200,200),[1,2,3]);
        graph.upsert(2,solid(4,5,6)).unwrap();
        assert_eq!(graph.upsert(3,solid(7,8,9)),Err(MultiSceneError::TooManyWindows));
        assert_eq!(graph.window_count(),2);
    }

    #[test]
    fn removal_drops_owned_pixels_and_keeps_other_window() {
        let mut graph=MultiWindowScene::default();
        graph.upsert(1,solid(1,2,3)).unwrap();
        graph.upsert(2,solid(4,5,6)).unwrap();
        assert!(!graph.remove(90));
        assert!(graph.remove(1));
        assert_eq!(graph.window_count(),1);
        assert_eq!(sample(&graph.composite().unwrap(),200,200),[4,5,6]);
    }
}
