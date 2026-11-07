//! Atomic configure/buffer acceptance for native COSMIC tab-group resize.
//!
//! Policy only: Smithay must supply *actual* XDG configure serial/ACK and
//! independent, validated, owned wl_shm buffers. The compositor may not move,
//! unhide or present a resized stack until *every* tab is ready.
//! No wayland calls, DRM operations, timers, or synthetic acknowledgements.
#![forbid(unsafe_code)]

use crate::tab_tiling_plan::StackTileSlot;
use crate::window_interaction::WindowRect;

const MAX_TRANSACTION_CLIENTS:usize=32;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum ResizeError {
    AlreadyPending,
    EmptyBatch,
    InvalidSlot,
    DuplicateWindow,
    TooManyWindows,
    EpochExhausted,
    WrongEpoch,
    UnknownWindow,
    MissingConfigure,
    SerialMismatch,
    BufferBeforeAck,
    BufferSizeMismatch,
    NotReady,
}

#[derive(Clone,Debug,PartialEq,Eq)]
struct ClientReceipt {
    id:u64,
    /// Nonzero XDG configure serial from the actual Wayland server.
    configure:Option<u32>,
    acked:bool,
    committed:bool,
    target:(usize,usize),
}

#[derive(Clone,Debug,PartialEq,Eq)]
struct Pending {
    epoch:u64,
    slots:Vec<StackTileSlot>,
    clients:Vec<ClientReceipt>,
}

/// One pending *output-wide* transaction: no individual group becomes
/// visible before every role in the complete new layout has committed.
#[derive(Clone,Debug,Default)]
pub struct TabResizeTransaction {
    next_epoch:u64,
    pending:Option<Pending>,
}

