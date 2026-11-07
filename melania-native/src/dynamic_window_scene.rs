//! Bounded variable-geometry retained window scene, independent from the
//! existing two fixed QEMU test windows.
//!
//! This is a compositing/data-model step for an eventual native Smithay
//! desktop; it is not a seat, window role policy, KMS backend or login
//! session. All images are owned copies; a borrowed client wl_shm pointer
//! never enters this scene. Output compositing still uses the tested
//! CPU scene::compose path and explicitly represents alpha semantics.
use crate::{damage_regions::DamageRegions,scene::{self, Layer, Pixels, Surface}};

pub const MAX_DYNAMIC_WINDOWS: usize=16;
pub const MAX_WINDOW_SIDE: usize=8192;
pub const MAX_TOTAL_CLIENT_BYTES: usize=96*1024*1024;
pub const MAX_OUTPUT_BYTES: usize=64*1024*1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DynamicAlpha {
    Opaque,
    // wl_shm ARGB8888: BGRA RGB components are already premultiplied.
    Premultiplied,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DynamicSceneError {
    InvalidOutput,
    InvalidClient,
    TooManyWindows,
    ClientMemoryBudget,
    UnknownWindow,
    RenderFailed,
}

#[derive(Debug)]
pub struct WindowPixels {
    width: usize,
    height: usize,
    alpha: DynamicAlpha,
    data: Vec<u8>,
}

impl WindowPixels {
    pub fn new(width:usize,height:usize,alpha:DynamicAlpha,data:Vec<u8>)
        ->Result<Self,DynamicSceneError>
    {
        let size=width.checked_mul(height).and_then(|n|n.checked_mul(4))
            .ok_or(DynamicSceneError::InvalidClient)?;
        if width==0 || height==0 || width>MAX_WINDOW_SIDE ||
           height>MAX_WINDOW_SIDE || size>MAX_OUTPUT_BYTES ||
           data.len()!=size
        {
            return Err(DynamicSceneError::InvalidClient);
        }
        Ok(Self {width,height,alpha,data})
    }
    /// Convert the fully validated, owned Wayland wl_shm copy from Stage51.
    /// No borrowed pool mapping survives this conversion.
    pub fn from_owned_shm(image:crate::owned_shm::OwnedClientPixels)
        ->Result<Self,DynamicSceneError>
    {
        let alpha=match image.alpha {
            crate::owned_shm::AlphaType::Opaque=>DynamicAlpha::Opaque,
            crate::owned_shm::AlphaType::Premultiplied=>DynamicAlpha::Premultiplied,
        };
        Self::new(image.width,image.height,alpha,image.data)
    }

    pub fn width(&self)->usize {self.width}
    pub fn height(&self)->usize {self.height}
    pub fn alpha(&self)->DynamicAlpha {self.alpha}
    pub fn bytes(&self)->usize {self.data.len()}
}

#[derive(Debug)]
struct Window {
    key:u64,
    x:i32,
    y:i32,
    layer:Layer,
    image:WindowPixels,
    /// A minimized client retains its owned buffer but does not paint or receive input.
    visible:bool,
}

#[derive(Debug)]
pub struct DynamicWindowScene {
    width:usize,
    height:usize,
    background:[u8;4],
    windows:Vec<Window>,
    used_bytes:usize,
    generation:u64,
    damage:DamageRegions,
}

impl DynamicWindowScene {
    fn output_is_valid(width:usize,height:usize)->bool {
        width>0 && height>0 &&
        width.checked_mul(height).and_then(|n|n.checked_mul(4))
             .is_some_and(|bytes|bytes<=MAX_OUTPUT_BYTES)
    }

    pub fn new(width:usize,height:usize,background:[u8;4])
        ->Result<Self,DynamicSceneError>
    {
        if !Self::output_is_valid(width,height) {
            return Err(DynamicSceneError::InvalidOutput);
        }
        Ok(Self {width,height,background,windows:Vec::new(),
            used_bytes:0,generation:0,
            damage:DamageRegions::new(width as u32,height as u32)
                .map_err(|_|DynamicSceneError::InvalidOutput)?})
    }

    pub fn output_size(&self)->(usize,usize) {(self.width,self.height)}
    pub fn window_count(&self)->usize {self.windows.len()}
    pub fn used_bytes(&self)->usize {self.used_bytes}
    pub fn generation(&self)->u64 {self.generation}
    pub fn damage_regions(&self)->&DamageRegions {&self.damage}

    /// Retire damaged pixels only after a REAL KMS ACK of this exact scene.
    /// A newer compositor update keeps its invalidated rectangles intact.
    pub fn confirm_presented_damage(&mut self,ack_generation:u64)->bool {
        if ack_generation!=self.generation {return false;}
        self.damage.clear_after_presented();
        true
    }

    fn dirty(&mut self) {
        // Serial numbers are only advisory to the renderer; never accept
        // a KMS presentation based solely on this value.
        self.generation=self.generation.wrapping_add(1);
    }

    pub fn resize_output(&mut self,width:usize,height:usize)
        ->Result<(),DynamicSceneError>
    {
        if !Self::output_is_valid(width,height) {
            return Err(DynamicSceneError::InvalidOutput);
        }
        if self.width!=width || self.height!=height {
            self.width=width;
            self.height=height;
            self.damage.resize_output(width as u32,height as u32)
                .map_err(|_|DynamicSceneError::InvalidOutput)?;
            self.dirty();
        }
        Ok(())
    }

    /// Create a window or replace a previous image/geometry for the same
    /// stable compositor-assigned ID. Limits are checked BEFORE mutation.
    pub fn upsert(&mut self,key:u64,x:i32,y:i32,image:WindowPixels)
        ->Result<(),DynamicSceneError>
    {
        self.upsert_in_layer(key,x,y,Layer::Normal,image)
    }

    /// The default upsert above remains unchanged for all existing QEMU
    /// xdg_toplevel clients. A future real layer-shell wl_shm handler can
    /// supply its role here, without allowing a normal window to paint over
    /// the taskbar or wallpaper to paint over a toplevel.
    pub fn upsert_in_layer(&mut self,key:u64,x:i32,y:i32,layer:Layer,image:WindowPixels)
        ->Result<(),DynamicSceneError>
    {
        if key==0 {return Err(DynamicSceneError::InvalidClient);}
        let previous=self.windows.iter().position(|w|w.key==key);
        if previous.is_none() && self.windows.len()==MAX_DYNAMIC_WINDOWS {
            return Err(DynamicSceneError::TooManyWindows);
        }
        let old_bytes=previous.map(|n|self.windows[n].image.bytes()).unwrap_or(0);
        let next=self.used_bytes.checked_sub(old_bytes)
            .and_then(|total|total.checked_add(image.bytes()))
            .ok_or(DynamicSceneError::ClientMemoryBudget)?;
        if next>MAX_TOTAL_CLIENT_BYTES {
            return Err(DynamicSceneError::ClientMemoryBudget);
        }
        let old_rect=previous.map(|n|{
            let previous=&self.windows[n];
            (previous.x,previous.y,previous.image.width,previous.image.height,previous.visible)
        });
        let new_rect=(x,y,image.width,image.height);
        self.used_bytes=next;
        if let Some(n)=previous {
            let existing=&mut self.windows[n];
            existing.x=x;
            existing.y=y;
            existing.layer=layer;
            existing.image=image;
        } else {
            self.windows.push(Window {key,x,y,layer,image,visible:true});
        }
        if let Some((ox,oy,ow,oh,true))=old_rect {
            self.damage.invalidate(ox,oy,ow,oh);
        }
        // A hidden client's new wl_shm buffer must remain invisible until
        // the shell explicitly restores it. Keep ownership/accounting.
        if self.is_visible(key)==Some(true) {
            self.damage.invalidate(new_rect.0,new_rect.1,new_rect.2,new_rect.3);
        }
        self.dirty();
        Ok(())
    }

    /// Change visibility without dropping the compositor-owned wl_shm copy.
    /// Damage the full rectangle on both minimize and restore so that the
    /// next confirmed KMS frame removes or repaints all affected pixels.
    pub fn set_visible(&mut self,key:u64,visible:bool)
        ->Result<bool,DynamicSceneError>
    {
        let window=self.windows.iter_mut().find(|w|w.key==key)
            .ok_or(DynamicSceneError::UnknownWindow)?;
        if window.visible==visible {return Ok(false);}
        window.visible=visible;
        self.damage.invalidate(window.x,window.y,window.image.width,window.image.height);
        self.dirty();
        Ok(true)
    }
    pub fn is_visible(&self,key:u64)->Option<bool>{
        self.windows.iter().find(|w|w.key==key).map(|w|w.visible)
    }

    pub fn move_to(&mut self,key:u64,x:i32,y:i32)
        ->Result<(),DynamicSceneError>
    {
        let window=self.windows.iter_mut().find(|w|w.key==key)
            .ok_or(DynamicSceneError::UnknownWindow)?;
        if window.x!=x || window.y!=y {
            let old=(window.x,window.y,window.image.width,window.image.height);
            let new=(x,y,window.image.width,window.image.height);
            window.x=x;window.y=y;
            if window.visible {self.damage.window_moved(old,new);}
            self.dirty();
        }
        Ok(())
    }

    pub fn raise(&mut self,key:u64)->Result<bool,DynamicSceneError> {
        let n=self.windows.iter().position(|w|w.key==key)
            .ok_or(DynamicSceneError::UnknownWindow)?;
        if n+1==self.windows.len() {return Ok(false);}
        let entry=self.windows.remove(n);
        if entry.visible {
            self.damage.invalidate(entry.x,entry.y,entry.image.width,entry.image.height);
        }
        self.windows.push(entry);
        self.dirty();
        Ok(true)
    }

    pub fn remove(&mut self,key:u64)->bool {
        let Some(n)=self.windows.iter().position(|w|w.key==key) else {
            return false;
        };
        let removed=self.windows.remove(n);
        self.used_bytes-=removed.image.bytes();
        if removed.visible {
            self.damage.invalidate(removed.x,removed.y,removed.image.width,removed.image.height);
        }
        self.dirty();
        true
    }

    /// Topmost to bottommost *visible normal-layer* client windows only.
    /// Never select GTK layer-shell, minimized, or disconnected surfaces in
    /// the keyboard task switcher; their wl_surface IDs are not focus targets.
    pub fn focusable_normal_keys_top_first(&self)->Vec<u64> {
        self.windows.iter().rev()
            .filter(|w|w.visible && w.layer==Layer::Normal)
            .map(|w|w.key).collect()
    }

    pub fn position(&self,key:u64)->Option<(i32,i32)> {
        self.windows.iter().find(|w|w.key==key).map(|w|(w.x,w.y))
    }

    /// Window geometry is taken from the currently retained client pixels,
    /// never from Stage48's hard-coded 360x224 test window dimensions.
    pub fn geometry(&self,key:u64)->Option<(i32,i32,usize,usize)> {
        self.windows.iter().find(|w|w.key==key)
            .map(|w|(w.x,w.y,w.image.width,w.image.height))
    }

    pub fn hit_test(&self,x:f64,y:f64)->Option<u64> {
        if !x.is_finite() || !y.is_finite() || x<0.0 || y<0.0 ||
           x>=self.width as f64 || y>=self.height as f64 {return None;}
        // Match scene::compose: higher layer wins, then later insertion
        // order within the same layer. Do not allow a normal toplevel to
        // steal a panel's pointer hit merely because it committed later.
        let mut target:Option<&Window>=None;
        for window in self.windows.iter().filter(|w|w.visible) {
            let left=f64::from(window.x);
            let top=f64::from(window.y);
            if x>=left && y>=top &&
               x<left+window.image.width as f64 &&
               y<top+window.image.height as f64 &&
               target.is_none_or(|old|window.layer>=old.layer) {
                target=Some(window);
            }
        }
        target.map(|window|window.key)
    }

    /// Patch only changed pixels in an owned frame from the most recent
    /// acknowledged presentation; never use an unconfirmed KMS candidate.
    /// The caller owns frame lifetime and confirms damage separately.
    pub fn composite_damaged_into(&self,frame:&mut [u8])
        ->Result<(),DynamicSceneError>
    {
        let size=self.width.checked_mul(self.height)
            .and_then(|n|n.checked_mul(4)).ok_or(DynamicSceneError::RenderFailed)?;
        if frame.len()!=size {return Err(DynamicSceneError::RenderFailed);}
        if self.damage.is_clean() {return Ok(());}
        if self.damage.is_full() {
            frame.copy_from_slice(&self.composite()?);
            return Ok(());
        }
        let surfaces=self.windows.iter().filter(|w|w.visible).map(|w|Surface {
            x:w.x,y:w.y,width:w.image.width,height:w.image.height,
            layer:w.layer,
            pixels:match w.image.alpha {
                DynamicAlpha::Opaque=>Pixels::OpaqueBgra {
                    data:&w.image.data,stride:w.image.width*4
                },
                DynamicAlpha::Premultiplied=>Pixels::PremultipliedAlphaBgra {
                    data:&w.image.data,stride:w.image.width*4
                },
            },
        }).collect::<Vec<_>>();
        crate::region_compositor::compose_regions_into(
            self.width,self.height,self.background,&surfaces,
            self.damage.regions(),frame
        ).map_err(|_|DynamicSceneError::RenderFailed)
    }

    /// CPU reference renderer. Geometry clips to the selected output and
    /// per-window alpha uses the Wayland premultiplied semantics, without
    /// altering the live fixed-size QEMU compositor or grabbing DRM master.
    pub fn composite(&self)->Result<Vec<u8>,DynamicSceneError> {
        let surfaces=self.windows.iter().filter(|w|w.visible).map(|w| {
            Surface {
                x:w.x,y:w.y,
                width:w.image.width,height:w.image.height,
                layer:w.layer,
                pixels:match w.image.alpha {
                    DynamicAlpha::Opaque=>Pixels::OpaqueBgra {
                        data:&w.image.data,stride:w.image.width*4
                    },
                    DynamicAlpha::Premultiplied=>Pixels::PremultipliedAlphaBgra {
                        data:&w.image.data,stride:w.image.width*4
                    },
                },
            }
        }).collect::<Vec<_>>();
        scene::compose(self.width,self.height,self.background,&surfaces)
            .map_err(|_|DynamicSceneError::RenderFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_switcher_filters_minimized_and_layer_shell_windows() {
        let mut scene=DynamicWindowScene::new(128,90,[1,2,3,0]).unwrap();
        let make=||WindowPixels::new(12,12,DynamicAlpha::Opaque,
                                    vec![42;12*12*4]).unwrap();
        scene.upsert(1,1,1,make()).unwrap();
        scene.upsert(2,2,2,make()).unwrap();
        scene.upsert(3,3,3,make()).unwrap();
        scene.upsert_in_layer(4,4,4,Layer::Top,make()).unwrap();
        assert_eq!(scene.focusable_normal_keys_top_first(),vec![3,2,1]);
        scene.set_visible(2,false).unwrap();
        assert_eq!(scene.focusable_normal_keys_top_first(),vec![3,1]);
        scene.raise(1).unwrap();
        assert_eq!(scene.focusable_normal_keys_top_first(),vec![1,3]);
        scene.remove(1);
        assert_eq!(scene.focusable_normal_keys_top_first(),vec![3]);
        scene.set_visible(3,false).unwrap();
        assert!(scene.focusable_normal_keys_top_first().is_empty());
    }

    fn solid(w:usize,h:usize,color:[u8;4],alpha:DynamicAlpha)->WindowPixels {
        let mut data=vec![0;w*h*4];
        for px in data.chunks_exact_mut(4) {px.copy_from_slice(&color);}
        WindowPixels::new(w,h,alpha,data).unwrap()
    }

    fn px(frame:&[u8],width:usize,x:usize,y:usize)->[u8;3] {
        let n=(y*width+x)*4;
        [frame[n],frame[n+1],frame[n+2]]
    }


    #[test]
    fn swap_two_acknowledged_same_size_windows_reuses_owned_pixels_and_damages_scene() {
        // Super+X swaps the two physical tile positions, but XDG clients
        // only know their size. New client buffers must not be invented or
        // awaited for a pure x/y-only relocation of already accepted pixels.
        let mut scene=DynamicWindowScene::new(128,64,[7,8,9,0]).unwrap();
        scene.upsert(11,0,0,solid(64,64,[10,20,30,0],DynamicAlpha::Opaque)).unwrap();
        scene.upsert(22,64,0,solid(64,64,[40,50,60,0],DynamicAlpha::Opaque)).unwrap();
        let initial=scene.generation();
        let total=scene.used_bytes();
        assert!(scene.confirm_presented_damage(initial));
        assert_eq!(px(&scene.composite().unwrap(),128,10,20),[10,20,30]);
        assert_eq!(px(&scene.composite().unwrap(),128,110,20),[40,50,60]);
        scene.move_to(11,64,0).unwrap();
        scene.move_to(22,0,0).unwrap();
        assert_eq!(scene.window_count(),2);
        assert_eq!(scene.used_bytes(),total);
        assert_eq!(scene.geometry(11),Some((64,0,64,64)));
        assert_eq!(scene.geometry(22),Some((0,0,64,64)));
        assert_eq!(scene.hit_test(10.0,20.0),Some(22));
        assert_eq!(scene.hit_test(110.0,20.0),Some(11));
        let swapped=scene.composite().unwrap();
        assert_eq!(px(&swapped,128,10,20),[40,50,60]);
        assert_eq!(px(&swapped,128,110,20),[10,20,30]);
        assert!(scene.generation()>initial);
        assert!(!scene.confirm_presented_damage(initial));
        assert!(scene.confirm_presented_damage(scene.generation()));
    }

    #[test]
    fn minimized_layered_window_retains_pixels_but_never_captures_pointer() {
        let mut scene=DynamicWindowScene::new(120,80,[1,2,3,0]).unwrap();
        scene.upsert_in_layer(1,10,10,Layer::Normal,
            solid(28,28,[70,80,90,0],DynamicAlpha::Opaque)).unwrap();
        scene.upsert_in_layer(2,18,18,Layer::Top,
            solid(22,22,[20,30,40,0],DynamicAlpha::Opaque)).unwrap();
        assert_eq!(scene.hit_test(20.0,20.0),Some(2));
        assert_eq!(px(&scene.composite().unwrap(),120,21,21),[20,30,40]);
        let bytes=scene.used_bytes();
        let before=scene.generation();
        assert_eq!(scene.set_visible(2,false),Ok(true));
        assert_eq!(scene.is_visible(2),Some(false));
        assert!(scene.generation()>before);
        assert_eq!(scene.hit_test(20.0,20.0),Some(1));
        assert_eq!(px(&scene.composite().unwrap(),120,21,21),[70,80,90]);
        assert_eq!(scene.used_bytes(),bytes);
        assert_eq!(scene.set_visible(2,false),Ok(false));
        assert!(scene.confirm_presented_damage(scene.generation()));
        scene.upsert_in_layer(2,18,18,Layer::Top,
            solid(22,22,[80,90,100,0],DynamicAlpha::Opaque)).unwrap();
        assert_eq!(scene.is_visible(2),Some(false));
        assert_eq!(scene.hit_test(20.0,20.0),Some(1));
        let mut composed=scene.composite().unwrap();
        scene.composite_damaged_into(&mut composed).unwrap();
        assert_eq!(px(&composed,120,21,21),[70,80,90]);
        assert_eq!(scene.set_visible(2,true),Ok(true));
        assert_eq!(scene.hit_test(20.0,20.0),Some(2));
        assert_eq!(px(&scene.composite().unwrap(),120,21,21),[80,90,100]);
        assert_eq!(scene.set_visible(999,false),Err(DynamicSceneError::UnknownWindow));
    }

    #[test]
    fn different_client_sizes_retain_pixels_on_other_client_damage() {
        let mut s=DynamicWindowScene::new(120,80,[1,2,3,0]).unwrap();
        s.upsert(101,4,5,solid(20,15,[11,12,13,0],DynamicAlpha::Opaque)).unwrap();
        s.upsert(203,70,45,solid(35,25,[20,21,22,0],DynamicAlpha::Opaque)).unwrap();
        let first=s.composite().unwrap();
        assert_eq!(px(&first,120,5,6),[11,12,13]);
        assert_eq!(px(&first,120,80,50),[20,21,22]);
        let old_count=s.window_count();
        s.upsert(101,4,5,solid(8,9,[90,80,70,0],DynamicAlpha::Opaque)).unwrap();
        let second=s.composite().unwrap();
        assert_eq!(px(&second,120,5,6),[90,80,70]);
        assert_eq!(px(&second,120,80,50),[20,21,22]);
        assert_eq!(px(&second,120,22,18),[1,2,3]);
        assert_eq!(s.window_count(),old_count);
    }

    #[test]
    fn overlapping_focus_raise_and_real_premultiplied_alpha() {
        let mut s=DynamicWindowScene::new(20,20,[10,20,30,0]).unwrap();
        s.upsert(1,2,2,solid(8,8,[60,70,80,0],DynamicAlpha::Opaque)).unwrap();
        s.upsert(2,5,5,solid(5,4,[20,30,40,128],
                                      DynamicAlpha::Premultiplied)).unwrap();
        assert_eq!(s.hit_test(6.0,6.0),Some(2));
        assert_eq!(px(&s.composite().unwrap(),20,6,6),[50,65,80]);
        assert_eq!(s.raise(1),Ok(true));
        assert_eq!(s.hit_test(6.0,6.0),Some(1));
        assert_eq!(px(&s.composite().unwrap(),20,6,6),[60,70,80]);
        assert_eq!(s.raise(1),Ok(false));
        assert_eq!(s.hit_test(f64::NAN,2.0),None);
        assert_eq!(s.hit_test(20.0,2.0),None);
    }

    #[test]
    fn move_close_reopen_and_output_change_clip_without_stale_pixels() {
        let mut s=DynamicWindowScene::new(10,10,[1,2,3,0]).unwrap();
        s.upsert(1,0,0,solid(6,6,[90,80,70,0],DynamicAlpha::Opaque)).unwrap();
        s.upsert(2,6,6,solid(6,6,[8,7,6,0],DynamicAlpha::Opaque)).unwrap();
        assert_eq!(px(&s.composite().unwrap(),10,9,9),[8,7,6]);
        assert_eq!(s.move_to(2,-3,-3),Ok(()));
        assert_eq!(px(&s.composite().unwrap(),10,9,9),[1,2,3]);
        assert!(s.remove(1));
        assert_eq!(s.window_count(),1);
        assert_eq!(px(&s.composite().unwrap(),10,5,5),[1,2,3]);
        s.upsert(1,4,4,solid(2,2,[5,4,3,0],DynamicAlpha::Opaque)).unwrap();
        assert_eq!(s.position(1),Some((4,4)));
        assert_eq!(px(&s.composite().unwrap(),10,4,4),[5,4,3]);
        let generation=s.generation();
        s.resize_output(16,12).unwrap();
        assert!(s.generation()>generation);
        assert_eq!(s.composite().unwrap().len(),16*12*4);
        assert_eq!(px(&s.composite().unwrap(),16,15,11),[1,2,3]);
    }

    #[test]
    fn pointer_geometry_uses_updated_owned_client_size() {
        let mut s=DynamicWindowScene::new(1280,800,[0,0,0,0]).unwrap();
        s.upsert(1,160,120,solid(400,240,[30,60,90,0],DynamicAlpha::Opaque)).unwrap();
        assert_eq!(s.geometry(1),Some((160,120,400,240)));
        assert_eq!(s.hit_test(520.0,330.0),Some(1));
        s.upsert(1,160,120,solid(320,192,[45,70,100,0],DynamicAlpha::Opaque)).unwrap();
        assert_eq!(s.geometry(1),Some((160,120,320,192)));
        assert_eq!(s.hit_test(520.0,330.0),None);
        s.move_to(1,240,230).unwrap();
        assert_eq!(s.geometry(1),Some((240,230,320,192)));
        assert_eq!(s.hit_test(270.0,250.0),Some(1));
        assert!(!s.remove(2));
        assert!(s.remove(1));
        assert_eq!(s.geometry(1),None);
    }

    #[test]
    fn size_count_and_memory_budgets_fail_before_mutation() {
        assert!(matches!(DynamicWindowScene::new(usize::MAX,2,[0;4]),
                        Err(DynamicSceneError::InvalidOutput)));
        assert!(matches!(WindowPixels::new(8193,1,DynamicAlpha::Opaque,vec![]),
                        Err(DynamicSceneError::InvalidClient)));
        assert!(matches!(WindowPixels::new(2,2,DynamicAlpha::Opaque,vec![0;12]),
                        Err(DynamicSceneError::InvalidClient)));
        let mut s=DynamicWindowScene::new(50,50,[0;4]).unwrap();
        for id in 1..=MAX_DYNAMIC_WINDOWS as u64 {
            s.upsert(id,0,0,solid(1,1,[1,2,3,0],DynamicAlpha::Opaque)).unwrap();
        }
        let before=s.generation();
        assert_eq!(s.upsert(88,0,0,solid(1,1,[1,2,3,0],DynamicAlpha::Opaque)),
                   Err(DynamicSceneError::TooManyWindows));
        assert_eq!(s.window_count(),MAX_DYNAMIC_WINDOWS);
        assert_eq!(s.generation(),before);
        assert_eq!(s.resize_output(usize::MAX,2),
                   Err(DynamicSceneError::InvalidOutput));
        assert_eq!(s.output_size(),(50,50));
        assert!(!s.remove(900));
    }

    #[test]
    fn dynamic_damage_keeps_old_and_new_window_extents() {
        let mut s=DynamicWindowScene::new(100,80,[1,2,3,0]).unwrap();
        assert!(s.damage_regions().is_clean());
        s.upsert(1,10,10,solid(20,15,[10,20,30,0],DynamicAlpha::Opaque)).unwrap();
        assert_eq!(s.damage_regions().regions()[0],
            crate::damage_regions::DamageRect{x:10,y:10,width:20,height:15});
        let first=s.generation();
        assert!(!s.confirm_presented_damage(first+1));
        assert!(!s.damage_regions().is_clean());
        assert!(s.confirm_presented_damage(first));
        assert!(s.damage_regions().is_clean());
        s.move_to(1,40,30).unwrap();
        assert_eq!(s.damage_regions().regions().len(),2);
        s.upsert(1,40,30,solid(10,8,[10,20,30,0],DynamicAlpha::Opaque)).unwrap();
        assert!(!s.confirm_presented_damage(first));
        assert!(!s.damage_regions().is_clean());
        assert!(s.remove(1));
        assert_eq!(s.window_count(),0);
        assert!(s.confirm_presented_damage(s.generation()));
        assert!(s.damage_regions().is_clean());
    }

    #[test]
    fn dynamic_damage_focus_and_output_change_are_conservative() {
        let mut s=DynamicWindowScene::new(100,80,[0;4]).unwrap();
        s.upsert(1,5,5,solid(20,20,[1,2,3,0],DynamicAlpha::Opaque)).unwrap();
        s.upsert(2,10,10,solid(20,20,[4,5,6,0],DynamicAlpha::Opaque)).unwrap();
        s.confirm_presented_damage(s.generation());
        assert!(s.raise(1).unwrap());
        assert!(!s.damage_regions().is_clean());
        s.confirm_presented_damage(s.generation());
        assert!(!s.raise(1).unwrap());
        assert!(s.damage_regions().is_clean());
        s.resize_output(150,100).unwrap();
        assert!(s.damage_regions().is_full());
        assert_eq!(s.damage_regions().output_size(),(150,100));
    }


    #[test]
    fn damaged_composition_equals_full_after_move_shrink_close_and_alpha() {
        let mut s=DynamicWindowScene::new(90,70,[10,20,30,0]).unwrap();
        s.upsert(1,10,10,solid(30,30,[70,80,90,0],DynamicAlpha::Opaque)).unwrap();
        s.upsert(2,20,20,solid(20,20,[12,18,24,128],DynamicAlpha::Premultiplied)).unwrap();
        let mut last=s.composite().unwrap();
        s.confirm_presented_damage(s.generation());
        s.move_to(2,35,28).unwrap();
        s.composite_damaged_into(&mut last).unwrap();
        assert_eq!(last,s.composite().unwrap());
        s.confirm_presented_damage(s.generation());
        s.upsert(1,10,10,solid(9,7,[40,50,60,0],DynamicAlpha::Opaque)).unwrap();
        s.composite_damaged_into(&mut last).unwrap();
        assert_eq!(last,s.composite().unwrap());
        s.confirm_presented_damage(s.generation());
        assert!(s.remove(2));
        s.composite_damaged_into(&mut last).unwrap();
        assert_eq!(last,s.composite().unwrap());
        s.resize_output(110,80).unwrap();
        let mut bigger=vec![99u8;110*80*4];
        s.composite_damaged_into(&mut bigger).unwrap();
        assert_eq!(bigger,s.composite().unwrap());
    }

    #[test]
    fn clean_frame_skips_repainting_and_rejects_wrong_size() {
        let mut s=DynamicWindowScene::new(10,10,[1,2,3,0]).unwrap();
        let mut frame=vec![37u8;10*10*4];
        s.composite_damaged_into(&mut frame).unwrap();
        assert!(frame.iter().all(|b|*b==37));
        assert_eq!(s.composite_damaged_into(&mut frame[..20]),
                   Err(DynamicSceneError::RenderFailed));
    }

    #[test]
    fn top_panel_overrides_newer_normal_window_in_pixels_and_hit_testing() {
        let mut s=DynamicWindowScene::new(60,40,[1,2,3,0]).unwrap();
        // Taskbar is inserted FIRST, before the normal window. Logical
        // scene z-order, not insertion order alone, must keep it in front.
        s.upsert_in_layer(11,0,0,Layer::Top,
            solid(60,8,[120,130,140,0],DynamicAlpha::Opaque)).unwrap();
        s.upsert(22,4,2,solid(30,20,[40,50,60,0],DynamicAlpha::Opaque)).unwrap();
        assert_eq!(px(&s.composite().unwrap(),60,10,4),[120,130,140]);
        assert_eq!(s.hit_test(10.0,4.0),Some(11));
        assert_eq!(s.hit_test(10.0,10.0),Some(22));
        assert_eq!(s.raise(22),Ok(false));
        assert_eq!(px(&s.composite().unwrap(),60,10,4),[120,130,140]);
    }

    #[test]
    fn role_layer_reordering_invalidates_pixels_until_presented_ack() {
        let mut s=DynamicWindowScene::new(30,20,[1,2,3,0]).unwrap();
        s.upsert_in_layer(1,0,0,Layer::Top,
            solid(30,6,[80,90,100,0],DynamicAlpha::Opaque)).unwrap();
        s.upsert(2,0,0,solid(30,20,[30,40,50,0],DynamicAlpha::Opaque)).unwrap();
        let previous_presented=s.composite().unwrap();
        let acknowledged=s.generation();
        assert!(s.confirm_presented_damage(acknowledged));
        assert!(s.damage_regions().is_clean());
        s.upsert_in_layer(1,0,0,Layer::Bottom,
            solid(30,6,[80,90,100,0],DynamicAlpha::Opaque)).unwrap();
        assert!(!s.damage_regions().is_clean());
        assert!(!s.confirm_presented_damage(acknowledged));
        assert_eq!(px(&s.composite().unwrap(),30,4,4),[30,40,50]);
        assert_eq!(s.hit_test(4.0,4.0),Some(2));
        let mut candidate=previous_presented;
        s.composite_damaged_into(&mut candidate).unwrap();
        assert_eq!(candidate,s.composite().unwrap());
        assert!(s.confirm_presented_damage(s.generation()));
    }

    #[test]
    fn overlay_premultiplied_alpha_blends_over_normal_window_once() {
        let mut s=DynamicWindowScene::new(12,12,[0,0,0,0]).unwrap();
        s.upsert(1,0,0,
            solid(12,12,[60,70,80,0],DynamicAlpha::Opaque)).unwrap();
        s.upsert_in_layer(2,0,0,Layer::Overlay,
            solid(12,4,[20,30,40,128],DynamicAlpha::Premultiplied)).unwrap();
        assert_eq!(px(&s.composite().unwrap(),12,2,2),[50,65,80]);
        assert_eq!(s.hit_test(2.0,2.0),Some(2));
        assert!(s.remove(2));
        assert_eq!(px(&s.composite().unwrap(),12,2,2),[60,70,80]);
    }

}
