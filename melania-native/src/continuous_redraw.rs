//! Constant-memory scene redraw planner for a future continuous native renderer.
//!
//! A real renderer must not depend on QEMU-only frame ordinals (1..=6).
//! It needs a new ticket whenever the retained scene changes, while never
//! presenting more than one framebuffer at once. This module coordinates
//! scene generations with the existing RenderScheduler, without opening a
//! DRM device or claiming a functional native login session.
//!
//! The renderer must call presented() ONLY after a verified DRM/KMS ACK;
//! client frame callbacks and buffer.release must not be issued beforehand.
use crate::render_scheduler::{FrameTicket, RenderScheduler};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlannedSceneFrame {
    pub ticket: FrameTicket,
    pub generation: u64,
}

#[derive(Debug, Default)]
pub struct ContinuousRedraw {
    scheduler: RenderScheduler,
    observed_generation: Option<u64>,
    pending: Option<PlannedSceneFrame>,
    presented_generation: Option<u64>,
}

impl ContinuousRedraw {
    pub fn seat_active(&self) -> bool { self.scheduler.seat_active() }
    pub fn pending(&self) -> Option<PlannedSceneFrame> { self.pending }
    pub fn presented_generation(&self) -> Option<u64> { self.presented_generation }
    pub fn observed_generation(&self) -> Option<u64> { self.observed_generation }
    pub fn needs_redraw(&self) -> bool { self.scheduler.needs_redraw() }
    pub fn coalesced_updates(&self) -> u64 { self.scheduler.coalesced_updates() }

    /// A VT pause/resume revokes all outstanding tickets; resume must
    /// redraw the latest scene even if its generation has not changed.
    pub fn set_seat_active(&mut self, active: bool) {
        self.scheduler.set_seat_active(active);
        self.pending = None;
        self.presented_generation = None;
        if active && self.observed_generation.is_some() {
            self.scheduler.damage();
        }
    }

    /// Connector, mode or GPU changes invalidate the old CRTC's ACKs.
    pub fn invalidate_output(&mut self) {
        self.scheduler.invalidate();
        self.pending = None;
        self.presented_generation = None;
        if self.scheduler.seat_active() && self.observed_generation.is_some() {
            self.scheduler.damage();
        }
    }

    /// Scene generation comes from MultiWindowScene::layout_generation().
    /// Re-observing the same generation never generates redundant traffic.
    /// During an outstanding KMS frame, updates coalesce into one successor.
    pub fn scene_changed(&mut self, generation: u64) -> bool {
        if self.observed_generation == Some(generation) {
            return false;
        }
        self.observed_generation = Some(generation);
        self.scheduler.damage()
    }

    /// The caller composites the latest scene for this ticket, and submits
    /// it to a bounded non-blocking KMS worker. No allocation or I/O here.
    pub fn next_frame(&mut self) -> Option<PlannedSceneFrame> {
        let generation = self.observed_generation?;
        let ticket = self.scheduler.begin_frame()?;
        let plan = PlannedSceneFrame { ticket, generation };
        self.pending = Some(plan);
        Some(plan)
    }

    /// False for duplicate/wrong/stale tickets; never implies a frame
    /// callback unless the real presenter already confirmed CRTC FB_ID.
    pub fn presented(&mut self, frame: PlannedSceneFrame) -> bool {
        if self.pending != Some(frame) ||
           !self.scheduler.complete_present(frame.ticket) {
            return false;
        }
        self.pending = None;
        self.presented_generation = Some(frame.generation);
        // scene_changed() normally queued this already. This guard also
        // protects against a future caller mutating the tracked generation
        // between prepare/present without explicitly damaging it.
        if self.observed_generation != Some(frame.generation) &&
           !self.scheduler.needs_redraw() {
            self.scheduler.damage();
        }
        true
    }

