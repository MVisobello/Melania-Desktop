//! Notification model and atomic, per-session snapshot for Melania Panel.
//! Rust-only; the GTK frontend and D-Bus transport are intentionally separate.
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, fs::{self, OpenOptions}, io::{self, Write},
    os::unix::fs::PermissionsExt, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

pub const MAX_HISTORY: usize = 100;
pub const MAX_TEXT: usize = 4096;
pub const MAX_APP: usize = 128;
pub const MAX_SUMMARY: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    pub id: u32,
    pub app: String,
    pub summary: String,
    pub body: String,
    pub created_ms: u64,
    /// Whether the user has seen this notification in the notification center.
    #[serde(default)]
    pub unread: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub generation: u64,
    pub notifications: Vec<Notice>,
}

#[derive(Default)]
pub struct Store {
    next_id: u32,
    generation: u64,
    entries: VecDeque<Notice>,
}

/// Text limits count UTF-8 bytes, not Unicode scalar values: this keeps the
/// serialized session snapshot safely below its 1 MiB read cap.
fn normalize(s: &str, max: usize, multiline: bool) -> String {
    let mut result = String::new();
    for ch in s.chars() {
        if ch.is_control() && !(multiline && ch == '\n') { continue; }
        if result.len() + ch.len_utf8() > max { break; }
        result.push(ch);
    }
    result
}

fn timestamp_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_millis().min(u64::MAX as u128) as u64
}

impl Store {
    /// Unknown replacement IDs are treated as new notifications, as per freedesktop semantics.
    pub fn notify(&mut self, replaces: u32, app: &str, summary: &str, body: &str) -> u32 {
        let id = if replaces != 0 && self.entries.iter().any(|e| e.id == replaces) {
            self.entries.retain(|e| e.id != replaces);
            replaces
        } else {
            // Never issue zero. Handles wrap-around without reusing a live ID.
            loop {
                self.next_id = self.next_id.wrapping_add(1).max(1);
                if self.entries.iter().all(|e| e.id != self.next_id) {break self.next_id;}
            }
        };
        self.entries.push_front(Notice {
            id, app: normalize(app, MAX_APP, false),
            summary: normalize(summary, MAX_SUMMARY, false),
            body: normalize(body, MAX_TEXT, true), created_ms: timestamp_ms(),
            unread: true,
        });
        self.entries.truncate(MAX_HISTORY);
        self.generation = self.generation.wrapping_add(1);
        id
    }
    pub fn close(&mut self, id: u32) -> bool {
        let before = self.entries.len();
        self.entries.retain(|n| n.id != id);
        if self.entries.len() != before {
            self.generation = self.generation.wrapping_add(1);
            true
        } else { false }
    }
    /// Mark existing history as read. Returns how many entries changed.
    pub fn mark_all_read(&mut self) -> u32 {
        let mut changed = 0;
        for item in self.entries.iter_mut() {
            if item.unread { item.unread = false; changed += 1; }
        }
        if changed > 0 { self.generation = self.generation.wrapping_add(1); }
        changed
    }
    pub fn unread_count(&self) -> usize {
        self.entries.iter().filter(|item| item.unread).count()
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot { generation: self.generation,
            notifications: self.entries.iter().cloned().collect() }
    }
}

pub fn snapshot_path() -> io::Result<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "XDG_RUNTIME_DIR mancante"))?;
    let dir = PathBuf::from(runtime).join("melania");
    Ok(dir.join("notifications.json"))
}

