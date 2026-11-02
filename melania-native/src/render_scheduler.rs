//! Bounded, fail-closed frame scheduling state for a future asynchronous
//! Wayland/DRM event loop. No DRM handles, host GPU access or native login.
//!
//! At most one presentation is in flight per output. New damage received while
//! a frame is outstanding is coalesced into a single successor. The producer
//! may deliver wl_surface.frame callbacks ONLY after complete_present() accepts
//! the exact submitted ticket. Pausing the seat or reprobeing an output revokes
//! every outstanding ticket, so a late KMS ACK cannot release stale buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameTicket {
    pub epoch: u64,
    pub sequence: u64,
}

#[derive(Debug, Default)]
pub struct RenderScheduler {
    seat_active: bool,
    epoch: u64,
    next_sequence: u64,
    dirty: bool,
    pending: Option<FrameTicket>,
    coalesced_updates: u64,
}

impl RenderScheduler {
    pub fn seat_active(&self) -> bool { self.seat_active }
    pub fn epoch(&self) -> u64 { self.epoch }
    pub fn pending(&self) -> Option<FrameTicket> { self.pending }
    pub fn needs_redraw(&self) -> bool { self.dirty }
    pub fn coalesced_updates(&self) -> u64 { self.coalesced_updates }

    /// Every transition revokes old tickets, even repeated activation.
    pub fn set_seat_active(&mut self, active: bool) {
        self.seat_active = active;
        self.invalidate();
    }

    /// Output removal, device hotplug or mode change invalidates old frames.
    pub fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        self.pending = None;
        self.dirty = false;
        self.coalesced_updates = 0;
    }

    /// Record a redraw request without allocating a new frame. While a frame
    /// is pending, requests collapse into one successor, never unbounded work.
    pub fn damage(&mut self) -> bool {
        if !self.seat_active {
            return false;
        }
        if self.dirty || self.pending.is_some() {
            self.coalesced_updates = self.coalesced_updates.saturating_add(1);
        }
        self.dirty = true;
        true
    }

    /// Take at most one dirty frame; do not start a successor until KMS has
    /// acknowledged (or failed) the current ticket.
    pub fn begin_frame(&mut self) -> Option<FrameTicket> {
        if !self.seat_active || !self.dirty || self.pending.is_some() {
            return None;
        }
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let ticket = FrameTicket {
            epoch: self.epoch, sequence: self.next_sequence,
        };
        self.pending = Some(ticket);
        self.dirty = false;
        Some(ticket)
    }

    /// Called ONLY after the verified CRTC FB_ID / presentation ACK. Return
    /// false for stale, duplicate or out-of-order events; never send a Wayland
    /// frame callback for rejected acknowledgements.
    pub fn complete_present(&mut self, ticket: FrameTicket) -> bool {
        if !self.seat_active || self.pending != Some(ticket) {
            return false;
        }
        self.pending = None;
        true
    }

    /// Failure never masquerades as a presented frame. Retry may be scheduled
    /// only on the same still-active generation; stale failures are ignored.
    pub fn fail_present(&mut self, ticket: FrameTicket) -> bool {
        if !self.seat_active || self.pending != Some(ticket) {
            return false;
        }
        self.pending = None;
        self.dirty = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_and_inactive_never_schedule() {
        let mut scheduler = RenderScheduler::default();
        assert!(!scheduler.damage());
        assert_eq!(scheduler.begin_frame(), None);
        scheduler.set_seat_active(true);
        assert_eq!(scheduler.begin_frame(), None);
    }

    #[test]
    fn coalesces_damage_while_a_frame_is_outstanding() {
        let mut scheduler = RenderScheduler::default();
        scheduler.set_seat_active(true);
        assert!(scheduler.damage());
        let first = scheduler.begin_frame().unwrap();
        assert_eq!(scheduler.begin_frame(), None);
        assert!(scheduler.damage());
        assert!(scheduler.damage());
        assert_eq!(scheduler.coalesced_updates(), 2);
        assert_eq!(scheduler.begin_frame(), None);
        assert!(scheduler.complete_present(first));
        let second = scheduler.begin_frame().unwrap();
        assert_ne!(first.sequence, second.sequence);
        assert!(!scheduler.needs_redraw());
        assert!(scheduler.complete_present(second));
        assert_eq!(scheduler.begin_frame(), None);
    }

    #[test]
    fn cannot_acknowledge_duplicate_wrong_or_old_generation() {
        let mut scheduler = RenderScheduler::default();
        scheduler.set_seat_active(true);
        scheduler.damage();
        let frame = scheduler.begin_frame().unwrap();
        let wrong = FrameTicket { sequence: frame.sequence + 1, ..frame };
        assert!(!scheduler.complete_present(wrong));
        assert_eq!(scheduler.pending(), Some(frame));
        assert!(scheduler.complete_present(frame));
        assert!(!scheduler.complete_present(frame));
        scheduler.damage();
        let stale = scheduler.begin_frame().unwrap();
        scheduler.set_seat_active(false);
        scheduler.set_seat_active(true);
        assert!(!scheduler.complete_present(stale));
        assert!(!scheduler.fail_present(stale));
        assert!(!scheduler.needs_redraw());
    }

    #[test]
    fn failed_presentation_requeues_but_never_completes() {
        let mut scheduler = RenderScheduler::default();
        scheduler.set_seat_active(true);
        scheduler.damage();
        let first = scheduler.begin_frame().unwrap();
        assert!(scheduler.fail_present(first));
        assert!(!scheduler.complete_present(first));
        assert!(scheduler.needs_redraw());
        let retry = scheduler.begin_frame().unwrap();
        assert_ne!(retry, first);
        assert!(scheduler.complete_present(retry));
    }

    #[test]
    fn hotplug_revokes_pending_and_drops_queued_damage() {
        let mut scheduler = RenderScheduler::default();
        scheduler.set_seat_active(true);
        scheduler.damage();
        let stale = scheduler.begin_frame().unwrap();
        scheduler.damage();
        let old_epoch = scheduler.epoch();
        scheduler.invalidate();
        assert_ne!(old_epoch, scheduler.epoch());
        assert!(!scheduler.complete_present(stale));
        assert_eq!(scheduler.begin_frame(), None);
        scheduler.damage();
        let final_ticket = scheduler.begin_frame().unwrap();
        assert!(scheduler.complete_present(final_ticket));
    }
}
