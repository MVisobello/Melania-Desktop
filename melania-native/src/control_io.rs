//! Bounded nonblocking native control IPC for the isolated Smithay/QEMU bridge.
//!
//! A client must close the write half after one complete LF command. Reading,
//! command dispatch and partial replies are driven by the normal compositor
//! loop. No peer is permitted to block the libinput, Wayland or DRM ACK pump.
//! The UnixListener must already be bound to the 0600 private XDG socket.
use melania_control::{self, Command, MAX_COMMAND, MAX_RESPONSE};
use std::{
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::net::{UnixListener, UnixStream},
    time::{Duration, Instant},
};

const MAX_PEERS: usize = 16;
const ACCEPT_BUDGET: usize = 8;
const IO_STEPS_PER_PEER: usize = 2;
const PEER_TIMEOUT: Duration = Duration::from_secs(2);

struct Peer {
    stream: UnixStream,
    received: Vec<u8>,
    reply: Option<Vec<u8>>,
    sent: usize,
    connected: Instant,
}

impl Peer {
    fn new(stream: UnixStream) -> Self {
        Self {
            stream,
            received: Vec::with_capacity(MAX_COMMAND + 1),
            reply: None,
            sent: 0,
            connected: Instant::now(),
        }
    }
    fn respond(&mut self, text: String) {
        let bytes = text.into_bytes();
        // Never create an unbounded queued response on the main thread.
        self.reply = Some(if bytes.len() <= MAX_RESPONSE {
            bytes
        } else {
            melania_control::respond_error("Risposta IPC troppo grande").into_bytes()
        });
        self.sent = 0;
    }

    /// Return false only when the connection has finished or failed.
    fn advance(&mut self, now: Instant, command: &mut impl FnMut(Command) -> String) -> bool {
        if now.duration_since(self.connected) >= PEER_TIMEOUT {
            // A stalled peer cannot retain an FD indefinitely or keep the
            // compositor waiting for an EOF that will never arrive.
            return false;
        }
        if self.reply.is_none() {
            for _ in 0..IO_STEPS_PER_PEER {
                let mut bytes = [0u8; 128];
                match self.stream.read(&mut bytes) {
                    Ok(0) => {
                        let parsed = std::str::from_utf8(&self.received)
                            .ok().and_then(Command::parse);
                        let response = parsed.map(&mut *command).unwrap_or_else(|| {
                            melania_control::respond_error("Comando IPC incompleto o non valido")
                        });
                        self.respond(response);
                        break;
                    }
                    Ok(count) => {
                        if self.received.len().saturating_add(count) > MAX_COMMAND {
                            self.respond(melania_control::respond_error("Comando IPC troppo lungo"));
                            break;
                        }
                        self.received.extend_from_slice(&bytes[..count]);
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => return false,
                }
            }
        }
        if let Some(ref output) = self.reply {
            for _ in 0..IO_STEPS_PER_PEER {
                if self.sent == output.len() {return false;}
                match self.stream.write(&output[self.sent..]) {
                    Ok(0) => return false,
                    Ok(written) => self.sent += written,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => return false,
                }
            }
            if self.sent == output.len() {return false;}
        }
        true
    }
}

/// The caller owns the listener and its socket cleanup. A maximum of
/// sixteen small per-client buffers are kept, never a blocking read_to_end.
#[derive(Default)]
pub struct NativeControlPoller {
    peers: Vec<Peer>,
}
impl NativeControlPoller {
    pub fn new() -> Self {Self::default()}
    pub fn active_count(&self) -> usize {self.peers.len()}

    /// Execute on the compositor's existing event-loop tick. All accepted
    /// streams are set O_NONBLOCK before any read or write. A slow peer is
    /// isolated from other commands, input dispatch and rendering.
    pub fn pump(
        &mut self,
        listener: &UnixListener,
        mut command: impl FnMut(Command) -> String,
    ) -> io::Result<()> {
        for _ in 0..ACCEPT_BUDGET {
            match listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(true)?;
                    if self.peers.len() < MAX_PEERS {
                        self.peers.push(Peer::new(stream));
                    } else {
                        let _ = stream.shutdown(Shutdown::Both);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
        let now = Instant::now();
        self.peers.retain_mut(|peer| peer.advance(now, &mut command));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, net::Shutdown, os::unix::net::UnixStream};

    fn server() -> (UnixListener, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "melania-native-ipc-{}-{:?}.sock",
            std::process::id(), std::thread::current().id()
        ));
        let _ = fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        (listener, path)
    }
    fn read_reply(mut client: UnixStream) -> String {
        client.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
        let mut out = String::new();
        client.read_to_string(&mut out).unwrap();
        out
    }

