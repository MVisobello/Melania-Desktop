//! Bounded retained scene of client-owned wl_shm pixel snapshots.
//!
//! The compositor owns COPIES of client pixels; it never keeps a mapped
//! wl_shm pointer after Smithay returns. This scene is strictly the QEMU
//! demonstration: two initially positioned windows with bounded move, focus and hit-testing, not
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
    InvalidPosition,
    CompositionFailed,
}

#[derive(Debug)]
struct Entry {
    key: u64,
    x: i32,
    y: i32,
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
        let (x,y)=POSITIONS[self.windows.len()];
        self.windows.push(Entry { key, x, y, pixels });
        Ok(())
    }

    pub fn remove(&mut self, key: u64) -> bool {
        let previous = self.windows.len();
        self.windows.retain(|w| w.key != key);
        self.windows.len() != previous
    }

    /// Coordinates are physical pixels of the single QEMU output.
    /// Reject off-screen moves rather than relying on integer wrap/clipping.
    pub fn move_to(&mut self, key:u64, x:i32, y:i32) -> Result<(),MultiSceneError> {
        if x<0 || y<0 || x as usize>OUTPUT_W-WIN_W ||
           y as usize>OUTPUT_H-WIN_H {
            return Err(MultiSceneError::InvalidPosition);
        }
        let entry=self.windows.iter_mut().find(|w|w.key==key)
            .ok_or(MultiSceneError::UnknownWindow)?;
        entry.x=x;
        entry.y=y;
        Ok(())
    }

    pub fn position(&self,key:u64)->Option<(i32,i32)> {
        self.windows.iter().find(|w|w.key==key).map(|w|(w.x,w.y))
    }

    /// The last mapped/raised surface is topmost; borders are not hit targets.
    pub fn hit_test(&self,x:f64,y:f64)->Option<u64> {
        if !x.is_finite() || !y.is_finite() {return None;}
        self.windows.iter().rev().find(|w| {
            x>=f64::from(w.x) && y>=f64::from(w.y) &&
            x<f64::from(w.x)+WIN_W as f64 &&
            y<f64::from(w.y)+WIN_H as f64
        }).map(|w|w.key)
    }

    /// Raising a window preserves its stable key and geometry.
    pub fn raise(&mut self,key:u64)->bool {
        let Some(index)=self.windows.iter().position(|w|w.key==key) else {
            return false;
        };
        let entry=self.windows.remove(index);
        self.windows.push(entry);
        true
    }

    /// The scene compositor paints BOTH retained windows on every update:
    /// window 1's pixels survive window 2's independent wl_surface commit.
    pub fn composite(&self) -> Result<Vec<u8>, MultiSceneError> {
        let mut surfaces=Vec::with_capacity(self.windows.len()*2);
        for window in self.windows.iter() {
            let (x,y)=(window.x,window.y);
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
        assert_eq!(sample(&graph.composite().unwrap(),200,200),[27,33,58]);
        assert_eq!(sample(&graph.composite().unwrap(),700,250),[4,5,6]);
    }
    #[test]
    fn move_hit_test_and_raise_preserve_stable_id() {
        let mut graph=MultiWindowScene::default();
        graph.upsert(11,solid(245,108,34)).unwrap();
        graph.upsert(12,solid(73,126,237)).unwrap();
        assert_eq!(graph.hit_test(200.0,200.0),Some(11));
        assert_eq!(graph.hit_test(800.0,250.0),Some(12));
        assert_eq!(graph.hit_test(-1.0,0.0),None);
        assert_eq!(graph.hit_test(f64::NAN,10.0),None);
        assert_eq!(graph.move_to(11,700,230),Ok(()));
        assert_eq!(graph.hit_test(750.0,300.0),Some(11));
        assert_eq!(graph.position(11),Some((700,230)));
        assert_eq!(graph.hit_test(200.0,200.0),None);
        assert_eq!(graph.hit_test(900.0,270.0),Some(11));
        let overlap=graph.composite().unwrap();
        assert_eq!(sample(&overlap,900,270),[245,108,34]);
        assert!(graph.raise(12));
        assert_eq!(graph.hit_test(900.0,270.0),Some(12));
        let overlap=graph.composite().unwrap();
        assert_eq!(sample(&overlap,900,270),[73,126,237]);
        assert_eq!(graph.position(11),Some((700,230)));
    }

    #[test]
    fn invalid_geometry_and_unknown_window_fail_without_mutation() {
        let mut graph=MultiWindowScene::default();
        graph.upsert(101,solid(1,2,3)).unwrap();
        let old=graph.position(101);
        assert_eq!(graph.move_to(101,-1,20),Err(MultiSceneError::InvalidPosition));
        assert_eq!(graph.move_to(101,9999,20),Err(MultiSceneError::InvalidPosition));
        assert_eq!(graph.move_to(101,20,9999),Err(MultiSceneError::InvalidPosition));
        assert_eq!(graph.move_to(500,100,100),Err(MultiSceneError::UnknownWindow));
        assert!(!graph.raise(500));
        assert_eq!(graph.position(101),old);
    }

}
