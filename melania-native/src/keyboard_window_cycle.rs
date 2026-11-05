//! Native Alt+Tab focus policy. Only actual libinput events can call this
//! through the active Smithay/libseat bridge; this cannot enable native login.
const LEFT_ALT:u32=56;
const RIGHT_ALT:u32=100;
const TAB:u32=15;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ShortcutResult {Forward, Consume, Focus(u64)}

#[derive(Default,Debug)]
pub struct KeyboardWindowCycle {
    left_alt:bool,
    right_alt:bool,
    tab_held:bool,
    // Freeze focus order while Alt is held, even after compositor raises
    // the selected window: cycling three windows must not toggle two only.
    order:Vec<u64>,
    selected:Option<u64>,
}
impl KeyboardWindowCycle {
    fn alt_active(&self)->bool {self.left_alt || self.right_alt}
    pub fn reset(&mut self) {*self=Self::default();}
    pub fn cancel_cycle(&mut self) {
        self.order.clear();
        self.selected=None;
    }
    /// Code is the Linux evdev key code, after Smithay XKB's +8 offset.
    /// Caller must require an active libseat/device generation.
    pub fn event(
        &mut self,code:Option<u32>,pressed:bool,
        stack_top_first:&[u64],focused:Option<u64>,
    )->ShortcutResult {
        match code {
            Some(LEFT_ALT)=>{
                self.left_alt=pressed;
                if !self.alt_active(){self.cancel_cycle();}
                ShortcutResult::Forward
            },
            Some(RIGHT_ALT)=>{
                self.right_alt=pressed;
                if !self.alt_active(){self.cancel_cycle();}
                ShortcutResult::Forward
            },
            Some(TAB)=>{
                if !pressed {
                    let consumed=self.tab_held;
                    self.tab_held=false;
                    return if consumed {ShortcutResult::Consume}
                           else {ShortcutResult::Forward};
                }
                if !self.alt_active() {return ShortcutResult::Forward;}
                if self.tab_held {return ShortcutResult::Consume;}
                self.tab_held=true;
                // Keep MRU snapshot, but drop destroyed/minimized windows.
                if self.order.is_empty() {
                    self.order=stack_top_first.iter().copied()
                        .filter(|id|*id!=0).collect();
                    if let Some(i)=focused.and_then(|id|
                        self.order.iter().position(|key|*key==id)) {
                        self.order.rotate_left(i);
                    }
                    self.selected=focused.filter(|id|self.order.contains(id));
                } else {
                    self.order.retain(|id|stack_top_first.contains(id));
                }
                if self.order.len()<2 {return ShortcutResult::Consume;}
                let next=if let Some(i)=self.selected
                    .and_then(|id|self.order.iter().position(|key|*key==id))
                {
                    (i+1)%self.order.len()
                } else if let Some(i)=focused
                    .and_then(|id|self.order.iter().position(|key|*key==id))
                {
                    (i+1)%self.order.len()
                } else {0};
                let id=self.order[next];
                self.selected=Some(id);
                ShortcutResult::Focus(id)
            },
            _=>ShortcutResult::Forward,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(s:&mut KeyboardWindowCycle,code:u32,down:bool,
           order:&[u64],focused:Option<u64>)->ShortcutResult {
        s.event(Some(code),down,order,focused)
    }
    #[test]
    fn alt_tab_three_windows_uses_snapshot_not_most_recent_raise() {
        let mut m=KeyboardWindowCycle::default();
        assert_eq!(key(&mut m,LEFT_ALT,true,&[3,2,1],Some(3)),ShortcutResult::Forward);
        assert_eq!(key(&mut m,TAB,true,&[3,2,1],Some(3)),ShortcutResult::Focus(2));
        assert_eq!(key(&mut m,TAB,true,&[2,3,1],Some(2)),ShortcutResult::Consume);
        assert_eq!(key(&mut m,TAB,false,&[2,3,1],Some(2)),ShortcutResult::Consume);
        assert_eq!(key(&mut m,TAB,true,&[2,3,1],Some(2)),ShortcutResult::Focus(1));
        assert_eq!(key(&mut m,TAB,false,&[1,2,3],Some(1)),ShortcutResult::Consume);
        assert_eq!(key(&mut m,TAB,true,&[1,2,3],Some(1)),ShortcutResult::Focus(3));
        assert_eq!(key(&mut m,LEFT_ALT,false,&[3,1,2],Some(3)),ShortcutResult::Forward);
        assert_eq!(key(&mut m,TAB,false,&[3,1,2],Some(3)),ShortcutResult::Consume);
        assert_eq!(key(&mut m,LEFT_ALT,true,&[3,1,2],Some(3)),ShortcutResult::Forward);
        assert_eq!(key(&mut m,TAB,true,&[3,1,2],Some(3)),ShortcutResult::Focus(1));
    }
    #[test]
    fn right_alt_and_both_alt_release_only_when_both_released() {
        let mut m=KeyboardWindowCycle::default();
        key(&mut m,RIGHT_ALT,true,&[2,1],Some(2));
        key(&mut m,LEFT_ALT,true,&[2,1],Some(2));
        assert_eq!(key(&mut m,TAB,true,&[2,1],Some(2)),ShortcutResult::Focus(1));
        key(&mut m,TAB,false,&[1,2],Some(1));
        key(&mut m,LEFT_ALT,false,&[1,2],Some(1));
        assert_eq!(key(&mut m,TAB,true,&[1,2],Some(1)),ShortcutResult::Focus(2));
        key(&mut m,RIGHT_ALT,false,&[2,1],Some(2));
        assert_eq!(key(&mut m,TAB,false,&[2,1],Some(2)),ShortcutResult::Consume);
        assert_eq!(key(&mut m,TAB,true,&[2,1],Some(2)),ShortcutResult::Forward);
    }
    #[test]
    fn closed_or_minimized_window_cannot_receive_focus() {
        let mut m=KeyboardWindowCycle::default();
        key(&mut m,LEFT_ALT,true,&[3,2,1],Some(3));
        assert_eq!(key(&mut m,TAB,true,&[3,2,1],Some(3)),ShortcutResult::Focus(2));
        key(&mut m,TAB,false,&[2,3,1],Some(2));
        assert_eq!(key(&mut m,TAB,true,&[3],Some(3)),ShortcutResult::Consume);
        key(&mut m,TAB,false,&[3],Some(3));
        m.cancel_cycle();
        assert_eq!(key(&mut m,TAB,true,&[5,3],Some(3)),ShortcutResult::Focus(5));
    }
    #[test]
    fn ordinary_tab_duplicate_press_and_seat_reset_do_not_switch() {
        let mut m=KeyboardWindowCycle::default();
        assert_eq!(key(&mut m,TAB,true,&[2,1],Some(2)),ShortcutResult::Forward);
        assert_eq!(key(&mut m,TAB,false,&[2,1],Some(2)),ShortcutResult::Forward);
        key(&mut m,LEFT_ALT,true,&[2,1],Some(2));
        assert_eq!(key(&mut m,TAB,true,&[2,1],Some(2)),ShortcutResult::Focus(1));
        assert_eq!(key(&mut m,TAB,true,&[2,1],Some(1)),ShortcutResult::Consume);
        m.reset();
        assert_eq!(key(&mut m,TAB,true,&[1,2],Some(1)),ShortcutResult::Forward);
        assert_eq!(key(&mut m,TAB,false,&[1,2],Some(1)),ShortcutResult::Forward);
        assert_eq!(m.event(None,true,&[1,2],Some(1)),ShortcutResult::Forward);
    }
}
