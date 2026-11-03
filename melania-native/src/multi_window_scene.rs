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
    layout_generation:u64,
}

impl MultiWindowScene {
    pub fn window_count(&self) -> usize { self.windows.len() }

    /// Incremented whenever a scene mutation requires a new output image.
    /// A KMS presenter must not retire a generation before its own ACK.
    pub fn layout_generation(&self)->u64 { self.layout_generation }
    fn dirty(&mut self) { self.layout_generation=self.layout_generation.wrapping_add(1); }

    /// Keys are compositor-local, never client-provided filenames or paths.
    /// A first commit creates a slot; subsequent commits update that slot.
    pub fn upsert(&mut self, key: u64, pixels: Vec<u8>) -> Result<(), MultiSceneError> {
        if pixels.len() != WINDOW_BYTES {
            return Err(MultiSceneError::InvalidPixelBuffer);
        }
        if let Some(window) = self.windows.iter_mut().find(|w| w.key == key) {
            window.pixels = pixels;
            self.dirty();
            return Ok(());
        }
        if self.windows.len() == MAX_WINDOWS {
            return Err(MultiSceneError::TooManyWindows);
        }
        // Slot IDs 1 and 2 are stable across close/reopen in the QEMU
        // Wayland bridge. Use their actual assigned positions, not the count
        // of currently visible windows: after closing window 1 while window 2
        // remains, the reopened window 1 must return to the left slot rather
        // than overlapping the still-running right-hand window.
        //
        // Arbitrary IDs are retained for standalone scene unit tests; their
        // initial positions follow insertion order as before.
        let slot=usize::try_from(key).ok()
            .filter(|n|(1..=MAX_WINDOWS).contains(n))
            .map(|n|n-1)
            .unwrap_or(self.windows.len());
        let (x,y)=POSITIONS[slot];
        self.windows.push(Entry { key, x, y, pixels });
        self.dirty();
        Ok(())
    }

    pub fn remove(&mut self, key: u64) -> bool {
        let previous = self.windows.len();
        self.windows.retain(|w| w.key != key);
        let changed=self.windows.len()!=previous;
        if changed {self.dirty();}
        changed
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
        let changed=entry.x!=x || entry.y!=y;
        entry.x=x;
        entry.y=y;
        if changed {self.dirty();}
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
        if index+1!=self.windows.len() {self.dirty();}
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
    fn layout_generation_changes_only_for_actual_scene_damage() {
        let mut graph=MultiWindowScene::default();
        assert_eq!(graph.layout_generation(),0);
        graph.upsert(1,solid(1,2,3)).unwrap();
        assert_eq!(graph.layout_generation(),1);
        graph.move_to(1,160,120).unwrap();
        assert_eq!(graph.layout_generation(),1);
        graph.move_to(1,200,150).unwrap();
        assert_eq!(graph.layout_generation(),2);
        assert!(graph.raise(1));
        assert_eq!(graph.layout_generation(),2);
        graph.upsert(2,solid(4,5,6)).unwrap();
        assert_eq!(graph.layout_generation(),3);
        graph.raise(1);
        assert_eq!(graph.layout_generation(),4);
        assert!(!graph.remove(100));
        assert_eq!(graph.layout_generation(),4);
        assert!(graph.remove(1));
        assert_eq!(graph.layout_generation(),5);
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
        assert_eq!(graph.hit_test(750.0,300.0),Some(12));
        assert_eq!(graph.position(11),Some((700,230)));
        assert_eq!(graph.hit_test(200.0,200.0),None);
        // Moving a window must NOT silently raise it: the newer B stays on top.
        assert_eq!(graph.hit_test(900.0,270.0),Some(12));
        assert!(graph.raise(11));
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
    fn closed_first_client_can_reopen_without_overlapping_second() {
        let mut graph=MultiWindowScene::default();
        graph.upsert(1,solid(230,50,40)).unwrap();
        graph.upsert(2,solid(40,110,230)).unwrap();
        let second_position=graph.position(2);
        assert_eq!(second_position,Some((650,190)));
        let before=graph.composite().unwrap();
        assert_eq!(sample(&before,200,200),[230,50,40]);
        assert_eq!(sample(&before,700,250),[40,110,230]);

        // Destruction frees only the first retained wl_shm snapshot. Until
        // the next real KMS presentation the display may show stale pixels;
        // a new composition must already contain the correct background.
        let previous_generation=graph.layout_generation();
        assert!(graph.remove(1));
        assert!(graph.layout_generation()>previous_generation);
        assert_eq!(graph.window_count(),1);
        assert_eq!(graph.position(2),second_position);
        let removed=graph.composite().unwrap();
        assert_eq!(sample(&removed,200,200),[27,33,58]);
        assert_eq!(sample(&removed,700,250),[40,110,230]);

        graph.upsert(1,solid(80,220,90)).unwrap();
        assert_eq!(graph.window_count(),2);
        assert_eq!(graph.position(1),Some((160,120)));
        assert_eq!(graph.position(2),second_position);
        let reopened=graph.composite().unwrap();
        assert_eq!(sample(&reopened,200,200),[80,220,90]);
        assert_eq!(sample(&reopened,700,250),[40,110,230]);
        assert!(!graph.remove(200));
    }

    #[test]
    fn left_slot_remains_free_when_right_client_is_first_to_commit() {
        let mut graph=MultiWindowScene::default();
        graph.upsert(2,solid(30,90,200)).unwrap();
        assert_eq!(graph.position(2),Some((650,190)));
        assert_eq!(sample(&graph.composite().unwrap(),200,200),[27,33,58]);
        graph.upsert(1,solid(180,70,30)).unwrap();
        assert_eq!(graph.position(1),Some((160,120)));
        assert_eq!(sample(&graph.composite().unwrap(),200,200),[180,70,30]);
        assert_eq!(sample(&graph.composite().unwrap(),700,250),[30,90,200]);
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
