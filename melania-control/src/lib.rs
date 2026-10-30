//! Small, local IPC protocol between the compositor and Melania Panel.
//! Deliberately std-only: the protocol can be tested without GTK or Smithay.
//! One request/response per Unix socket connection. The socket lives in the
//! user's private XDG_RUNTIME_DIR, with permissions 0600.
use std::{fmt, io::{self, BufRead, BufReader, Read, Write}, os::unix::net::UnixStream, path::Path, time::Duration};

pub const SOCKET_ENV: &str = "MELANIA_CONTROL_SOCKET";
pub const MAX_RESPONSE: usize = 128 * 1024;
/// Maximum wire length for a single command, including its final LF.
pub const MAX_COMMAND: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInfo {
    pub id: u64,
    pub title: String,
    pub app_id: String,
    pub active: bool,
    pub minimized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command { List, Status, Workspace(u8), Send(u64,u8), Activate(u64), Minimize(u64), Close(u64), Quit }

impl Command {
    pub fn wire(self) -> String {
        match self {
            Self::List => "LIST\n".into(),
            Self::Status => "STATUS\n".into(),
            Self::Workspace(ws) => format!("WORKSPACE {ws}\n"),
            Self::Send(id,ws) => format!("SEND {id} {ws}\n"),
            Self::Quit => "QUIT\n".into(),
            Self::Activate(id) => format!("ACTIVATE {id}\n"),
            Self::Minimize(id) => format!("MINIMIZE {id}\n"),
            Self::Close(id) => format!("CLOSE {id}\n"),
        }
    }
    pub fn parse(raw: &str) -> Option<Self> {
        // Never execute an incomplete read: a request must be a single LF-terminated line.
        if raw.len() > MAX_COMMAND || !raw.ends_with('\n') { return None; }
        let line = raw.strip_suffix('\n')?;
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.contains('\n') || line.contains('\r') { return None; }
        let fields: Vec<&str> = line.split(' ').collect();
        match fields.as_slice() {
            ["LIST"] => Some(Self::List),
            ["STATUS"] => Some(Self::Status),
            ["WORKSPACE", ws] => ws.parse::<u8>().ok().filter(|w| (1..=4).contains(w)).map(Self::Workspace),
            ["SEND", id, ws] => id.parse::<u64>().ok().filter(|i| *i > 0).and_then(|id| ws.parse::<u8>().ok().filter(|w| (1..=4).contains(w)).map(|ws|Self::Send(id,ws))),
            ["QUIT"] => Some(Self::Quit),
            ["ACTIVATE", id] => id.parse().ok().filter(|i| *i > 0).map(Self::Activate),
            ["MINIMIZE", id] => id.parse().ok().filter(|i| *i > 0).map(Self::Minimize),
            ["CLOSE", id] => id.parse().ok().filter(|i| *i > 0).map(Self::Close),
            _ => None,
        }
    }
}

// Only unreserved bytes may travel unencoded inside tab-separated records.
// All other UTF-8 bytes are escaped, so titles cannot inject new protocol rows.
fn safe(c: u8) -> bool { c.is_ascii_alphanumeric() || b"-_.~ ".contains(&c) }
pub fn escape(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if safe(b) { out.push(b as char); }
        else { out.push_str(&format!("%{b:02X}")); }
    }
    out
}
pub fn unescape(value: &str) -> Option<String> {
    let mut out = Vec::new();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() { return None; }
            let hi = (bytes[i + 1] as char).to_digit(16)?;
            let lo = (bytes[i + 2] as char).to_digit(16)?;
            out.push(((hi << 4) | lo) as u8);
            i += 3;
        } else if safe(bytes[i]) { out.push(bytes[i]); i += 1; }
        else { return None; }
    }
    String::from_utf8(out).ok()
}

impl WindowInfo {
    pub fn row(&self) -> String {
        format!("WINDOW\t{}\t{}\t{}\t{}\t{}\n", self.id,
            u8::from(self.active), u8::from(self.minimized),
            escape(&self.title), escape(&self.app_id))
    }
    pub fn parse(row: &str) -> Option<Self> {
        let fields: Vec<&str> = row.trim_end_matches(['\r','\n']).split('\t').collect();
        let ["WINDOW", id, active, minimized, title, app_id] = fields.as_slice() else { return None; };
        if !["0", "1"].contains(active) || !["0", "1"].contains(minimized) {return None;}
        let id = id.parse().ok()?;
        if id == 0 {return None;}
        Some(Self {
            id, active: *active == "1", minimized: *minimized == "1",
            title: unescape(title)?, app_id: unescape(app_id)?,
        })
    }
}

