//! Recent Start launches. Store desktop IDs only, never executable commands.
//! Privacy: local user-owned state; no telemetry or background application scan.
use std::{env, fs, io, path::{Path, PathBuf}, time::{SystemTime, UNIX_EPOCH}};

pub const LIMIT: usize = 8;

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 180 &&
    id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

pub fn history_path() -> Option<PathBuf> {
    let root = env::var_os("XDG_STATE_HOME").filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").filter(|v| !v.is_empty())
            .map(|v| PathBuf::from(v).join(".local/state")))?;
    Some(root.join("melania").join("recent-applications"))
}

pub fn load_from(path: &Path) -> Vec<String> {
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
    if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
    let timestamp=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let temp=path.with_extension(format!("tmp-{}-{timestamp}",std::process::id()));
    fs::write(&temp,format!("{}\n",history.join("\n")))?;
    if let Err(error)=fs::rename(&temp,path) {let _=fs::remove_file(temp);return Err(error);}
    Ok(())
}

pub fn record_launch(id: &str) {
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
        for id in ["","/usr/bin/firefox","firefox\nnotepad","a;rm","$(command)","a".repeat(181).as_str()] {
            assert!(!valid_id(id),"{id:?}");
        }
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
    fn broken_or_untrusted_history_is_ignored() {
        let dir=env::temp_dir().join(format!("melania-recent-parse-{}",std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path=dir.join("recent");
        fs::write(&path,"good.desktop\ngood.desktop\n/invalid/path\nother.desktop\n").unwrap();
        assert_eq!(load_from(&path), vec!["good.desktop","other.desktop"]);
        fs::remove_dir_all(dir).unwrap();
    }
}
