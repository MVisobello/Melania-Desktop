//! Desktop filesystem and selection model, deliberately independent of GTK.
//! No file is removed or replaced by any operation in this crate.
use std::{collections::BTreeSet, fs, io, path::{Path, PathBuf}};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopItem {
    pub path: PathBuf,
    pub name: String,
    pub is_directory: bool,
    pub is_symlink: bool,
}

/// A non-recursive bounded listing. Symbolic links are never followed here.
pub fn list_desktop(dir: &Path, limit: usize, show_hidden: bool) -> io::Result<Vec<DesktopItem>> {
    if limit == 0 { return Ok(Vec::new()); }
    // Keep only the first N sorted entries, rather than accumulating every
    // item from a potentially enormous user directory in memory.
    let mut items: Vec<DesktopItem> = Vec::with_capacity(limit.min(1024));
    let compare = |a: &DesktopItem, b: &DesktopItem| {
        b.is_directory.cmp(&a.is_directory)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.name.cmp(&b.name))
    };
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !show_hidden && name.starts_with('.') { continue; }
        let path = entry.path();
        // Entries can disappear between readdir and metadata during drag/drop.
        let metadata = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        let item = DesktopItem {
            path, name, is_directory: metadata.is_dir(),
            is_symlink: metadata.file_type().is_symlink(),
        };
        let index = items.binary_search_by(|other| compare(other, &item))
            .unwrap_or_else(|index| index);
        if index < limit {
            items.insert(index, item);
            if items.len() > limit { items.pop(); }
        }
    }
    Ok(items)
}

/// Creates a new folder using atomic `mkdir`, never overwriting any existing
/// file, symlink or directory with the candidate name. Does not create `dir`.
pub fn create_unique_folder(dir: &Path) -> io::Result<PathBuf> {
    if !dir.is_dir() { return Err(io::Error::new(io::ErrorKind::NotFound, "Cartella Scrivania non disponibile")); }
    for n in 1..=10_000usize {
        let name = if n == 1 { "Nuova cartella".to_owned() } else { format!("Nuova cartella ({n})") };
        let candidate = dir.join(name);
        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "Troppe cartelle con lo stesso nome"))
}

/// Selection keys are full paths, not transient indices in the icon grid.
#[derive(Debug, Default, Clone)]
pub struct Selection { selected: BTreeSet<PathBuf> }
impl Selection {
    pub fn select(&mut self, path: PathBuf, additive: bool) {
        if !additive { self.selected.clear(); self.selected.insert(path); }
        else if !self.selected.remove(&path) { self.selected.insert(path); }
    }
    pub fn clear(&mut self) { self.selected.clear(); }
    pub fn is_selected(&self, path: &Path) -> bool { self.selected.contains(path) }
    pub fn len(&self) -> usize { self.selected.len() }
    pub fn paths(&self) -> impl Iterator<Item = &PathBuf> { self.selected.iter() }
    pub fn retain_existing(&mut self) { self.selected.retain(|p| p.exists()); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    struct TestDir(PathBuf);
    impl TestDir { fn new() -> Self {
        let path = std::env::temp_dir().join(format!("melania-desktop-core-{}-{}",std::process::id(),COUNTER.fetch_add(1,Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }}
    impl Drop for TestDir { fn drop(&mut self) { let _=fs::remove_dir_all(&self.0); } }

    #[test] fn makes_folders_without_overwriting_files() {
        let t=TestDir::new(); fs::write(t.0.join("Nuova cartella"), b"important").unwrap();
        let folder=create_unique_folder(&t.0).unwrap();
        assert_eq!(folder.file_name().unwrap().to_str().unwrap(),"Nuova cartella (2)");
        assert!(folder.is_dir()); assert_eq!(fs::read(t.0.join("Nuova cartella")).unwrap(),b"important");
        assert_eq!(create_unique_folder(&t.0).unwrap().file_name().unwrap().to_str().unwrap(),"Nuova cartella (3)");
    }
    #[test] fn simultaneous_creation_never_collides() {
        let t=TestDir::new();
        let jobs=(0..12).map(|_|{let dir=t.0.clone();std::thread::spawn(move||create_unique_folder(&dir).unwrap())}).collect::<Vec<_>>();
        let unique=jobs.into_iter().map(|j|j.join().unwrap()).collect::<BTreeSet<_>>();
        assert_eq!(unique.len(),12); assert!(unique.iter().all(|p|p.is_dir()));
    }
    #[test] fn sorting_is_stable_and_hidden_files_are_hidden() {
        let t=TestDir::new(); fs::create_dir(t.0.join("Z folder")).unwrap();
        fs::write(t.0.join("b.txt"),b"b").unwrap();fs::write(t.0.join("A.txt"),b"a").unwrap();
        fs::write(t.0.join(".secret"),b"private").unwrap();
        let items=list_desktop(&t.0,2,false).unwrap();
        assert_eq!(items.iter().map(|i|i.name.as_str()).collect::<Vec<_>>(),vec!["Z folder","A.txt"]);
        assert_eq!(list_desktop(&t.0,100,true).unwrap().len(),4);
    }
    #[test] fn select_replaces_and_control_click_toggles() {
        let mut s=Selection::default();
        s.select("/tmp/a".into(),false);s.select("/tmp/b".into(),true);
        assert_eq!(s.len(),2);s.select("/tmp/a".into(),true);
        assert_eq!(s.len(),1); assert!(s.is_selected(Path::new("/tmp/b")));
        s.select("/tmp/c".into(),false); assert_eq!(s.len(),1);
        assert!(s.is_selected(Path::new("/tmp/c")));
    }
    #[cfg(unix)]
    #[test] fn symlink_is_not_treated_as_directory() {
        let t=TestDir::new();fs::create_dir(t.0.join("real")).unwrap();
        std::os::unix::fs::symlink(t.0.join("real"),t.0.join("shortcut")).unwrap();
        let items=list_desktop(&t.0,10,false).unwrap();
        let link=items.iter().find(|i|i.name=="shortcut").unwrap();
        assert!(link.is_symlink);assert!(!link.is_directory);
    }
}