#[derive(Debug)]
pub enum ControlError { Io(io::Error), Protocol(&'static str), Rejected(String) }
impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {Self::Io(e) => write!(f,"{e}"), Self::Protocol(e) => write!(f,"{e}"), Self::Rejected(e) => write!(f,"{e}")}
    }
}
impl std::error::Error for ControlError {}
impl From<io::Error> for ControlError { fn from(e: io::Error) -> Self {Self::Io(e)} }

pub fn request(path: &Path, command: Command) -> Result<Vec<WindowInfo>, ControlError> {
    let mut sock = UnixStream::connect(path)?;
    sock.set_read_timeout(Some(Duration::from_millis(350)))?;
    sock.set_write_timeout(Some(Duration::from_millis(350)))?;
    sock.write_all(command.wire().as_bytes())?;
    // Explicit EOF lets the compositor reject truncated/multi-command requests.
    sock.shutdown(std::net::Shutdown::Write)?;
    let reader = BufReader::new(sock);
    let mut data = Vec::new();
    let mut read = 0usize;
    for line in reader.split(b'\n') {
        let line = line?;
        read += line.len() + 1;
        if read > MAX_RESPONSE { return Err(ControlError::Protocol("Risposta IPC troppo grande")); }
        let line = std::str::from_utf8(&line).map_err(|_| ControlError::Protocol("UTF-8 non valido"))?;
        if line == "END" { return Ok(data); }
        if line == "OK" { continue; }
        if let Some(msg) = line.strip_prefix("ERR\t") {return Err(ControlError::Rejected(unescape(msg).unwrap_or_else(||msg.into())));}
        if let Some(w) = WindowInfo::parse(line) { data.push(w); }
        else { return Err(ControlError::Protocol("Risposta IPC inattesa")); }
    }
    Err(ControlError::Protocol("Risposta IPC incompleta"))
}

