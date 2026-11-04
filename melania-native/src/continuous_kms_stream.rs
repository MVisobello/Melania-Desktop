//! Versioned, QEMU-only transport for a future generation-driven KMS loop.
//!
//! This module is deliberately NOT connected to the live native bridge yet.
//! The existing six-frame DRM/KMS proof remains the only active VM transport.
//! The receiver MUST send its ACK only after a verified DRM/KMS presentation;
//! passing the in-memory socket tests below does not prove hardware scanout.
//!
//! Wire frame: "MLF1" + epoch:u64 + sequence:u64 + generation:u64 +
//!             payload_bytes:u32 + exactly 1280*800*4 BGRA bytes.
//! Wire ACK:   "MLA1" + epoch:u64 + sequence:u64 + generation:u64 +
//!             status:u8 (0 means verified presentation).
//! All integers are little-endian. Fixed-size headers avoid framing ambiguity.
use crate::continuous_redraw::PlannedSceneFrame;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    net::Shutdown,
    sync::mpsc::{self, Receiver, SyncSender, TryRecvError, TrySendError},
    thread,
};

const IMAGE_BYTES: usize = 1280 * 800 * 4;
const HEADER_BYTES: usize = 4 + 8 + 8 + 8 + 4;
const ACK_BYTES: usize = 4 + 8 + 8 + 8 + 1;

#[derive(Debug)]
pub struct ContinuousKmsJob {
    pub planned: PlannedSceneFrame,
    pub pixels: Vec<u8>,
}

#[derive(Debug)]
pub struct ContinuousKmsResult {
    pub planned: PlannedSceneFrame,
    pub status: Result<(), String>,
}

/// Single bounded queued job, single worker, and no blocking operations on
/// the Smithay dispatch thread. The caller's ContinuousRedraw ledger enforces
/// at most one actual presentation in flight.
pub struct ContinuousKmsWorker {
    sender: Option<SyncSender<ContinuousKmsJob>>,
    receiver: Receiver<ContinuousKmsResult>,
    // A second handle lets shutdown interrupt a blocked read_exact() on
    // VT loss or compositor error without waiting for a socket timeout.
    shutdown: UnixStream,
    worker: Option<thread::JoinHandle<()>>,
}

fn frame_header(planned: PlannedSceneFrame) -> [u8; HEADER_BYTES] {
    let mut bytes = [0u8; HEADER_BYTES];
    bytes[..4].copy_from_slice(b"MLF1");
    bytes[4..12].copy_from_slice(&planned.ticket.epoch.to_le_bytes());
    bytes[12..20].copy_from_slice(&planned.ticket.sequence.to_le_bytes());
    bytes[20..28].copy_from_slice(&planned.generation.to_le_bytes());
    bytes[28..32].copy_from_slice(&(IMAGE_BYTES as u32).to_le_bytes());
    bytes
}

fn verified_ack(ack: &[u8; ACK_BYTES], planned: PlannedSceneFrame) -> Result<(), String> {
    if &ack[..4] != b"MLA1" {
        return Err("invalid continuous KMS ACK protocol marker".into());
    }
    let epoch = u64::from_le_bytes(ack[4..12].try_into().unwrap());
    let sequence = u64::from_le_bytes(ack[12..20].try_into().unwrap());
    let generation = u64::from_le_bytes(ack[20..28].try_into().unwrap());
    if epoch != planned.ticket.epoch ||
       sequence != planned.ticket.sequence ||
       generation != planned.generation {
        return Err("stale, reordered, or mismatched continuous KMS ACK".into());
    }
    if ack[28] != 0 {
        return Err("continuous KMS peer did not confirm presentation".into());
    }
    Ok(())
}

fn transfer(stream: &mut UnixStream, job: &ContinuousKmsJob) -> Result<(), String> {
    if job.pixels.len() != IMAGE_BYTES {
        return Err("invalid continuous KMS framebuffer dimensions".into());
    }
    stream.write_all(&frame_header(job.planned)).map_err(|e| e.to_string())?;
    stream.write_all(&job.pixels).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;
    let mut ack = [0u8; ACK_BYTES];
    stream.read_exact(&mut ack).map_err(|e| e.to_string())?;
    verified_ack(&ack, job.planned)
}