impl TabResizeTransaction {
    pub fn has_pending(&self)->bool {self.pending.is_some()}
    pub fn epoch(&self)->Option<u64> {
        self.pending.as_ref().map(|batch|batch.epoch)
    }
    pub fn begin(
        &mut self,slots:&[StackTileSlot],output_width:i32,output_height:i32
    )->Result<u64,ResizeError> {
        if self.pending.is_some(){return Err(ResizeError::AlreadyPending);}
        if slots.is_empty(){return Err(ResizeError::EmptyBatch);}
        let epoch=self.next_epoch.checked_add(1)
            .ok_or(ResizeError::EpochExhausted)?;
        let mut clients=Vec::new();
        for slot in slots {
            if slot.active==0 || slot.members.is_empty() ||
                !slot.members.contains(&slot.active) ||
                !slot.rect.valid_for(output_width,output_height) {
                return Err(ResizeError::InvalidSlot);
            }
            let target=(slot.rect.width as usize,slot.rect.height as usize);
            for &id in &slot.members {
                if id==0 {return Err(ResizeError::InvalidSlot);}
                if clients.iter().any(|c:&ClientReceipt|c.id==id){
                    return Err(ResizeError::DuplicateWindow);
                }
                if clients.len()>=MAX_TRANSACTION_CLIENTS {
                    return Err(ResizeError::TooManyWindows);
                }
                clients.push(ClientReceipt{
                    id,configure:None,acked:false,committed:false,target
                });
            }
        }
        // All preconditions pass: publish the generation exactly once.
        self.next_epoch=epoch;
        self.pending=Some(Pending{epoch,slots:slots.to_vec(),clients});
        Ok(epoch)
    }
    fn client_mut(&mut self,epoch:u64,id:u64)
        ->Result<&mut ClientReceipt,ResizeError> {
        let batch=self.pending.as_mut().ok_or(ResizeError::WrongEpoch)?;
        if batch.epoch!=epoch{return Err(ResizeError::WrongEpoch);}
        batch.clients.iter_mut().find(|c|c.id==id)
            .ok_or(ResizeError::UnknownWindow)
    }
    /// Called ONLY when Smithay has actually issued this client's configure.
    /// A repeated configure for the same client invalidates its prior ACK
    /// and its previously committed pixels.
    pub fn sent_configure(&mut self,epoch:u64,id:u64,serial:u32)
        ->Result<(),ResizeError> {
        if serial==0 {return Err(ResizeError::SerialMismatch);}
        let client=self.client_mut(epoch,id)?;
        client.configure=Some(serial);
        client.acked=false;
        client.committed=false;
        Ok(())
    }
    /// Must be invoked from the actual XDG ack_configure event, never from
    /// a requested geometry or a client frame callback.
    pub fn ack_configure(&mut self,epoch:u64,id:u64,serial:u32)
        ->Result<(),ResizeError> {
        let client=self.client_mut(epoch,id)?;
        if client.configure.is_none(){return Err(ResizeError::MissingConfigure);}
        if client.configure!=Some(serial){return Err(ResizeError::SerialMismatch);}
        client.acked=true;
        Ok(())
    }
    /// Only a fresh *validated owned wl_shm* commit after the ACK counts.
    /// A stale-size buffer is rejected without marking progress.
    pub fn owned_buffer_committed(
        &mut self,epoch:u64,id:u64,width:usize,height:usize
    )->Result<bool,ResizeError> {
        let client=self.client_mut(epoch,id)?;
        if !client.acked{return Err(ResizeError::BufferBeforeAck);}
        if client.target!=(width,height) {
            return Err(ResizeError::BufferSizeMismatch);
        }
        client.committed=true;
        Ok(self.is_ready(epoch))
    }
    pub fn is_ready(&self,epoch:u64)->bool {
        self.pending.as_ref().is_some_and(|batch|
            batch.epoch==epoch && batch.clients.iter().all(|c|
                c.configure.is_some() && c.acked && c.committed))
    }
    /// Ownership passes to the bridge only when the *entire* batch is ready.
    /// An early call leaves the transaction intact and cannot reveal pixels.
    pub fn finish(&mut self,epoch:u64)
        ->Result<Vec<StackTileSlot>,ResizeError> {
        if self.epoch()!=Some(epoch){return Err(ResizeError::WrongEpoch);}
        if !self.is_ready(epoch){return Err(ResizeError::NotReady);}
        let batch=self.pending.take().expect("ready transaction is present");
        Ok(batch.slots)
    }
    /// Client disconnect, VT/libseat revocation, output removal or configure
    /// timeout must cancel the entire plan, not apply any partial geometry.
    pub fn abort(&mut self,epoch:u64)->bool {
        if self.epoch()!=Some(epoch){return false;}
        self.pending=None;
        true
    }
    pub fn abort_if_member_destroyed(&mut self,id:u64)->bool {
        let matches=self.pending.as_ref().is_some_and(|b|
            b.clients.iter().any(|client|client.id==id));
        if matches{self.pending=None;}
        matches
    }
    pub fn revoke(&mut self) {
        self.pending=None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn slot(active:u64,members:&[u64],x:i32)->StackTileSlot {
        StackTileSlot{
            active,members:members.to_vec(),
            rect:WindowRect{x,y:0,width:400,height:600},
        }
    }
    fn plan()->Vec<StackTileSlot>{
        vec![slot(2,&[1,2],0),slot(3,&[3],408)]
    }
    #[test]
    fn requires_every_real_ack_and_matching_owned_buffer_before_commit(){
        let mut m=TabResizeTransaction::default();
        let epoch=m.begin(&plan(),1280,800).unwrap();
        assert!(!m.is_ready(epoch));
        assert_eq!(m.finish(epoch),Err(ResizeError::NotReady));
        for id in 1..=3 {
            m.sent_configure(epoch,id,100+id as u32).unwrap();
            assert_eq!(m.owned_buffer_committed(epoch,id,400,600),
                       Err(ResizeError::BufferBeforeAck));
            m.ack_configure(epoch,id,100+id as u32).unwrap();
            assert_eq!(m.owned_buffer_committed(epoch,id,400,600).unwrap(),
                       id==3);
        }
        assert!(m.is_ready(epoch));
        assert_eq!(m.finish(epoch).unwrap(),plan());
        assert!(!m.has_pending());
    }
    #[test]
    fn wrong_serial_dimensions_and_old_epoch_cannot_complete(){
        let mut m=TabResizeTransaction::default();
        let epoch=m.begin(&plan(),1280,800).unwrap();
        assert_eq!(m.sent_configure(epoch,1,0),Err(ResizeError::SerialMismatch));
        m.sent_configure(epoch,1,77).unwrap();
        assert_eq!(m.ack_configure(epoch,1,76),Err(ResizeError::SerialMismatch));
        assert_eq!(m.owned_buffer_committed(epoch,1,400,600),
                   Err(ResizeError::BufferBeforeAck));
        m.ack_configure(epoch,1,77).unwrap();
        assert_eq!(m.owned_buffer_committed(epoch,1,399,600),
                   Err(ResizeError::BufferSizeMismatch));
        assert_eq!(m.sent_configure(epoch+1,1,99),Err(ResizeError::WrongEpoch));
        assert!(!m.is_ready(epoch));
    }
    #[test]
    fn second_configure_revokes_prior_client_ack_and_buffer(){
        let mut m=TabResizeTransaction::default();
        let epoch=m.begin(&plan(),1280,800).unwrap();
        for id in 1..=3 {
            m.sent_configure(epoch,id,id as u32).unwrap();
            m.ack_configure(epoch,id,id as u32).unwrap();
            m.owned_buffer_committed(epoch,id,400,600).unwrap();
        }
        assert!(m.is_ready(epoch));
        m.sent_configure(epoch,2,55).unwrap();
        assert!(!m.is_ready(epoch));
        assert_eq!(m.finish(epoch),Err(ResizeError::NotReady));
        m.ack_configure(epoch,2,55).unwrap();
        assert!(m.owned_buffer_committed(epoch,2,400,600).unwrap());
    }
    #[test]
    fn malformed_plan_never_mutates_or_advances_epoch(){
        let mut m=TabResizeTransaction::default();
        assert_eq!(m.begin(&[],1280,800),Err(ResizeError::EmptyBatch));
        assert_eq!(m.begin(&[slot(1,&[1,1],0)],1280,800),
                   Err(ResizeError::DuplicateWindow));
        assert_eq!(m.begin(&[slot(9,&[1,2],0)],1280,800),
                   Err(ResizeError::InvalidSlot));
        assert_eq!(m.begin(&[slot(1,&[1],1200)],1280,800),
                   Err(ResizeError::InvalidSlot));
        assert_eq!(m.begin(&[slot(1,&[1],0),slot(2,&[1,2],408)],1280,800),
                   Err(ResizeError::DuplicateWindow));
        assert_eq!(m.epoch(),None);
        assert_eq!(m.begin(&plan(),1280,800),Ok(1));
        assert_eq!(m.begin(&plan(),1280,800),Err(ResizeError::AlreadyPending));
        assert_eq!(m.epoch(),Some(1));
    }
    #[test]
    fn destroyed_tab_and_vt_revoke_fail_closed_without_partial_layout(){
        let mut m=TabResizeTransaction::default();
        let epoch=m.begin(&plan(),1280,800).unwrap();
        m.sent_configure(epoch,1,22).unwrap();
        m.ack_configure(epoch,1,22).unwrap();
        m.owned_buffer_committed(epoch,1,400,600).unwrap();
        assert!(!m.abort_if_member_destroyed(90));
        assert!(m.abort_if_member_destroyed(2));
        assert_eq!(m.finish(epoch),Err(ResizeError::WrongEpoch));
        let next=m.begin(&plan(),1280,800).unwrap();
        assert_ne!(next,epoch);
        assert!(!m.abort(epoch));
        m.revoke();
        assert!(!m.has_pending());
        assert!(!m.is_ready(next));
    }
    #[test]
    fn member_capacity_is_bounded_and_fails_atomically(){
        let mut m=TabResizeTransaction::default();
        let big=slot(1,&(1..=33).collect::<Vec<_>>(),0);
        assert_eq!(m.begin(&[big],1280,800),Err(ResizeError::TooManyWindows));
        assert_eq!(m.epoch(),None);
    }
}