/// Read exactly one complete command after the client has closed its write half.
/// Never execute a prefix that merely resembles a valid command.
pub fn read_command<R: io::Read>(reader: &mut R) -> io::Result<Option<Command>> {
    let mut bytes = Vec::with_capacity(MAX_COMMAND + 1);
    reader.take((MAX_COMMAND + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_COMMAND {return Ok(None);}
    Ok(std::str::from_utf8(&bytes).ok().and_then(Command::parse))
}


/// Read the active workspace through the same bounded, private IPC channel.
/// STATUS is deliberately not a window-list request.
pub fn current_workspace(path: &Path) -> Result<u8, ControlError> {
    let mut sock=UnixStream::connect(path)?;
    sock.set_read_timeout(Some(Duration::from_millis(350)))?;
    sock.set_write_timeout(Some(Duration::from_millis(350)))?;
    sock.write_all(Command::Status.wire().as_bytes())?;
    sock.shutdown(std::net::Shutdown::Write)?;
    let mut bounded=Vec::new();
    sock.take(256).read_to_end(&mut bounded)?;
    let raw=std::str::from_utf8(&bounded).map_err(|_|ControlError::Protocol("UTF-8 non valido"))?;
    let lines:Vec<_>=raw.lines().collect();
    if lines.len()!=3 || lines[0]!="OK" || lines[2]!="END" {
        return Err(ControlError::Protocol("Risposta workspace inattesa"));
    }
    let value=lines[1].strip_prefix("WORKSPACE\t")
        .and_then(|s|s.parse::<u8>().ok())
        .filter(|i|(1..=4).contains(i))
        .ok_or(ControlError::Protocol("Workspace non valido"))?;
    Ok(value)
}

pub fn respond_workspace(workspace:u8)->String {
    format!("OK\nWORKSPACE\t{workspace}\nEND\n")
}

pub fn respond_list(rows: &[WindowInfo]) -> String {
    let mut out = String::from("OK\n");
    for row in rows { out.push_str(&row.row()); }
    out.push_str("END\n"); out
}
pub fn respond_error(msg: &str) -> String {format!("ERR\t{}\nEND\n", escape(msg))}
pub const ACK: &str = "OK\nEND\n";

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn commands_parse_strictly() {
        assert_eq!(Command::parse("LIST\n"),Some(Command::List));
        assert_eq!(Command::parse("QUIT\n"),Some(Command::Quit));
        assert_eq!(Command::parse("STATUS\n"),Some(Command::Status));
        assert_eq!(Command::parse("WORKSPACE 3\n"),Some(Command::Workspace(3)));
        assert_eq!(Command::parse("SEND 29 4\n"),Some(Command::Send(29,4)));
        for raw in ["WORKSPACE 0\n","WORKSPACE 5\n","SEND 0 1\n","SEND 2 9\n","SEND 2 -1\n"] {
            assert_eq!(Command::parse(raw),None);
        }
        assert_eq!(Command::parse("QUIT"),None);
        assert_eq!(Command::parse("QUIT anything\n"),None);
        assert_eq!(Command::parse("ACTIVATE 42\n"),Some(Command::Activate(42)));
        for s in ["MINIMIZE 0\n","CLOSE -1\n","LIST extra\n","ACTIVATE 2 3\n","BOGUS 12\n","ACTIVATE 1\nINJECT","CLOSE 1", "CLOSE 1\nJUNK\n"] {
            assert_eq!(Command::parse(s),None, "{s}");
        }
    }
    #[test] fn rejects_truncated_and_oversized_requests() {
        assert_eq!(Command::parse("CLOSE 1"), None);
        assert_eq!(Command::parse("CLOSE 1\r\n"), Some(Command::Close(1)));
        assert_eq!(Command::parse(&format!("ACTIVATE {}\n", "1".repeat(100))), None);
    }
    #[test] fn escaping_round_trip() {
        for s in ["File.txt","Documento\tprivato\n2026", "Caffè ☕", "100% completo"] {
            assert_eq!(unescape(&escape(s)).as_deref(),Some(s));
        }
    }
    #[test] fn invalid_encoding() {
        for s in ["%G0","%","raw\ttab","%FF"] {assert!(unescape(s).is_none());}
    }
    #[test] fn window_round_trip() {
        let x=WindowInfo{id:4,title:"Finestra €\nnuova".into(),app_id:"org.test.App".into(),active:true,minimized:false};
        assert_eq!(WindowInfo::parse(&x.row()),Some(x));
    }
    #[test] fn response_parsing_roundtrip_on_socket() {
        use std::os::unix::net::UnixListener;
        let base=std::env::temp_dir().join(format!("melania-test-{}-{}",std::process::id(),"socket"));
        let listener=UnixListener::bind(&base).unwrap();
        let server=std::thread::spawn(move || {
            let (mut sock,_) = listener.accept().unwrap();
            let mut input=[0u8;100];let n=sock.read(&mut input).unwrap();
            assert_eq!(&input[..n],b"LIST\n");
            sock.write_all(respond_list(&[WindowInfo{id:1,title:"Terminale".into(),app_id:"term".into(),active:true,minimized:false}]).as_bytes()).unwrap();
        });
        let rows=request(&base,Command::List).unwrap();
        assert_eq!(rows[0].title,"Terminale");
        server.join().unwrap();let _=std::fs::remove_file(base);
    }
    #[test] fn fragmented_ipc_requires_eof() {
        use std::os::unix::net::UnixStream;
        let (mut one,mut two)=UnixStream::pair().unwrap();
        let server=std::thread::spawn(move ||read_command(&mut one).unwrap());
        two.write_all(b"CLO").unwrap();
        two.write_all(b"SE 12\n").unwrap();
        two.shutdown(std::net::Shutdown::Write).unwrap();
        assert_eq!(server.join().unwrap(), Some(Command::Close(12)));
    }
    #[test] fn rejects_multiple_commands_in_one_connection() {
        let mut reader=&b"CLOSE 12\nLIST\n"[..];
        assert_eq!(read_command(&mut reader).unwrap(),None);
    }
    #[test] fn status_response_is_strict() {
        assert_eq!(respond_workspace(2),"OK\nWORKSPACE\t2\nEND\n");
        let temp=std::env::temp_dir().join(format!("melania-workspace-test-{}",std::process::id()));
        let listener=std::os::unix::net::UnixListener::bind(&temp).unwrap();
        let server=std::thread::spawn(move ||{
            let (mut s,_) = listener.accept().unwrap();
            assert_eq!(read_command(&mut s).unwrap(),Some(Command::Status));
            s.write_all(respond_workspace(3).as_bytes()).unwrap();
        });
        assert_eq!(current_workspace(&temp).unwrap(),3);
        server.join().unwrap();let _=std::fs::remove_file(temp);
    }
    use std::io::Read;
}
