//! Output-local pointer routing and deterministic focus transitions.
//! Does not synthesize input events or access the host libinput seat.
use crate::output_layout::OutputLayout;

#[derive(Clone,Copy,Debug,PartialEq)]
pub struct RoutedPointer {
    pub output_id:u32,
    pub local_x:f64,
    pub local_y:f64,
}
#[derive(Clone,Copy,Debug,PartialEq)]
pub struct PointerTransition {
    pub previous_output:Option<u32>,
    pub current:Option<RoutedPointer>,
    pub output_changed:bool,
}
#[derive(Debug,Default)]
pub struct OutputPointerRouter {
    output:Option<u32>,
}
impl OutputPointerRouter {
    pub fn current_output(&self)->Option<u32>{self.output}
    /// Re-resolve when output layout or pointer coordinates change.
    /// Pointer entering a disconnected screen clears its old focus.
    pub fn update(&mut self,layout:&OutputLayout,x:f64,y:f64)->PointerTransition {
        let previous=self.output;
        let current=layout.route(x,y).map(|(id,local_x,local_y)|
            RoutedPointer{output_id:id,local_x,local_y});
        self.output=current.map(|r|r.output_id);
        PointerTransition {
            previous_output:previous,current,
            output_changed:previous!=self.output,
        }
    }
    pub fn revoke(&mut self)->Option<u32> {self.output.take()}
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::output_layout::DisplayRect;
    #[test]
    fn crossing_two_displays_yields_exact_local_coordinates() {
        let mut layout=OutputLayout::default();
        layout.set(DisplayRect{id:1,x:-640,y:0,width:640,height:480}).unwrap();
        layout.set(DisplayRect{id:2,x:0,y:0,width:1280,height:800}).unwrap();
        let mut pointer=OutputPointerRouter::default();
        let left=pointer.update(&layout,-1.0,23.0);
        assert_eq!(left.current,Some(RoutedPointer{output_id:1,local_x:639.0,local_y:23.0}));
        assert!(left.output_changed);
        let right=pointer.update(&layout,0.0,23.0);
        assert_eq!(right.previous_output,Some(1));
        assert_eq!(right.current,Some(RoutedPointer{output_id:2,local_x:0.0,local_y:23.0}));
        assert!(right.output_changed);
        assert!(!pointer.update(&layout,1.0,24.0).output_changed);
    }
    #[test]
    fn unplug_and_invalid_pointer_coordinates_clear_focus() {
        let mut layout=OutputLayout::default();
        layout.set(DisplayRect{id:1,x:0,y:0,width:400,height:300}).unwrap();
        let mut pointer=OutputPointerRouter::default();
        pointer.update(&layout,10.0,10.0);
        layout.remove(1);
        let exit=pointer.update(&layout,10.0,10.0);
        assert_eq!(exit.previous_output,Some(1));
        assert_eq!(exit.current,None);
        assert_eq!(pointer.update(&layout,f64::NAN,10.0).current,None);
        assert_eq!(pointer.revoke(),None);
    }
}
