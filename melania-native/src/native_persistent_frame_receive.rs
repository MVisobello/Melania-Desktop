//! Bounded *incomplete* persistent KMS frame reader on the real UnixStream.
//!
//! A desktop can legitimately go idle for hours between surface commits.
//! Once one MLF1 byte has arrived, however, a client/transport failure must
//! not hold the libseat DRM CRTC forever by trickling header or pixel bytes.
//! The same absolute deadline covers the remaining header AND scene pixels.
//! Any truncated/timed-out frame is an error: CPU and GBM presenters execute
//! their normal guarded rollback and never emit an MLA1 success ACK.
//! This is real socket I/O used by both kernel presenters, not KMS mock data.
use std::{
    io::{self, Error, ErrorKind, Read},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

/// Bound one partially transmitted 1280x800 frame while permitting unlimited
/// idle time *before* the first byte. This is not a DRM/page-flip deadline.
pub const MAX_PARTIAL_FRAME_TIME:Duration=Duration::from_secs(30);

pub struct PersistentFrameRead {
    deadline:Instant,
}
impl PersistentFrameRead {
    /// Wait without an idle timeout for a frame to begin. Only a zero-byte
    /// read at the FIRST byte is an orderly EOF; later EOF is always fatal.
    pub fn begin(
        stream:&mut UnixStream,
        header:&mut [u8;32],
        timeout:Duration,
    )->io::Result<Option<Self>> {
        stream.set_read_timeout(None)?;
        loop {
            match stream.read(&mut header[..1]) {
                Ok(0)=>return Ok(None),
                Ok(1)=>break,
                Ok(_)=>unreachable!("single-byte slice"),
                Err(error) if error.kind()==ErrorKind::Interrupted=>continue,
                Err(error)=>return Err(error),
            }
        }
        let deadline=Instant::now().checked_add(timeout)
            .ok_or_else(||Error::new(ErrorKind::InvalidInput,
                "persistent KMS partial-frame deadline overflow"))?;
        let reader=Self{deadline};
        reader.read_exact_before_deadline(stream,&mut header[1..])?;
        Ok(Some(reader))
    }

    /// Complete the same started frame, WITHOUT extending its deadline when
    /// the sender trickles bytes. Only success restores unlimited idle time
    /// for the next full frame boundary.
    pub fn read_payload(
        self,
        stream:&mut UnixStream,
        pixels:&mut [u8],
    )->io::Result<()> {
        self.read_exact_before_deadline(stream,pixels)?;
        stream.set_read_timeout(None)
    }

    fn read_exact_before_deadline(
        &self,
        stream:&mut UnixStream,
        mut destination:&mut [u8],
    )->io::Result<()> {
        while !destination.is_empty() {
            let remaining=self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Error::new(ErrorKind::TimedOut,
                    "persistent KMS scene incomplete after frame deadline"));
            }
            // A kernel socket read may wake for one byte at a time; update
            // the timeout from the ORIGINAL deadline, never from the most
            // recently received byte. EINTR also cannot extend the deadline.
            stream.set_read_timeout(Some(remaining))?;
            match stream.read(destination) {
                Ok(0)=>return Err(Error::new(ErrorKind::UnexpectedEof,
                    "persistent KMS scene truncated within frame")),
                Ok(received)=>{
                    if Instant::now()>self.deadline {
                        return Err(Error::new(ErrorKind::TimedOut,
                            "persistent KMS frame completed too late"));
                    }
                    destination=&mut destination[received..];
                },
                Err(error) if error.kind()==ErrorKind::Interrupted=>continue,
                Err(error) if matches!(error.kind(),ErrorKind::TimedOut|ErrorKind::WouldBlock)=>
                    return Err(Error::new(ErrorKind::TimedOut,
                        "persistent KMS scene stalled before completion")),
                Err(error)=>return Err(error),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write,thread};

    fn header()->[u8;32] {
        let mut h=[0u8;32];
        h[..4].copy_from_slice(b"MLF1");
        h[4..12].copy_from_slice(&1u64.to_le_bytes());
        h[12..20].copy_from_slice(&2u64.to_le_bytes());
        h[20..28].copy_from_slice(&3u64.to_le_bytes());
        h[28..32].copy_from_slice(&16u32.to_le_bytes());
        h
    }

    #[test]
    fn idle_closed_at_frame_boundary_is_valid_eof() {
        let (mut reader,writer)=UnixStream::pair().unwrap();
        drop(writer);
        assert!(PersistentFrameRead::begin(
            &mut reader,&mut [0u8;32],Duration::from_millis(80),
        ).unwrap().is_none());
    }

    #[test]
    fn real_fragmented_header_and_pixels_complete_before_one_deadline() {
        let (mut reader,mut writer)=UnixStream::pair().unwrap();
        let worker=thread::spawn(move ||{
            let h=header();
            writer.write_all(&h[..5]).unwrap();
            thread::sleep(Duration::from_millis(8));
            writer.write_all(&h[5..]).unwrap();
            writer.write_all(&[7u8;6]).unwrap();
            thread::sleep(Duration::from_millis(8));
            writer.write_all(&[7u8;10]).unwrap();
        });
        let mut h=[0u8;32];
        let in_flight=PersistentFrameRead::begin(
            &mut reader,&mut h,Duration::from_secs(1),
        ).unwrap().unwrap();
        assert_eq!(h,header());
        let mut pixels=[0u8;16];
        in_flight.read_payload(&mut reader,&mut pixels).unwrap();
        assert_eq!(pixels,[7u8;16]);
        assert_eq!(reader.read_timeout().unwrap(),None);
        worker.join().unwrap();
        assert!(PersistentFrameRead::begin(
            &mut reader,&mut h,Duration::from_secs(1),
        ).unwrap().is_none());
    }

    #[test]
    fn unfinished_header_times_out_but_idle_before_first_byte_does_not() {
        let (mut reader,mut writer)=UnixStream::pair().unwrap();
        // A real peer remains connected but never completes MLF1.
        writer.write_all(b"M").unwrap();
        let mut h=[0u8;32];
        let error=match PersistentFrameRead::begin(
            &mut reader,&mut h,Duration::from_millis(65),
        ) {Ok(_)=>panic!("partial header must be rejected"),Err(e)=>e};
        assert_eq!(error.kind(),ErrorKind::TimedOut);
    }

    #[test]
    fn partial_payload_timeout_never_looks_like_clean_shutdown() {
        let (mut reader,mut writer)=UnixStream::pair().unwrap();
        writer.write_all(&header()).unwrap();
        writer.write_all(&[8u8;4]).unwrap();
        let mut h=[0u8;32];
        let in_flight=PersistentFrameRead::begin(
            &mut reader,&mut h,Duration::from_millis(65),
        ).unwrap().unwrap();
        let mut pixels=[0u8;16];
        let error=in_flight.read_payload(&mut reader,&mut pixels).unwrap_err();
        assert_eq!(error.kind(),ErrorKind::TimedOut);
    }

    #[test]
    fn trickled_payload_cannot_extend_original_header_deadline() {
        let (mut reader,mut writer)=UnixStream::pair().unwrap();
        writer.write_all(&header()).unwrap();
        let sender=thread::spawn(move ||{
            for _ in 0..8 {
                thread::sleep(Duration::from_millis(30));
                if writer.write_all(&[0x33]).is_err() {break;}
            }
        });
        let mut h=[0u8;32];
        let in_flight=PersistentFrameRead::begin(
            &mut reader,&mut h,Duration::from_millis(85),
        ).unwrap().unwrap();
        let mut pixels=[0u8;8];
        let error=in_flight.read_payload(&mut reader,&mut pixels).unwrap_err();
        assert_eq!(error.kind(),ErrorKind::TimedOut);
        drop(reader);
        sender.join().unwrap();
    }
}