impl ContinuousKmsWorker {
    pub fn spawn(mut stream: UnixStream) -> std::io::Result<Self> {
        // If duplicating the FD fails, do NOT start a worker that cannot be
        // interrupted safely during libseat/VT cleanup.
        let shutdown = stream.try_clone()?;
        let (sender, jobs) = mpsc::sync_channel::<ContinuousKmsJob>(1);
        let (results, receiver) = mpsc::sync_channel::<ContinuousKmsResult>(1);
        let worker = thread::spawn(move || {
            while let Ok(job) = jobs.recv() {
                let status = transfer(&mut stream, &job);
                let failed = status.is_err();
                // Never block the worker while handing results to a stalled
                // compositor: a full result channel is a fatal desync.
                if results.try_send(ContinuousKmsResult { planned: job.planned, status }).is_err()
                    || failed {
                    // A partially transferred frame or bad ACK poisons this
                    // stream. Never reuse it or invent a successful callback.
                    break;
                }
            }
        });
        Ok(Self { sender: Some(sender), receiver, shutdown, worker: Some(worker) })
    }

    pub fn submit(&self, job: ContinuousKmsJob)
        -> Result<(), TrySendError<ContinuousKmsJob>> {
        match &self.sender {
            Some(sender) => sender.try_send(job),
            None => Err(TrySendError::Disconnected(job)),
        }
    }