/// Atomic replacement prevents the UI from reading a half-written JSON file.
/// The directory is user-private; all created files are mode 0600.
pub fn publish(path: &Path, snapshot: &Snapshot) -> io::Result<()> {
    let dir = path.parent().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path senza genitore"))?;
    fs::create_dir_all(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    let temp = dir.join(format!(".notifications-{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temp)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        serde_json::to_writer(&mut file, snapshot).map_err(io::Error::other)?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {let _ = fs::remove_file(&temp);}
    result
}

pub fn read_snapshot(path: &Path) -> io::Result<Snapshot> {
    let data = fs::read(path)?;
    if data.len() > 1024 * 1024 {return Err(io::Error::new(io::ErrorKind::InvalidData, "snapshot troppo grande"));}
    serde_json::from_slice(&data).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn replace_and_close() {
        let mut store=Store::default();
        let id=store.notify(0,"Files","Copia","In corso");
        assert_ne!(id,0);
        assert_eq!(store.notify(id,"Files","Copia","Completata"),id);
        assert_eq!(store.snapshot().notifications.len(),1);
        assert_eq!(store.snapshot().notifications[0].body,"Completata");
        assert!(store.close(id));
        assert!(!store.close(id));
    }
    #[test] fn bounds_and_truncation() {
        let mut store=Store::default();
        for i in 0..150 {store.notify(0,"app",&i.to_string(),&"x".repeat(9000));}
        assert_eq!(store.snapshot().notifications.len(),100);
        assert_eq!(store.snapshot().notifications[0].summary,"149");
        assert_eq!(store.snapshot().notifications[0].body.len(),MAX_TEXT);
    }
    #[test] fn unknown_replacement_gets_new_id() {
        let mut store=Store::default();
        assert_ne!(store.notify(74,"a","b","c"),74);
    }
    #[test] fn unread_count_tracks_replacement_and_close() {
        let mut store = Store::default();
        let first = store.notify(0, "mail", "Nuovo", "Messaggio");
        store.notify(0, "calendar", "Appuntamento", "Domani");
        assert_eq!(store.unread_count(), 2);
        assert_eq!(store.mark_all_read(), 2);
        assert_eq!(store.mark_all_read(), 0);
        assert_eq!(store.unread_count(), 0);
        let before = store.snapshot().generation;
        store.notify(first, "mail", "Aggiornato", "Nuovo testo");
        assert_eq!(store.unread_count(), 1);
        assert!(store.snapshot().generation > before);
        assert!(store.close(first));
        assert_eq!(store.unread_count(), 0);
    }
    #[test] fn boundaries_keep_snapshot_under_size_limit() {
        let mut store = Store::default();
        let invalid = "A".repeat(10000);
        for _ in 0..MAX_HISTORY { store.notify(0, &invalid, &invalid, &invalid); }
        let snapshot = store.snapshot();
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        assert!(bytes.len() < 1024 * 1024);
        assert_eq!(snapshot.notifications[0].app.len(), MAX_APP);
        assert_eq!(snapshot.notifications[0].summary.len(), MAX_SUMMARY);
        assert_eq!(snapshot.notifications[0].body.len(), MAX_TEXT);
    }
    #[test] fn unicode_does_not_bypass_byte_limits() {
        let mut store = Store::default();
        let text = "🔔".repeat(7000);
        for _ in 0..MAX_HISTORY {
            store.notify(0, &text, &text, &text);
        }
        let serialized = serde_json::to_vec(&store.snapshot()).unwrap();
        assert!(serialized.len() < 1024 * 1024);
        let entry = &store.snapshot().notifications[0];
        assert!(entry.app.len() <= MAX_APP);
        assert!(entry.summary.len() <= MAX_SUMMARY);
        assert!(entry.body.len() <= MAX_TEXT);
    }
    #[test] fn old_snapshot_without_unread_is_supported() {
        let legacy = r#"{"generation":1,"notifications":[{"id":1,"app":"A","summary":"B","body":"C","created_ms":12}]}"#;
        let parsed: Snapshot = serde_json::from_str(legacy).unwrap();
        assert!(!parsed.notifications[0].unread);
    }
    #[test] fn writes_valid_private_snapshot() {
        let path=std::env::temp_dir().join(format!("melania-notify-test-{}",std::process::id()))
            .join("notifications.json");
        let mut store=Store::default();store.notify(0,"test","titolo","corpo");
        publish(&path,&store.snapshot()).unwrap();
        assert_eq!(read_snapshot(&path).unwrap().notifications.len(),1);
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777,0o600);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}