    /// Requeue the latest scene on a failed flip, without reporting success.
    /// A stale error must not cancel a newer presentation.
    pub fn failed(&mut self, frame: PlannedSceneFrame) -> bool {
        if self.pending != Some(frame) || !self.scheduler.fail_present(frame.ticket) {
            return false;
        }
        self.pending = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_does_not_produce_frames_or_busy_loop() {
        let mut pump = ContinuousRedraw::default();
        assert_eq!(pump.next_frame(), None);
        assert!(!pump.scene_changed(1));
        assert_eq!(pump.next_frame(), None);
        pump.set_seat_active(true);
        let one = pump.next_frame().unwrap();
        assert_eq!(one.generation, 1);
        assert_eq!(pump.next_frame(), None);
        assert!(pump.presented(one));
        assert!(!pump.needs_redraw());
        assert_eq!(pump.next_frame(), None);
        assert!(!pump.scene_changed(1));
        assert_eq!(pump.next_frame(), None);
    }

    #[test]
    fn one_in_flight_and_many_mutations_collapse_to_newest() {
        let mut pump = ContinuousRedraw::default();
        pump.set_seat_active(true);
        assert!(pump.scene_changed(10));
        let first = pump.next_frame().unwrap();
        assert_eq!(first.generation, 10);
        for generation in 11..=100 {
            assert!(pump.scene_changed(generation));
            assert_eq!(pump.next_frame(), None);
        }
        assert!(pump.coalesced_updates()>=1);
        assert!(pump.presented(first));
        assert_eq!(pump.presented_generation(),Some(10));
        let successor=pump.next_frame().unwrap();
        assert_eq!(successor.generation,100);
        assert_ne!(first.ticket,successor.ticket);
        assert!(pump.presented(successor));
        assert_eq!(pump.presented_generation(),Some(100));
        assert_eq!(pump.next_frame(),None);
    }

    #[test]
    fn handles_more_than_six_frames_without_ordinal_limit() {
        let mut pump=ContinuousRedraw::default();
        pump.set_seat_active(true);
        for generation in 1..=64 {
            assert!(pump.scene_changed(generation));
            let frame=pump.next_frame().unwrap();
            assert_eq!(frame.generation,generation);
            assert_eq!(pump.next_frame(),None);
            assert!(pump.presented(frame));
        }
        assert_eq!(pump.presented_generation(),Some(64));
        assert!(!pump.needs_redraw());
    }

    #[test]
    fn pause_revokes_old_ack_and_resume_repaints_latest() {
        let mut pump=ContinuousRedraw::default();
        pump.set_seat_active(true);
        assert!(pump.scene_changed(9));
        let stale=pump.next_frame().unwrap();
        pump.set_seat_active(false);
        assert!(!pump.presented(stale));
        assert!(!pump.failed(stale));
        assert!(!pump.scene_changed(10));
        assert_eq!(pump.next_frame(),None);
        pump.set_seat_active(true);
        let restarted=pump.next_frame().unwrap();
        assert_eq!(restarted.generation,10);
        assert_ne!(stale.ticket.epoch,restarted.ticket.epoch);
        assert!(!pump.presented(stale));
        assert!(pump.presented(restarted));
    }

    #[test]
    fn output_hotplug_invalidates_inflight_without_stale_release() {
        let mut pump=ContinuousRedraw::default();
        pump.set_seat_active(true);
        assert!(pump.scene_changed(21));
        let stale=pump.next_frame().unwrap();
        pump.invalidate_output();
        assert!(!pump.presented(stale));
        assert!(!pump.failed(stale));
        let replacement=pump.next_frame().unwrap();
        assert_eq!(replacement.generation,21);
        assert_ne!(replacement.ticket.epoch,stale.ticket.epoch);
        assert!(pump.presented(replacement));
    }

    #[test]
    fn failed_flip_retries_latest_damage_and_rejects_wrong_tickets() {
        let mut pump=ContinuousRedraw::default();
        pump.set_seat_active(true);
        pump.scene_changed(1);
        let first=pump.next_frame().unwrap();
        pump.scene_changed(2);
        let wrong=PlannedSceneFrame{generation:2,..first};
        assert!(!pump.failed(wrong));
        assert!(!pump.presented(wrong));
        assert_eq!(pump.pending(),Some(first));
        assert!(pump.failed(first));
        assert!(!pump.presented(first));
        let retry=pump.next_frame().unwrap();
        assert_eq!(retry.generation,2);
        assert_ne!(retry.ticket,first.ticket);
        assert!(pump.presented(retry));
        assert!(!pump.presented(retry));
        assert_eq!(pump.next_frame(),None);
    }
}