    /// Revoke queued jobs, abort any blocked socket I/O and join the writer
    /// before the owner releases the libseat DRM descriptor. Idempotent.
    pub fn stop(&mut self) {
        self.sender.take();
        let _ = self.shutdown.shutdown(Shutdown::Both);
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                eprintln!("MELANIA_CONTINUOUS_KMS_WORKER_PANICKED=true");
            }
        }
    }

    pub fn try_result(&self) -> Result<Option<ContinuousKmsResult>, TryRecvError> {
        match self.receiver.try_recv() {
            Ok(result) => Ok(Some(result)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

impl Drop for ContinuousKmsWorker {
    fn drop(&mut self) { self.stop(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render_scheduler::FrameTicket;
    use std::time::{Duration, Instant};

    fn plan(epoch: u64, sequence: u64, generation: u64) -> PlannedSceneFrame {
        PlannedSceneFrame { ticket: FrameTicket { epoch, sequence }, generation }
    }

    fn ack(frame: PlannedSceneFrame, status: u8) -> [u8; ACK_BYTES] {
        let mut data = [0u8; ACK_BYTES];
        data[..4].copy_from_slice(b"MLA1");
        data[4..12].copy_from_slice(&frame.ticket.epoch.to_le_bytes());
        data[12..20].copy_from_slice(&frame.ticket.sequence.to_le_bytes());
        data[20..28].copy_from_slice(&frame.generation.to_le_bytes());
        data[28] = status;
        data
    }

    fn result_for(worker: &ContinuousKmsWorker) -> ContinuousKmsResult {
        for _ in 0..300 {
            if let Some(result) = worker.try_result().expect("worker connection") {
                return result;
            }
            thread::sleep(Duration::from_millis(5));
        }
        panic!("bounded KMS worker did not report peer response");
    }

    fn consume(peer: &mut UnixStream) -> PlannedSceneFrame {
        let mut header = [0u8; HEADER_BYTES];
        peer.read_exact(&mut header).unwrap();
        assert_eq!(&header[..4], b"MLF1");
        assert_eq!(u32::from_le_bytes(header[28..32].try_into().unwrap()) as usize,
                   IMAGE_BYTES);
        let mut pixels = vec![0u8; IMAGE_BYTES];
        peer.read_exact(&mut pixels).unwrap();
        PlannedSceneFrame {
            ticket: FrameTicket {
                epoch: u64::from_le_bytes(header[4..12].try_into().unwrap()),
                sequence: u64::from_le_bytes(header[12..20].try_into().unwrap()),
            },
            generation: u64::from_le_bytes(header[20..28].try_into().unwrap()),
        }
    }

    #[test]
    fn ten_consecutive_generations_without_six_frame_ceiling() {
        let (client, mut peer) = UnixStream::pair().unwrap();
        client.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
        let worker = ContinuousKmsWorker::spawn(client).unwrap();
        let responder = thread::spawn(move || {
            for number in 1..=10 {
                let frame = consume(&mut peer);
                assert_eq!(frame, plan(3, number, number + 100));
                peer.write_all(&ack(frame, 0)).unwrap();
            }
        });
        for number in 1..=10 {
            let frame = plan(3, number, number + 100);
            worker.submit(ContinuousKmsJob {
                planned: frame, pixels: vec![number as u8; IMAGE_BYTES],
            }).unwrap();
            let result = result_for(&worker);
            assert_eq!(result.planned, frame);
            assert!(result.status.is_ok());
        }
        responder.join().unwrap();
    }

    #[test]
    fn polling_does_not_block_while_peer_delays_ack() {
        let (client, mut peer) = UnixStream::pair().unwrap();
        client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let worker = ContinuousKmsWorker::spawn(client).unwrap();
        let frame = plan(4, 1, 9);
        worker.submit(ContinuousKmsJob {
            planned: frame, pixels: vec![0; IMAGE_BYTES],
        }).unwrap();
        let start = Instant::now();
        assert!(worker.try_result().unwrap().is_none());
        assert!(start.elapsed() < Duration::from_secs(1));
        let received = consume(&mut peer);
        assert_eq!(received, frame);
        assert!(worker.try_result().unwrap().is_none());
        peer.write_all(&ack(frame, 0)).unwrap();
        assert!(result_for(&worker).status.is_ok());
    }

    #[test]
    fn stop_interrupts_blocked_ack_and_cannot_accept_more_jobs() {
        let (client, mut peer) = UnixStream::pair().unwrap();
        client.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut worker = ContinuousKmsWorker::spawn(client).unwrap();
        let frame = plan(9, 1, 13);
        worker.submit(ContinuousKmsJob {
            planned: frame, pixels: vec![0; IMAGE_BYTES],
        }).unwrap();
        // Read the whole frame but never send MLA1. The worker is blocked
        // waiting for the ACK when a VT/error teardown stops it.
        assert_eq!(consume(&mut peer), frame);
        let start = Instant::now();
        worker.stop();
        assert!(start.elapsed() < Duration::from_secs(2));
        assert!(matches!(worker.submit(ContinuousKmsJob {
            planned: frame, pixels: vec![0; IMAGE_BYTES],
        }), Err(TrySendError::Disconnected(_))));
        let mut byte = [0u8; 1];
        assert_eq!(peer.read(&mut byte).unwrap(), 0);
        // Shutdown is idempotent; Drop must not wait on an already joined worker.
        worker.stop();
    }

    #[test]
    fn stale_epoch_or_generation_cannot_acknowledge_frame() {
        for wrong in [plan(7, 9, 12), plan(8, 9, 11), plan(8, 10, 12)] {
            let (client, mut peer) = UnixStream::pair().unwrap();
            client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let worker = ContinuousKmsWorker::spawn(client).unwrap();
            let expected = plan(8, 9, 12);
            worker.submit(ContinuousKmsJob {
                planned: expected, pixels: vec![0; IMAGE_BYTES],
            }).unwrap();
            let received = consume(&mut peer);
            assert_eq!(received, expected);
            peer.write_all(&ack(wrong, 0)).unwrap();
            assert!(result_for(&worker).status.is_err());
        }
    }

    #[test]
    fn failed_ack_and_invalid_payload_never_report_success() {
        let (client, mut peer) = UnixStream::pair().unwrap();
        client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let worker = ContinuousKmsWorker::spawn(client).unwrap();
        let frame = plan(2, 1, 1);
        worker.submit(ContinuousKmsJob {
            planned: frame, pixels: vec![0; IMAGE_BYTES],
        }).unwrap();
        assert_eq!(consume(&mut peer), frame);
        peer.write_all(&ack(frame, 1)).unwrap();
        assert!(result_for(&worker).status.is_err());

        let (client, _peer) = UnixStream::pair().unwrap();
        let worker = ContinuousKmsWorker::spawn(client).unwrap();
        worker.submit(ContinuousKmsJob {
            planned: frame, pixels: vec![0; 16],
        }).unwrap();
        assert!(result_for(&worker).status.is_err());
    }
}
