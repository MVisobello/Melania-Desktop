//! Recent Start launches. Store desktop IDs only, never executable commands.
//! Privacy: local user-owned state; no telemetry or background application scan.
use std::{env, fs, io::{self, Write}, path::Path, time::{SystemTime, UNIX_EPOCH}};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

pub const LIMIT: usize = 8;
const MAX_HISTORY_BYTES: u64 = 8 * 1024;

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 180 &&
    id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

pub fn history_path() -> Option<std::path::PathBuf> {
    melania_config::recent_apps_path()
}

pub fn load_from(path: &Path) -> Vec<String> {
    // State is intentionally tiny. Refuse corrupted or oversized histories
    // before allocating memory on the UI thread.
    let Ok(meta) = fs::metadata(path) else { return Vec::new(); };
    if !meta.is_file() || meta.len() > MAX_HISTORY_BYTES {return Vec::new();}
    let Ok(text) = fs::read_to_string(path) else { return Vec::new(); };
    let mut result = Vec::new();
    for line in text.lines() {
        let id = line.trim();
        if valid_id(id) && !result.iter().any(|candidate| candidate == id) {
            result.push(id.to_owned());
            if result.len() == LIMIT { break; }
        }
    }
    result
}

pub fn recent() -> Vec<String> {
    history_path().map(|path| load_from(&path)).unwrap_or_default()
}

pub fn record_at(path: &Path, id: &str) -> io::Result<()> {
    if !valid_id(id) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,"Invalid desktop application ID"));
    }
    let mut history = load_from(path);
    history.retain(|entry| entry != id);
    history.insert(0,id.to_owned());
    history.truncate(LIMIT);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        // The Melania directory stores a user's activity. Do not leak it via
        // ordinary world-readable defaults on multi-user Linux machines.
        #[cfg(unix)]
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    }
    let timestamp=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let temp=path.with_extension(format!("tmp-{}-{timestamp}",std::process::id()));
    let result = (|| -> io::Result<()> {
        let mut options=fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut out=options.open(&temp)?;
        out.write_all(format!("{}\n",history.join("\n")).as_bytes())?;
        out.sync_all()?;
        fs::rename(&temp,path)?;
        Ok(())
    })();
    if result.is_err() {let _=fs::remove_file(&temp);}
    result
}

pub fn record_launch(id: &str) {
    if !valid_id(id) {return;}
    crate::start_usage::record_launch(id);
    if let Some(path) = history_path() {
        if let Err(error) = record_at(&path,id) {
            eprintln!("Melania Start: cannot remember recent application: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_identifiers_without_accepting_commands_or_newlines() {
        for id in ["org.melaniadesktop.Files","org.gnome.Nautilus.desktop","firefox"] {
            assert!(valid_id(id));
        }
        for id in ["","/usr/bin/firefox","firefox\nnotepad","a;rm","$(command)"] {
            assert!(!valid_id(id),"{id:?}");
        }
        assert!(!valid_id(&"a".repeat(181)));
    }
    #[test]
    fn recent_history_keeps_unique_latest_first_with_cap() {
        let dir=env::temp_dir().join(format!("melania-recent-test-{}-{:?}",std::process::id(),std::thread::current().id()));
        let path=dir.join("recent");
        for n in 0..12 {record_at(&path,&format!("org.example.App{n}")).unwrap();}
        record_at(&path,"org.example.App6").unwrap();
        let recent=load_from(&path);
        assert_eq!(recent.len(),LIMIT);
        assert_eq!(recent[0],"org.example.App6");
        assert_eq!(recent.iter().filter(|x| *x=="org.example.App6").count(),1);
        assert!(!recent.contains(&"org.example.App0".to_string()));
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn corrupt_oversized_state_is_never_loaded() {
        let dir=env::temp_dir().join(format!("melania-recent-oversized-{}",std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path=dir.join("recent");
        fs::write(&path,"a".repeat((MAX_HISTORY_BYTES+1) as usize)).unwrap();
        assert!(load_from(&path).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn recent_app_state_is_private_to_current_user() {
        let dir=env::temp_dir().join(format!("melania-recent-private-{}",std::process::id()));
        let path=dir.join("recent");
        record_at(&path,"org.example.App").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o777,0o600);
        assert_eq!(fs::metadata(&dir).unwrap().permissions().mode() & 0o777,0o700);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn broken_or_untrusted_history_is_ignored() {
        let dir=env::temp_dir().join(format!("melania-recent-parse-{}",std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path=dir.join("recent");
        fs::write(&path,"good.desktop\ngood.desktop\n/invalid/path\nother.desktop\n").unwrap();
        assert_eq!(load_from(&path), vec!["good.desktop","other.desktop"]);
        fs::remove_dir_all(dir).unwrap();
    }
}
