//! Geometry policy for future xdg popup positioning. The compositor still
//! needs to wire this into its xdg-positioner configure/placement path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect { pub x:i32, pub y:i32, pub width:u32, pub height:u32 }
/// Clamp a popup to the viewport after preferred placement. Oversize content
/// is reduced to fit rather than wrapping an unsigned subtraction.
pub fn constrain(mut popup:Rect, viewport:Rect, margin:u32)->Rect {
    let margin = margin.min(viewport.width/2).min(viewport.height/2) as i32;
    let left = viewport.x.saturating_add(margin);
    let top = viewport.y.saturating_add(margin);
    let available_w = viewport.width.saturating_sub((margin*2) as u32);
    let available_h = viewport.height.saturating_sub((margin*2) as u32);
    popup.width = popup.width.min(available_w);
    popup.height = popup.height.min(available_h);
    let right = left.saturating_add(available_w as i32).saturating_sub(popup.width as i32);
    let bottom = top.saturating_add(available_h as i32).saturating_sub(popup.height as i32);
    popup.x = popup.x.clamp(left,right);
    popup.y = popup.y.clamp(top,bottom);
    popup
}
#[cfg(test)] mod tests {
  use super::*;
  #[test] fn popup_clamped_to_screen() {
    let p=Rect{x:1900,y:1200,width:320,height:260};
    assert_eq!(constrain(p,Rect{x:0,y:0,width:1280,height:800},8),
        Rect{x:952,y:532,width:320,height:260});
  }
  #[test] fn oversized_popup_reduced() {
    let p=Rect{x:-30,y:-50,width:500,height:800};
    assert_eq!(constrain(p,Rect{x:10,y:20,width:280,height:160},10),
        Rect{x:20,y:30,width:260,height:140});
  }
  #[test] fn tiny_output_does_not_underflow() {
    let p=Rect{x:5,y:5,width:100,height:100};
    assert_eq!(constrain(p,Rect{x:0,y:0,width:2,height:2},30),
        Rect{x:1,y:1,width:0,height:0});
  }
}
