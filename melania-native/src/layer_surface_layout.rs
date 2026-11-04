//! Bounded work-area and anchor policy for future native wlr-layer-shell.
//! This pure model does not import real layer buffers or submit KMS frames.
use crate::scene::Layer;

pub const MAX_LAYER_SIDE:u32=8192;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ShellLayer {Background,Bottom,Top,Overlay}
impl ShellLayer {
    pub fn scene_layer(self)->Layer {
        match self {
            Self::Background=>Layer::Background,
            Self::Bottom=>Layer::Bottom,
            Self::Top=>Layer::Top,
            Self::Overlay=>Layer::Overlay,
        }
    }
}
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct Anchors {pub top:bool,pub bottom:bool,pub left:bool,pub right:bool}
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct Insets {pub top:i32,pub right:i32,pub bottom:i32,pub left:i32}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Edge {Top,Bottom,Left,Right}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct LayerRequest {
    pub layer:ShellLayer,pub anchors:Anchors,
    /// Zero requests stretching between opposite anchored edges.
    pub width:u32,pub height:u32,pub margins:Insets,pub exclusive_zone:i32,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Rect {pub x:i32,pub y:i32,pub width:u32,pub height:u32}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Reservation {pub edge:Edge,pub pixels:u32}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct LayerPlacement {
    pub rect:Rect,pub layer:ShellLayer,pub reservation:Option<Reservation>,
}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum LayoutError {
    InvalidOutput,InvalidDimensions,InvalidAnchor,InvalidMargins,
    NoRoom,AmbiguousExclusiveEdge,
}

fn axis(output:i64,length:u32,start:bool,end:bool,
        start_margin:i32,end_margin:i32)->Result<(i32,u32),LayoutError>
{
    if length==0 && !(start && end) {return Err(LayoutError::InvalidAnchor);}
    let span=if length==0 {
        output-i64::from(start_margin)-i64::from(end_margin)
    } else {i64::from(length)};
    if span<1 || span>i64::from(MAX_LAYER_SIDE) {return Err(LayoutError::NoRoom);}
    let pos=match (start,end) {
        (true,false)=>i64::from(start_margin),
        (false,true)=>output-i64::from(end_margin)-span,
        (true,true) if length==0=>i64::from(start_margin),
        (true,true)=>i64::from(start_margin)+
             (output-i64::from(start_margin)-i64::from(end_margin)-span)/2,
        (false,false)=>(output-span)/2,
    };
    let coordinate=i32::try_from(pos).map_err(|_|LayoutError::InvalidDimensions)?;
    Ok((coordinate,span as u32))
}
fn exclusive_edge(a:Anchors)->Option<Edge> {
    // Opposing anchors give no unique edge to exclude from normal work areas.
    if a.top && !a.bottom {Some(Edge::Top)}
    else if a.bottom && !a.top {Some(Edge::Bottom)}
    else if a.left && !a.right {Some(Edge::Left)}
    else if a.right && !a.left {Some(Edge::Right)}
    else {None}
}
/// Compute a safe layer placement on a single output. This does not alter
/// client-owned pixels or claim a native panel has rendered.
pub fn place(r:LayerRequest,output_width:u32,output_height:u32)
    ->Result<LayerPlacement,LayoutError>
{
    if output_width==0 || output_height==0 ||
       output_width>MAX_LAYER_SIDE || output_height>MAX_LAYER_SIDE {
        return Err(LayoutError::InvalidOutput);
    }
    if r.width>MAX_LAYER_SIDE || r.height>MAX_LAYER_SIDE {
        return Err(LayoutError::InvalidDimensions);
    }
    if [r.margins.top,r.margins.right,r.margins.bottom,r.margins.left]
        .iter().any(|m|i64::from(*m).abs()>i64::from(MAX_LAYER_SIDE)) {
        return Err(LayoutError::InvalidMargins);
    }
    let (x,width)=axis(i64::from(output_width),r.width,
        r.anchors.left,r.anchors.right,r.margins.left,r.margins.right)?;
    let (y,height)=axis(i64::from(output_height),r.height,
        r.anchors.top,r.anchors.bottom,r.margins.top,r.margins.bottom)?;
    let reservation=if r.exclusive_zone>0 {
        let edge=exclusive_edge(r.anchors).ok_or(LayoutError::AmbiguousExclusiveEdge)?;
        let margin=match edge {
            Edge::Top=>r.margins.top, Edge::Bottom=>r.margins.bottom,
            Edge::Left=>r.margins.left, Edge::Right=>r.margins.right,
        };
        let px=(i64::from(r.exclusive_zone)+i64::from(margin))
            .max(0).min(i64::from(MAX_LAYER_SIDE)) as u32;
        Some(Reservation{edge,pixels:px})
    } else {None};
    Ok(LayerPlacement{rect:Rect{x,y,width,height},layer:r.layer,reservation})
}
/// Space available for normal toplevels, not the rendered output bounds.
/// Summed zones are saturated, so malicious panels cannot underflow width.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct WorkAreaReservations {
    pub top:u32,pub bottom:u32,pub left:u32,pub right:u32,
}
impl WorkAreaReservations {
    pub fn add(&mut self,r:Option<Reservation>) {
        let Some(r)=r else {return;};
        let target=match r.edge {
            Edge::Top=>&mut self.top,Edge::Bottom=>&mut self.bottom,
            Edge::Left=>&mut self.left,Edge::Right=>&mut self.right,
        };
        *target=target.saturating_add(r.pixels);
    }
    pub fn normal_area(self,width:u32,height:u32)->Result<Rect,LayoutError> {
        if width==0 || height==0 || width>MAX_LAYER_SIDE || height>MAX_LAYER_SIDE {
            return Err(LayoutError::InvalidOutput);
        }
        let left=self.left.min(width);
        let top=self.top.min(height);
        let free_width=width.saturating_sub(left).saturating_sub(self.right);
        let free_height=height.saturating_sub(top).saturating_sub(self.bottom);
        if free_width==0 || free_height==0 {return Err(LayoutError::NoRoom);}
        Ok(Rect{x:left as i32,y:top as i32,width:free_width,height:free_height})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{compose,Pixels,Surface};

    fn panel()->LayerRequest {
        LayerRequest{layer:ShellLayer::Top,
            anchors:Anchors{top:true,left:true,right:true,bottom:false},
            width:0,height:40,margins:Insets::default(),exclusive_zone:40}
    }
    #[test]
    fn top_panel_stretches_and_reserves_work_area() {
        let p=place(panel(),1280,800).unwrap();
        assert_eq!(p.rect,Rect{x:0,y:0,width:1280,height:40});
        assert_eq!(p.layer.scene_layer(),Layer::Top);
        let mut work=WorkAreaReservations::default();
        work.add(p.reservation);
        assert_eq!(work.normal_area(1280,800),
                   Ok(Rect{x:0,y:40,width:1280,height:760}));
    }
    #[test]
    fn gtk_taskbar_52px_stretches_and_reserves_bottom_work_area() {
        let request=LayerRequest {
            layer:ShellLayer::Top,
            anchors:Anchors{bottom:true,left:true,right:true,top:false},
            width:0,height:52,margins:Insets::default(),exclusive_zone:52,
        };
        let placed=place(request,1280,800).unwrap();
        assert_eq!(placed.rect,Rect{x:0,y:748,width:1280,height:52});
        assert_eq!(placed.reservation,
                   Some(Reservation{edge:Edge::Bottom,pixels:52}));
        let mut work=WorkAreaReservations::default();
        work.add(placed.reservation);
        assert_eq!(work.normal_area(1280,800),
                   Ok(Rect{x:0,y:0,width:1280,height:748}));
    }
    #[test]
    fn real_gtk_shell_five_surfaces_fit_virtual_output_without_stealing_work_area() {
        // Values come from melania-panel/src/main.rs make_surface calls.
        // This tests placement only; no GTK process or Wayland buffer is used.
        let cases=[
            ("taskbar",ShellLayer::Top,Anchors{bottom:true,left:true,right:true,top:false},
             0,52,0,0,52,Rect{x:0,y:748,width:1280,height:52}),
            ("start",ShellLayer::Overlay,Anchors{left:true,bottom:true,top:false,right:false},
             510,565,8,6,0,Rect{x:8,y:229,width:510,height:565}),
            ("quick",ShellLayer::Overlay,Anchors{right:true,bottom:true,top:false,left:false},
             350,430,0,65,12,Rect{x:930,y:305,width:350,height:430}),
            ("notifications",ShellLayer::Overlay,Anchors{right:true,bottom:true,top:false,left:false},
             380,440,0,65,12,Rect{x:900,y:295,width:380,height:440}),
            ("calendar",ShellLayer::Overlay,Anchors{right:true,bottom:true,top:false,left:false},
             330,315,0,58,12,Rect{x:950,y:427,width:330,height:315}),
        ];
        let mut work=WorkAreaReservations::default();
        for (name,layer,anchors,width,height,left,bottom,zone,expected) in cases {
            let request=LayerRequest {
                layer,anchors,width,height,
                margins:Insets{left,bottom,..Insets::default()},
                exclusive_zone:zone,
            };
            let placed=place(request,1280,800).unwrap();
            assert_eq!(placed.rect,expected,"{name} geometry changed");
            if name=="taskbar" {
                work.add(placed.reservation);
            } else {
                assert_eq!(placed.reservation,None,"{name} must not reserve workspace");
            }
        }
        assert_eq!(work.normal_area(1280,800),
                   Ok(Rect{x:0,y:0,width:1280,height:748}));
    }
    #[test]
    fn bottom_taskbar_margins_and_exclusive_zone() {
        let mut r=panel();
        r.anchors=Anchors{bottom:true,left:true,right:true,top:false};
        r.margins.bottom=4;
        r.exclusive_zone=42;
        let p=place(r,1280,800).unwrap();
        assert_eq!(p.rect,Rect{x:0,y:756,width:1280,height:40});
        assert_eq!(p.reservation,
                   Some(Reservation{edge:Edge::Bottom,pixels:46}));
    }
    #[test]
    fn overlay_centers_and_does_not_reserve() {
        let r=LayerRequest{layer:ShellLayer::Overlay,
            anchors:Anchors::default(),width:400,height:200,
            margins:Insets::default(),exclusive_zone:-1};
        let p=place(r,1280,800).unwrap();
        assert_eq!(p.rect,Rect{x:440,y:300,width:400,height:200});
        assert_eq!(p.reservation,None);
    }
    #[test]
    fn dock_vertical_stretch_and_wallpaper_full_output() {
        let r=LayerRequest{layer:ShellLayer::Bottom,
            anchors:Anchors{left:true,top:true,bottom:true,right:false},
            width:64,height:0,margins:Insets::default(),exclusive_zone:64};
        assert_eq!(place(r,800,600).unwrap().rect,
                   Rect{x:0,y:0,width:64,height:600});
        let mut wall=LayerRequest{layer:ShellLayer::Background,
            anchors:Anchors{top:true,bottom:true,left:true,right:true},
            width:0,height:0,margins:Insets::default(),exclusive_zone:0};
        assert_eq!(place(wall,800,600).unwrap().rect,
                   Rect{x:0,y:0,width:800,height:600});
        wall.exclusive_zone=12;
        assert_eq!(place(wall,800,600),Err(LayoutError::AmbiguousExclusiveEdge));
    }
    #[test]
    fn compositor_scene_layers_put_panel_above_normal_windows() {
        let p=place(panel(),16,12).unwrap();
        let layers=[
            Surface{x:0,y:0,width:16,height:12,layer:Layer::Normal,
                pixels:Pixels::Solid([40,50,60,0])},
            Surface{x:p.rect.x,y:p.rect.y,
                width:p.rect.width as usize,
                height:p.rect.height as usize,
                layer:p.layer.scene_layer(),pixels:Pixels::Solid([80,90,100,0])},
        ];
        let img=compose(16,12,[1,2,3,0],&layers).unwrap();
        assert_eq!(&img[..3],&[80,90,100]);
    }
    #[test]
    fn rejects_invalid_stretch_dimensions_and_margins() {
        let mut r=panel();
        assert_eq!(place(r,0,800),Err(LayoutError::InvalidOutput));
        assert_eq!(place(r,1280,9000),Err(LayoutError::InvalidOutput));
        r.width=9000;
        assert_eq!(place(r,1280,800),Err(LayoutError::InvalidDimensions));
        r.width=0;
        r.anchors.right=false;
        assert_eq!(place(r,1280,800),Err(LayoutError::InvalidAnchor));
        r.anchors.right=true;
        r.margins.left=8193;
        assert_eq!(place(r,1280,800),Err(LayoutError::InvalidMargins));
        r.margins.left=8000;
        assert_eq!(place(r,1280,800),Err(LayoutError::NoRoom));
    }
    #[test]
    fn multiple_panels_saturate_without_negative_work_area() {
        let mut w=WorkAreaReservations::default();
        w.add(Some(Reservation{edge:Edge::Top,pixels:24}));
        w.add(Some(Reservation{edge:Edge::Bottom,pixels:42}));
        w.add(Some(Reservation{edge:Edge::Left,pixels:8}));
        assert_eq!(w.normal_area(1280,800),
                   Ok(Rect{x:8,y:24,width:1272,height:734}));
        w.add(Some(Reservation{edge:Edge::Top,pixels:9000}));
        assert_eq!(w.normal_area(1280,800),Err(LayoutError::NoRoom));
    }
}
