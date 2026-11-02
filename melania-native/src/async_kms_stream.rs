//! QEMU-only nonblocking Wayland-to-Rust-KMS transport.
//!
//! All potentially slow socket writes, sleeps and ACK reads live on a
//! dedicated worker thread. The Smithay event loop only uses try_send and
//! try_recv, never blocking on KMS or waiting for the virtual GPU.
//! Bounded at one queued frame; the separate scheduler permits at most one
//! frame in flight and ties ACKs to epoch/sequence tickets.
use crate::render_scheduler::FrameTicket;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    thread,
};

const MAX_IMAGE_BYTES: usize = 1280*800*4;

#[derive(Debug)]
pub struct FrameJob {
    pub ticket: FrameTicket,
    pub ordinal: u8,
    pub pixels: Vec<u8>,
}

#[derive(Debug)]
pub struct FrameResult {
    pub ticket: FrameTicket,
    pub ordinal: u8,
    pub status: Result<(), String>,
}

pub struct AsyncKmsWorker {
    sender: SyncSender<FrameJob>,
    receiver: Receiver<FrameResult>,
}

impl AsyncKmsWorker {
    pub fn spawn(mut stream: UnixStream) -> Self {
        let (tx, jobs)=mpsc::sync_channel::<FrameJob>(1);
        let (results, rx)=mpsc::sync_channel::<FrameResult>(1);
        thread::spawn(move || {
            while let Ok(job)=jobs.recv() {
                let result=(||->Result<(),String> {
                    if job.pixels.len()!=MAX_IMAGE_BYTES || !(1..=4).contains(&job.ordinal) {
                        return Err("invalid bounded QEMU frame dimensions or ordinal".into());
                    }
                    let size=u32::try_from(job.pixels.len())
                        .map_err(|e|e.to_string())?;
                    stream.write_all(&size.to_le_bytes())
                        .map_err(|e|e.to_string())?;
                    stream.write_all(&job.pixels)
                        .map_err(|e|e.to_string())?;
                    stream.flush().map_err(|e|e.to_string())?;
                    let mut ack=[0u8;1];
                    stream.read_exact(&mut ack).map_err(|e|e.to_string())?;
                    if ack[0]!=job.ordinal {
                        return Err("Rust KMS acknowledged a different frame".into());
                    }
                    Ok(())
                })();
                let failed=result.is_err();
                if results.send(FrameResult {
                    ticket:job.ticket, ordinal:job.ordinal, status:result,
                }).is_err() || failed {
                    // Close the stream on error; never retry a partly written
                    // frame without explicitly restarting the guest probe.
                    break;
                }
            }
        });
        Self { sender:tx, receiver:rx }
    }

    pub fn submit(&self, job:FrameJob) -> Result<(),TrySendError<FrameJob>> {
        self.sender.try_send(job)
    }

    pub fn try_result(&self) -> Result<Option<FrameResult>,TryRecvError> {
        match self.receiver.try_recv() {
            Ok(reply)=>Ok(Some(reply)),
            Err(TryRecvError::Empty)=>Ok(None),
            Err(error)=>Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn event_loop_can_poll_while_kms_has_not_acknowledged() {
        let (sender,mut consumer)=UnixStream::pair().unwrap();
        sender.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        consumer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let worker=AsyncKmsWorker::spawn(sender);
        let ticket=FrameTicket{epoch:7,sequence:11};
        let start=Instant::now();
        worker.submit(FrameJob{ticket,ordinal:1,pixels:vec![7;MAX_IMAGE_BYTES]}).unwrap();
        // Worker is blocked on a real socket ACK while the model's caller
        // can poll and service the Wayland event loop immediately.
        assert!(worker.try_result().unwrap().is_none());
        assert!(start.elapsed()<Duration::from_secs(2));
        let responder=thread::spawn(move || {
            let mut length=[0u8;4];
            consumer.read_exact(&mut length).unwrap();
            assert_eq!(u32::from_le_bytes(length) as usize,MAX_IMAGE_BYTES);
            let mut pixels=vec![0u8;MAX_IMAGE_BYTES];
            consumer.read_exact(&mut pixels).unwrap();
            assert_eq!(pixels[0],7);
            consumer.write_all(&[1]).unwrap();
        });
        responder.join().unwrap();
        let mut reply=None;
        for _ in 0..200 {
            if let Some(done)=worker.try_result().unwrap() {
                reply=Some(done);
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let done=reply.expect("real worker result after ACK");
        assert_eq!(done.ticket,ticket);
        assert_eq!(done.ordinal,1);
        assert!(done.status.is_ok());
    }

    #[test]
    fn mismatched_ack_cannot_be_reported_as_success() {
        let (sender,mut consumer)=UnixStream::pair().unwrap();
        sender.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let worker=AsyncKmsWorker::spawn(sender);
        let ticket=FrameTicket{epoch:1,sequence:1};
        worker.submit(FrameJob{ticket,ordinal:1,pixels:vec![0;MAX_IMAGE_BYTES]}).unwrap();
        let t=thread::spawn(move || {
            let mut length=[0u8;4];
            consumer.read_exact(&mut length).unwrap();
            let mut payload=vec![0;MAX_IMAGE_BYTES];
            consumer.read_exact(&mut payload).unwrap();
            consumer.write_all(&[2]).unwrap();
        });
        t.join().unwrap();
        let mut got=None;
        for _ in 0..200 {
            if let Some(event)=worker.try_result().unwrap() {
                got=Some(event);break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(got.unwrap().status.is_err());
    }

    #[test]
    fn invalid_frame_is_rejected_without_socket_transfer() {
        let (sender,_consumer)=UnixStream::pair().unwrap();
        let worker=AsyncKmsWorker::spawn(sender);
        worker.submit(FrameJob{
            ticket:FrameTicket{epoch:1,sequence:1},ordinal:1,pixels:vec![0;4],
        }).unwrap();
        let mut got=None;
        for _ in 0..200 {
            if let Some(event)=worker.try_result().unwrap() {
                got=Some(event);break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert!(got.unwrap().status.is_err());
    }
}
