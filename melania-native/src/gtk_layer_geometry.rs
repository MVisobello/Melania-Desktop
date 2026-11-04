//! Bounded, VM-only configure policy for the five real GTK4 Melania
//! layer-shell namespaces. Does not start GTK or enable native login.
use crate::layer_surface_layout::{
    self, Anchors, Insets, LayerPlacement, LayerRequest, ShellLayer,
};

pub const MAX_GTK_LAYER_SURFACES:usize=5;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct GtkLayerSpec {
    pub namespace:&'static str,
    pub request:LayerRequest,
    pub configured_width:u32,
    pub configured_height:u32,
}
impl GtkLayerSpec {
    pub fn placement(self)->LayerPlacement {
        // These five designs fit the fixed disposable QEMU 1280x800 output.
        layer_surface_layout::place(self.request,1280,800)
            .expect("fixed bounded GTK panel layout")
    }
}
fn anchors(top:bool,bottom:bool,left:bool,right:bool)->Anchors {
    Anchors{top,bottom,left,right}
}
pub fn spec(namespace:&str)->Option<GtkLayerSpec> {
    let (request,configured_width,configured_height)=match namespace {
        "melania-panel"=>(
            LayerRequest{layer:ShellLayer::Top,
                anchors:anchors(false,true,true,true),
                width:0,height:52,margins:Insets::default(),exclusive_zone:52},
            1280,52),
        "melania-start"=>(
            LayerRequest{layer:ShellLayer::Overlay,
                anchors:anchors(false,true,true,false),
                width:510,height:565,
                margins:Insets{left:8,bottom:6,..Insets::default()},exclusive_zone:0},
            510,565),
        "melania-quick"=>(
            LayerRequest{layer:ShellLayer::Overlay,
                anchors:anchors(false,true,false,true),
                width:350,height:430,
                margins:Insets{right:12,bottom:65,..Insets::default()},exclusive_zone:0},
            350,430),
        "melania-notifications"=>(
            LayerRequest{layer:ShellLayer::Overlay,
                anchors:anchors(false,true,false,true),
                width:380,height:440,
                margins:Insets{right:12,bottom:65,..Insets::default()},exclusive_zone:0},
            380,440),
        "melania-calendar"=>(
            LayerRequest{layer:ShellLayer::Overlay,
                anchors:anchors(false,true,false,true),
                width:330,height:315,
                margins:Insets{right:12,bottom:58,..Insets::default()},exclusive_zone:0},
            330,315),
        _=>return None,
    };
    Some(GtkLayerSpec{namespace:match namespace{
        "melania-panel"=>"melania-panel",
        "melania-start"=>"melania-start",
        "melania-quick"=>"melania-quick",
        "melania-notifications"=>"melania-notifications",
        "melania-calendar"=>"melania-calendar",
        _=>unreachable!(),
    },request,configured_width,configured_height})
}

/// Fail closed on unknown, duplicate, excessive or incorrectly layered roles.
/// The legacy QEMU layer-shell path remains unchanged unless explicitly opted in.
pub fn permit<'a>(
    namespace:&str,
    requested_layer:ShellLayer,
    active:impl IntoIterator<Item=&'a str>,
)->Option<GtkLayerSpec> {
    let expected=spec(namespace)?;
    if expected.request.layer!=requested_layer {return None;}
    let mut occupied=0usize;
    for name in active {
        if name==namespace{return None;}
        occupied=occupied.saturating_add(1);
        if occupied>=MAX_GTK_LAYER_SURFACES {return None;}
    }
    Some(expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer_surface_layout::{Rect,WorkAreaReservations,Reservation,Edge};

    #[test]
    fn all_five_real_gtk_namespaces_get_independent_configures() {
        let rows=[
            ("melania-panel",ShellLayer::Top,(1280,52),
             Rect{x:0,y:748,width:1280,height:52}),
            ("melania-start",ShellLayer::Overlay,(510,565),
             Rect{x:8,y:229,width:510,height:565}),
            ("melania-quick",ShellLayer::Overlay,(350,430),
             Rect{x:918,y:305,width:350,height:430}),
            ("melania-notifications",ShellLayer::Overlay,(380,440),
             Rect{x:888,y:295,width:380,height:440}),
            ("melania-calendar",ShellLayer::Overlay,(330,315),
             Rect{x:938,y:427,width:330,height:315}),
        ];
        let mut occupied=Vec::<&str>::new();
        let mut reserved=WorkAreaReservations::default();
        for (name,layer,configured,rect) in rows {
            let found=permit(name,layer,occupied.iter().copied()).unwrap();
            assert_eq!((found.configured_width,found.configured_height),configured);
            let placement=found.placement();
            assert_eq!(placement.rect,rect);
            if name=="melania-panel" {
                assert_eq!(placement.reservation,
                           Some(Reservation{edge:Edge::Bottom,pixels:52}));
            } else {
                assert_eq!(placement.reservation,None);
            }
            reserved.add(placement.reservation);
            occupied.push(name);
        }
        assert_eq!(reserved.normal_area(1280,800),
                   Ok(Rect{x:0,y:0,width:1280,height:748}));
        assert!(permit("melania-panel",ShellLayer::Top,occupied.iter().copied()).is_none());
    }

    #[test]
    fn rejects_duplicates_unknown_roles_wrong_layer_and_capacity() {
        assert!(permit("melania-panel",ShellLayer::Overlay,std::iter::empty()).is_none());
        assert!(permit("melania-start",ShellLayer::Top,std::iter::empty()).is_none());
        assert!(permit("another-client",ShellLayer::Top,std::iter::empty()).is_none());
        assert!(permit("melania-panel",ShellLayer::Top,["melania-panel"]).is_none());
        assert!(permit("melania-calendar",ShellLayer::Overlay,
            ["a","b","c","d","e"]).is_none());
        assert!(permit("melania-calendar",ShellLayer::Overlay,
            ["a","b","c","d"]).is_some());
    }
}