    #[test]
    fn stalled_client_cannot_block_complete_ipc_from_another_client() {
        let (listener, path) = server();
        let mut poller = NativeControlPoller::new();
        let mut slow = UnixStream::connect(&path).unwrap();
        slow.write_all(b"ACTIVATE 1\n").unwrap(); // no write EOF
        let mut fast = UnixStream::connect(&path).unwrap();
        fast.write_all(b"LIST\n").unwrap();
        fast.shutdown(Shutdown::Write).unwrap();
        let mut calls = Vec::new();
        for _ in 0..4 {
            poller.pump(&listener, |cmd| {
                calls.push(cmd);
                "OK\nEND\n".into()
            }).unwrap();
        }
        assert_eq!(calls, vec![Command::List]);
        assert_eq!(read_reply(fast), "OK\nEND\n");
        slow.shutdown(Shutdown::Write).unwrap();
        poller.pump(&listener, |cmd| {calls.push(cmd); "OK\nEND\n".into()}).unwrap();
        assert_eq!(calls, vec![Command::List, Command::Activate(1)]);
        assert_eq!(read_reply(slow), "OK\nEND\n");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn invalid_and_oversized_messages_never_dispatch() {
        let (listener, path) = server();
        let mut poller = NativeControlPoller::new();
        let mut bad = UnixStream::connect(&path).unwrap();
        bad.write_all(&vec![b'A'; MAX_COMMAND + 5]).unwrap();
        bad.shutdown(Shutdown::Write).unwrap();
        let mut truncated = UnixStream::connect(&path).unwrap();
        truncated.write_all(b"MINIMIZE 1").unwrap();
        truncated.shutdown(Shutdown::Write).unwrap();
        let mut executed = 0;
        for _ in 0..4 {
            poller.pump(&listener, |_| {executed += 1; "OK\nEND\n".into()}).unwrap();
        }
        assert_eq!(executed, 0);
        assert!(read_reply(bad).starts_with("ERR\t"));
        assert!(read_reply(truncated).starts_with("ERR\t"));
        assert_eq!(poller.active_count(), 0);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn backpressure_is_bounded_and_a_slow_peer_does_not_drop_fast_requests() {
        let (listener, path) = server();
        let mut poller = NativeControlPoller::new();
        let held: Vec<_> = (0..MAX_PEERS).map(|_| UnixStream::connect(&path).unwrap()).collect();
        for _ in 0..4 { poller.pump(&listener, |_| unreachable!()).unwrap(); }
        assert_eq!(poller.active_count(), MAX_PEERS);
        let overflow = UnixStream::connect(&path).unwrap();
        poller.pump(&listener, |_| unreachable!()).unwrap();
        assert_eq!(poller.active_count(), MAX_PEERS);
        drop(overflow);
        drop(held);
        for _ in 0..4 {poller.pump(&listener, |_| unreachable!()).unwrap();}
        assert_eq!(poller.active_count(), 0);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn truncated_utf8_and_multicommand_payloads_fail_closed() {
        let (listener, path) = server();
        let mut poller = NativeControlPoller::new();
        for message in [b"LIST\nQUIT\n".as_slice(), &[0xff, b'\n']] {
            let mut client = UnixStream::connect(&path).unwrap();
            client.write_all(message).unwrap();
            client.shutdown(Shutdown::Write).unwrap();
            for _ in 0..3 {
                poller.pump(&listener, |_| panic!("invalid command was dispatched")).unwrap();
            }
            assert!(read_reply(client).starts_with("ERR\t"));
        }
        fs::remove_file(path).unwrap();
    }
}
