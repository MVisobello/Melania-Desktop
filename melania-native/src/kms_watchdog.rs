//! Bounded timeout accounting for pending DRM frames. No timer threads,
//! DRM IO, fabricated KMS acknowledgements, or host session access.
use crate::render_scheduler::FrameTicket;
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum WatchdogError {InvalidTimeout,InvalidEpoch,AlreadyPending,ClockOverflow}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct Pending {ticket:FrameTicket,deadline_ms:u64}
#[derive(Debug,Default)]
pub struct KmsFrameWatchdog {epoch:u64,pending:Option<Pending>}
impl KmsFrameWatchdog {
    pub fn epoch(&self)->u64 {self.epoch}
    pub fn reset(&mut self,epoch:u64) {
        self.epoch=epoch;
        self.pending=None;
    }
    pub fn arm(&mut self,ticket:FrameTicket,now_ms:u64,timeout_ms:u64)
        ->Result<(),WatchdogError>
    {
        if ticket.epoch!=self.epoch {return Err(WatchdogError::InvalidEpoch);}
        if self.pending.is_some() {return Err(WatchdogError::AlreadyPending);}
        if !(1..=30000).contains(&timeout_ms){return Err(WatchdogError::InvalidTimeout);}
        let deadline_ms=now_ms.checked_add(timeout_ms)
            .ok_or(WatchdogError::ClockOverflow)?;
        self.pending=Some(Pending{ticket,deadline_ms});
        Ok(())
    }
    pub fn expired(&self,now_ms:u64)->Option<FrameTicket> {
        self.pending.and_then(|p|(now_ms>=p.deadline_ms).then_some(p.ticket))
    }
    /// The caller independently validates the real CRTC FB_ID ACK.
    /// This watchdog only rejects stale, late and wrong tickets.
    pub fn acknowledged(&mut self,ticket:FrameTicket,now_ms:u64)->bool {
        match self.pending {
            Some(p) if p.ticket==ticket && now_ms<p.deadline_ms=>{
                self.pending=None;true
            },
            _=>false,
        }
    }
    pub fn cancel(&mut self,ticket:FrameTicket)->bool {
        if self.pending.is_some_and(|p|p.ticket==ticket) {
            self.pending=None;true
        }else{false}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn ticket(epoch:u64,sequence:u64)->FrameTicket {FrameTicket{epoch,sequence}}
    #[test]
    fn ACK_before_deadline_retires_only_matching_pending_ticket() {
        let mut w=KmsFrameWatchdog::default();
        w.reset(3);
        w.arm(ticket(3,1),1000,250).unwrap();
        assert_eq!(w.expired(1249),None);
        assert!(!w.acknowledged(ticket(3,2),1200));
        assert!(w.acknowledged(ticket(3,1),1249));
        assert!(!w.acknowledged(ticket(3,1),1249));
    }
    #[test]
    fn late_or_epoch_stale_ACK_never_reports_success() {
        let mut w=KmsFrameWatchdog::default();
        w.reset(4);
        assert_eq!(w.arm(ticket(3,1),0,250),Err(WatchdogError::InvalidEpoch));
        w.arm(ticket(4,1),100,20).unwrap();
        assert_eq!(w.expired(120),Some(ticket(4,1)));
        assert!(!w.acknowledged(ticket(4,1),121));
        w.reset(5);
        assert!(!w.cancel(ticket(4,1)));
        assert_eq!(w.expired(121),None);
        assert_eq!(w.arm(ticket(5,1),u64::MAX,20),Err(WatchdogError::ClockOverflow));
    }
}
