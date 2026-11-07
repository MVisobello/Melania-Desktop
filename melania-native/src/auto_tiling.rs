//! Native automatic tiling *policy* for one workspace.
//!
//! A future Wayland bridge owns one instance per workspace/output. This module
//! deliberately does not send XDG configure, swap a framebuffer or claim KMS
//! presentation: an accepted plan must still be applied to real clients.
//! Floating and minimized windows remain registered but never consume a tile.
#![forbid(unsafe_code)]

use crate::window_interaction::WindowRect;

const MIN_W: i32 = 96;
const MIN_H: i32 = 72;
pub const MAX_TILE_WINDOWS: usize = 32;
pub const MAX_TILE_GAP: i32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileAxis {
    /// Split left/right first, then top/bottom on the next level.
    Vertical,
    /// Split top/bottom first, then left/right on the next level.
    Horizontal,
}
impl TileAxis {
    fn other(self) -> Self {
        match self { Self::Vertical => Self::Horizontal, Self::Horizontal => Self::Vertical }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileError {
    InvalidWindow,
    DuplicateWindow,
    TooManyWindows,
    UnknownWindow,
    InvalidGap,
    InvalidWorkArea,
    CannotFit,
}

/// Physical direction in logical output coordinates. This policy never
/// changes client geometry until the Wayland bridge issues XDG configure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileDirection { Left, Right, Up, Down }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TilePlacement {
    pub window_id: u64,
    pub rect: WindowRect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Member {
    id: u64,
    floating: bool,
    minimized: bool,
    // Role exclusion must not erase an application's user floating choice.
    role_excluded: bool,
}

/// State belongs to *one* workspace. Instances must not be shared across
/// outputs without explicit policy from the compositor's output manager.
#[derive(Clone, Debug)]
pub struct AutoTiling {
    enabled: bool,
    axis: TileAxis,
    gap: i32,
    members: Vec<Member>,
}

impl Default for AutoTiling {
    fn default() -> Self {
        Self {
            enabled: false,
            axis: TileAxis::Vertical,
            gap: 8,
            members: Vec::new(),
        }
    }
}

impl AutoTiling {
    pub fn enabled(&self) -> bool { self.enabled }
    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }
    pub fn axis(&self) -> TileAxis { self.axis }
    pub fn set_axis(&mut self, axis: TileAxis) { self.axis = axis; }
    pub fn gap(&self) -> i32 { self.gap }
    pub fn set_gap(&mut self, gap: i32) -> Result<(), TileError> {
        if !(0..=MAX_TILE_GAP).contains(&gap) {
            return Err(TileError::InvalidGap);
        }
        self.gap = gap;
        Ok(())
    }
    pub fn register(&mut self, id: u64) -> Result<(), TileError> {
        if id == 0 { return Err(TileError::InvalidWindow); }
        if self.members.iter().any(|w| w.id == id) {
            return Err(TileError::DuplicateWindow);
        }
        if self.members.len() >= MAX_TILE_WINDOWS {
            return Err(TileError::TooManyWindows);
        }
        self.members.push(Member { id, floating: false, minimized: false, role_excluded: false });
        Ok(())
    }
    pub fn forget(&mut self, id: u64) -> bool {
        let before = self.members.len();
        self.members.retain(|w| w.id != id);
        before != self.members.len()
    }
    pub fn set_floating(&mut self, id: u64, floating: bool) -> Result<(), TileError> {
        let w = self.members.iter_mut().find(|w| w.id == id).ok_or(TileError::UnknownWindow)?;
        w.floating = floating;
        Ok(())
    }
    pub fn is_floating(&self, id: u64) -> Result<bool, TileError> {
        self.members.iter().find(|w| w.id == id)
            .map(|w| w.floating).ok_or(TileError::UnknownWindow)
    }
    pub fn set_role_excluded(&mut self, id: u64, excluded: bool) -> Result<(), TileError> {
        let w = self.members.iter_mut().find(|w| w.id == id).ok_or(TileError::UnknownWindow)?;
        w.role_excluded = excluded;
        Ok(())
    }
    pub fn set_minimized(&mut self, id: u64, minimized: bool) -> Result<(), TileError> {
        let w = self.members.iter_mut().find(|w| w.id == id).ok_or(TileError::UnknownWindow)?;
        w.minimized = minimized;
        Ok(())
    }
    pub fn swap(&mut self, a: u64, b: u64) -> Result<(), TileError> {
        let ia = self.members.iter().position(|w| w.id == a).ok_or(TileError::UnknownWindow)?;
        let ib = self.members.iter().position(|w| w.id == b).ok_or(TileError::UnknownWindow)?;
        self.members.swap(ia, ib);
        Ok(())
    }
    /// COSMIC Super+X + direction: swap the focused *eligible* tile with
    /// the nearest tile in the requested direction. Never swap a floating,
    /// minimized or XDG-role-excluded window. If the direction has no
    /// neighbor, the layout is untouched. All decisions use a complete,
    /// validated current work-area plan (not stale cached rectangles).
    /// The compositor remains responsible for configure, buffer ACK,
    /// focus and KMS presentation; this is only an ordering transaction.
    pub fn swap_direction(
        &mut self, focused: u64, direction: TileDirection,
        work: WindowRect, output_width: i32, output_height: i32,
    ) -> Result<Option<u64>, TileError> {
        if !self.members.iter().any(|member| member.id==focused) {
            return Err(TileError::UnknownWindow);
        }
        let Some(plan)=self.plan(work,output_width,output_height)? else {
            return Ok(None);
        };
        let Some(from)=plan.iter().find(|entry|entry.window_id==focused) else {
            return Ok(None);
        };
        let mut best: Option<((u8,i64,i64,usize),u64)> = None;
        for (index,entry) in plan.iter().enumerate() {
            if entry.window_id==focused {continue;}
            let Some((alignment,distance,offset))=
                directional_score(from.rect,entry.rect,direction) else {continue;};
            let score=(alignment,distance,offset,index);
            if best.as_ref().is_none_or(|(old,_)|score<*old) {
                best=Some((score,entry.window_id));
            }
        }
        let Some((_,target))=best else{return Ok(None);};
        self.swap(focused,target)?;
        Ok(Some(target))
    }
    pub fn member_count(&self) -> usize { self.members.len() }

    /// Return None when automatic tiling is off; never change stored floating
    /// geometry in that case. A failed layout returns an error *without* a
    /// partially applicable placement list.
    ///
    /// Output bounds are checked in logical coordinates. Exclude layer-shell
    /// exclusive zones by passing the compositor's current usable work area.
    pub fn plan(
        &self, work: WindowRect, output_width: i32, output_height: i32,
    ) -> Result<Option<Vec<TilePlacement>>, TileError> {
        if !self.enabled { return Ok(None); }
        if output_width > 16_384 || output_height > 16_384 ||
           !work.valid_for(output_width, output_height) {
            return Err(TileError::InvalidWorkArea);
        }
        let ids: Vec<u64> = self.members.iter()
            .filter(|w| !w.floating && !w.minimized && !w.role_excluded)
            .map(|w| w.id).collect();
        let mut result = Vec::with_capacity(ids.len());
        if !layout(&ids, work, self.axis, self.gap, &mut result) {
            return Err(TileError::CannotFit);
        }
        Ok(Some(result))
    }
}

/// Prefer neighbors whose orthogonal projections overlap (real row/column
/// adjacency); among these, prefer the nearest center in the selected
/// direction. Doubled integer centers preserve half-pixel positions exactly.
fn directional_score(
    from: WindowRect, to: WindowRect, direction: TileDirection,
) -> Option<(u8,i64,i64)> {
    let fx=i64::from(from.x)*2+i64::from(from.width);
    let fy=i64::from(from.y)*2+i64::from(from.height);
    let tx=i64::from(to.x)*2+i64::from(to.width);
    let ty=i64::from(to.y)*2+i64::from(to.height);
    let (forward,lateral,overlaps)=match direction {
        TileDirection::Left=>(
            fx-tx,(fy-ty).abs(),
            from.y < to.y+to.height && to.y < from.y+from.height,
        ),
        TileDirection::Right=>(
            tx-fx,(fy-ty).abs(),
            from.y < to.y+to.height && to.y < from.y+from.height,
        ),
        TileDirection::Up=>(
            fy-ty,(fx-tx).abs(),
            from.x < to.x+to.width && to.x < from.x+from.width,
        ),
        TileDirection::Down=>(
            ty-fy,(fx-tx).abs(),
            from.x < to.x+to.width && to.x < from.x+from.width,
        ),
    };
    (forward>0).then_some((u8::from(!overlaps),forward,lateral))
}

/// Recursive balanced partitions. Alternate axes to avoid a single strip,
/// and try the other axis if minimum client dimensions prevent the first.
/// Shared integer cut-points assign all remainder pixels; only intentional
/// gaps are excluded, so odd dimensions do not create accidental seams.
fn layout(ids: &[u64], rect: WindowRect, axis: TileAxis, gap: i32,
          result: &mut Vec<TilePlacement>) -> bool {
    if ids.is_empty() { return true; }
    if rect.width < MIN_W || rect.height < MIN_H ||
       i64::from(rect.width) * i64::from(rect.height) <
       ids.len() as i64 * i64::from(MIN_W) * i64::from(MIN_H) {
        return false;
    }
    if ids.len() == 1 {
        result.push(TilePlacement { window_id: ids[0], rect });
        return true;
    }
    let count = ids.len();
    let left_count = (count + 1) / 2;
    for choice in [axis, axis.other()] {
        let span = if choice == TileAxis::Vertical { rect.width } else { rect.height };
        let available = span - gap;
        if available <= 0 { continue; }
        let first = ((i64::from(available) * left_count as i64) / count as i64) as i32;
        let second = available - first;
        let (a, b) = match choice {
            TileAxis::Vertical => (
                WindowRect { width: first, ..rect },
                WindowRect { x: rect.x + first + gap, width: second, ..rect },
            ),
            TileAxis::Horizontal => (
                WindowRect { height: first, ..rect },
                WindowRect { y: rect.y + first + gap, height: second, ..rect },
            ),
        };
        let original_len = result.len();
        if layout(&ids[..left_count], a, choice.other(), gap, result) &&
           layout(&ids[left_count..], b, choice.other(), gap, result) {
            return true;
        }
        result.truncate(original_len);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work() -> WindowRect {
        WindowRect { x: 80, y: 36, width: 1201, height: 803 }
    }
    fn tiler(n: u64) -> AutoTiling {
        let mut t = AutoTiling::default();
        t.set_enabled(true);
        for id in 1..=n { t.register(id).unwrap(); }
        t
    }
    fn no_overlap(a: WindowRect, b: WindowRect) -> bool {
        a.x + a.width <= b.x || b.x + b.width <= a.x ||
        a.y + a.height <= b.y || b.y + b.height <= a.y
    }

    #[test]
    fn disabled_is_explicit_and_non_destructive() {
        let mut t = AutoTiling::default();
        t.register(1).unwrap();
        assert_eq!(t.plan(work(), 1920, 1080), Ok(None));
        assert_eq!(t.member_count(), 1);
    }
    #[test]
    fn tiling_three_on_odd_area_has_no_seams_when_gap_zero() {
        let mut t = tiler(3);
        t.set_gap(0).unwrap();
        let p = t.plan(work(), 1920, 1080).unwrap().unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!(p.iter().map(|p| p.rect.width as i64 * p.rect.height as i64).sum::<i64>(),
                   1201 * 803);
        for (i, a) in p.iter().enumerate() {
            assert!(a.rect.valid_for(1920, 1080));
            for b in p.iter().skip(i + 1) { assert!(no_overlap(a.rect, b.rect)); }
        }
    }
    #[test]
    fn respects_panel_reserved_area_and_gaps() {
        let t = tiler(5);
        let p = t.plan(work(), 1920, 1080).unwrap().unwrap();
        assert_eq!(p.len(), 5);
        for (i, a) in p.iter().enumerate() {
            assert!(a.rect.x >= 80 && a.rect.y >= 36);
            assert!(a.rect.x + a.rect.width <= 1281);
            assert!(a.rect.y + a.rect.height <= 839);
            for b in p.iter().skip(i + 1) { assert!(no_overlap(a.rect, b.rect)); }
        }
    }
    #[test]
    fn floating_and_minimized_are_excluded_without_forgetting() {
        let mut t = tiler(4);
        t.set_floating(2, true).unwrap();
        t.set_minimized(3, true).unwrap();
        assert_eq!(t.plan(work(), 1920, 1080).unwrap().unwrap()
                      .iter().map(|p| p.window_id).collect::<Vec<_>>(), vec![1,4]);
        assert_eq!(t.member_count(), 4);
        t.set_minimized(3, false).unwrap();
        assert_eq!(t.plan(work(), 1920, 1080).unwrap().unwrap().len(), 3);
    }
    #[test]
    fn role_exclusion_preserves_user_floating_across_fullscreen_and_restore() {
        let mut t=tiler(3);
        t.set_floating(1,true).unwrap();
        t.set_role_excluded(1,true).unwrap();
        t.set_role_excluded(2,true).unwrap();
        assert_eq!(t.plan(work(),1920,1080).unwrap().unwrap().len(),1);
        t.set_role_excluded(1,false).unwrap();
        t.set_role_excluded(2,false).unwrap();
        assert!(t.is_floating(1).unwrap());
        assert!(!t.is_floating(2).unwrap());
        let ids=t.plan(work(),1920,1080).unwrap().unwrap()
            .iter().map(|p|p.window_id).collect::<Vec<_>>();
        assert_eq!(ids,vec![2,3]);
    }
    #[test]
    fn swap_changes_order_without_duplicating_windows() {
        let mut t = tiler(2);
        t.swap(1, 2).unwrap();
        let ids: Vec<_> = t.plan(work(), 1920, 1080).unwrap().unwrap()
            .iter().map(|p| p.window_id).collect();
        assert_eq!(ids, vec![2, 1]);
        assert_eq!(t.swap(1, 99), Err(TileError::UnknownWindow));
        assert_eq!(t.member_count(), 2);
    }
    #[test]
    fn directional_swap_prefers_aligned_neighbors_not_diagonal() {
        let mut t=tiler(4);
        let before=t.plan(work(),1920,1080).unwrap().unwrap();
        // Alternating split: upper-left 1, lower-left 2,
        // upper-right 3, lower-right 4.
        assert_eq!(before[0].window_id,1);
        assert_eq!(t.swap_direction(1,TileDirection::Up,work(),1920,1080),
                   Ok(None));
        assert_eq!(t.swap_direction(1,TileDirection::Right,work(),1920,1080),
                   Ok(Some(3)));
        let after=t.plan(work(),1920,1080).unwrap().unwrap();
        assert_eq!(after[0].window_id,3);
        assert_eq!(after[2].window_id,1);
        assert_eq!(t.swap_direction(1,TileDirection::Down,work(),1920,1080),
                   Ok(Some(4)));
    }
    #[test]
    fn directional_swap_ignores_excluded_tiles_and_invalid_actions() {
        let mut t=tiler(4);
        t.set_floating(3,true).unwrap();
        t.set_minimized(4,true).unwrap();
        assert_eq!(t.swap_direction(3,TileDirection::Right,work(),1920,1080),
                   Ok(None));
        assert_eq!(t.swap_direction(4,TileDirection::Left,work(),1920,1080),
                   Ok(None));
        assert_eq!(t.swap_direction(1,TileDirection::Right,work(),1920,1080),
                   Ok(Some(2)));
        assert_eq!(t.swap_direction(99,TileDirection::Right,work(),1920,1080),
                   Err(TileError::UnknownWindow));
        t.set_enabled(false);
        assert_eq!(t.swap_direction(1,TileDirection::Left,work(),1920,1080),
                   Ok(None));
    }
    #[test]
    fn directional_swap_preserves_order_when_layout_cannot_fit() {
        let mut t=tiler(8);
        let before=t.members.iter().map(|m|m.id).collect::<Vec<_>>();
        assert_eq!(t.swap_direction(1,TileDirection::Right,
                   WindowRect{x:0,y:0,width:200,height:160},200,160),
                   Err(TileError::CannotFit));
        assert_eq!(t.members.iter().map(|m|m.id).collect::<Vec<_>>(),before);
    }
    #[test]
    fn too_many_windows_bad_ids_and_invalid_gaps_are_rejected() {
        let mut t = tiler(MAX_TILE_WINDOWS as u64);
        assert_eq!(t.register(0), Err(TileError::InvalidWindow));
        assert_eq!(t.register(1), Err(TileError::DuplicateWindow));
        assert_eq!(t.register(100), Err(TileError::TooManyWindows));
        assert_eq!(t.set_gap(33), Err(TileError::InvalidGap));
        assert_eq!(t.gap(), 8);
    }
    #[test]
    fn impossible_layout_fails_atomically_and_recovers() {
        let mut t = tiler(8);
        assert_eq!(t.plan(WindowRect{x:0,y:0,width:200,height:160},200,160),
                   Err(TileError::CannotFit));
        t.set_floating(1, true).unwrap();
        assert!(t.plan(work(), 1920, 1080).unwrap().is_some());
        assert!(t.forget(8));
        assert!(!t.forget(8));
    }
    #[test]
    fn horizontal_orientation_and_bounded_invalid_area() {
        let mut t = tiler(2);
        t.set_axis(TileAxis::Horizontal);
        assert_eq!(t.axis(), TileAxis::Horizontal);
        let p = t.plan(work(), 1920, 1080).unwrap().unwrap();
        assert!(p[0].rect.y < p[1].rect.y);
        assert_eq!(t.plan(work(), 1000, 720), Err(TileError::InvalidWorkArea));
        assert_eq!(t.plan(work(), 16_385, 1080), Err(TileError::InvalidWorkArea));
    }
}
