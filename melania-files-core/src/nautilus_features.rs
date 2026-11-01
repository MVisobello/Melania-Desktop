//! Foundation for GNOME Files/Nautilus-like local features.
//! No GTK dependency. Never traverse symlinked directories; never overwrite a
//! target during batch planning. Bounded search prevents unbounded recursion.
use std::{collections::{HashSet, VecDeque}, fs, io, path::{Path, PathBuf}, time::SystemTime};

#[derive(Clone, Copy, Debug)]
pub struct SearchLimits {
    pub max_results: usize,
    pub max_directories: usize,
    pub show_hidden: bool,
}
impl Default for SearchLimits {
    fn default() -> Self { Self { max_results: 2_000, max_directories: 300, show_hidden: false } }
}
#[derive(Debug)]
pub struct SearchResult {
    pub paths: Vec<PathBuf>,
    pub directories_scanned: usize,
    pub truncated: bool,
}
/// Case-insensitive bounded recursive filename search. Searches names, not
/// file contents. Does not follow symlinks or leave the requested root.
pub fn search_tree(root: &Path, query: &str, limits: SearchLimits) -> io::Result<SearchResult> {
    search_tree_cancellable(root, query, limits, || false)
}

/// Runs in a worker thread. The callback is checked before each directory
/// and each entry so a closed search window can cancel large scans quickly.
pub fn search_tree_cancellable<F: Fn() -> bool>(
    root: &Path, query: &str, limits: SearchLimits, cancelled: F
) -> io::Result<SearchResult> {
    // A symlink as root is not traversed either; don't silently escape the
    // user-selected search tree by resolving a linked starting directory.
    if !fs::symlink_metadata(root)?.is_dir() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "La radice non è una cartella reale"));
    }
    let q = query.trim().to_lowercase();
    if q.is_empty() || limits.max_results == 0 || limits.max_directories == 0 {
        return Ok(SearchResult { paths: vec![], directories_scanned: 0, truncated: false });
    }
    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut out = Vec::new();
    let mut visited = 0usize;
    let mut truncated = false;
    while let Some(dir) = queue.pop_front() {
        if cancelled() { return Err(io::Error::new(io::ErrorKind::Interrupted,"Ricerca annullata")); }
        if visited >= limits.max_directories { truncated = true; break; }
        visited += 1;
        let iter = match fs::read_dir(&dir) {
            Ok(iter) => iter,
            Err(e) if dir == root => return Err(e),
            Err(_) => continue, // unreadable nested directories are skipped
        };
        for child in iter {
            if cancelled() { return Err(io::Error::new(io::ErrorKind::Interrupted,"Ricerca annullata")); }
            let child = match child { Ok(c) => c, Err(_) => continue };
            let name = child.file_name().to_string_lossy().into_owned();
            if !limits.show_hidden && name.starts_with('.') { continue; }
            let path = child.path();
            let meta = match fs::symlink_metadata(&path) { Ok(m) => m, Err(_) => continue };
            if name.to_lowercase().contains(&q) {
                out.push(path.clone());
                if out.len() >= limits.max_results { truncated = true; break; }
            }
            if meta.is_dir() { queue.push_back(path); }
        }
        if truncated { break; }
    }
    out.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    Ok(SearchResult { paths: out, directories_scanned: visited, truncated })
}

#[derive(Debug)]
pub struct FileProperties {
    pub path: PathBuf,
    pub kind: &'static str,
    pub bytes: u64,
    pub modified: Option<SystemTime>,
    pub readonly: bool,
    pub symlink_target: Option<PathBuf>,
    #[cfg(unix)] pub mode: u32,
}
/// Uses symlink_metadata, so inspecting a symlink does not follow its target.
pub fn properties(path: &Path) -> io::Result<FileProperties> {
    let meta = fs::symlink_metadata(path)?;
    let kind = if meta.file_type().is_symlink() { "Collegamento simbolico" }
        else if meta.is_dir() { "Cartella" }
        else if meta.is_file() { "File" } else { "File speciale" };
    #[cfg(unix)] use std::os::unix::fs::PermissionsExt;
    Ok(FileProperties { path: path.to_path_buf(), kind, bytes: meta.len(),
        modified: meta.modified().ok(), readonly: meta.permissions().readonly(),
        symlink_target: if meta.file_type().is_symlink() { fs::read_link(path).ok() } else { None },
        #[cfg(unix)] mode: meta.permissions().mode() & 0o7777,
    })
}

/// Preview a batch rename; does not write anything. Reject invalid names,
/// duplicates, conflicts, and repeated paths before planning any change.
pub fn batch_rename_plan(sources: &[PathBuf], prefix: &str, start: u32) -> io::Result<Vec<(PathBuf, PathBuf)>> {
    let prefix = prefix.trim();
    if prefix.is_empty() || prefix.contains('/') || prefix.contains('\0') || matches!(prefix, "." | "..") {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Prefisso non valido"));
    }
    if sources.len() > 500 { return Err(io::Error::new(io::ErrorKind::InvalidInput, "Massimo 500 file")); }
    let mut used = HashSet::new();
    let mut old = HashSet::new();
    let mut plan = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        if !old.insert(source.clone()) { return Err(io::Error::new(io::ErrorKind::InvalidInput, "Origine duplicata")); }
        let meta = fs::symlink_metadata(source)?;
        let suffix = if meta.is_file() { source.extension().and_then(|s| s.to_str()).filter(|s| !s.is_empty()).map(|s| format!(".{s}")).unwrap_or_default() } else { String::new() };
        let n = start.checked_add(index as u32).ok_or_else(||io::Error::new(io::ErrorKind::InvalidInput, "Numero troppo grande"))?;
        let name = format!("{prefix} {n:03}{suffix}");
        let target = source.parent().ok_or_else(||io::Error::new(io::ErrorKind::InvalidInput, "Origine senza cartella"))?.join(name);
        if !used.insert(target.clone()) {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Destinazione duplicata"));
        }
        // symlink_metadata treats even broken symlinks as occupied targets.
        match fs::symlink_metadata(&target) {
            Ok(_) => return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Destinazione occupata")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
        plan.push((source.clone(), target));
    }
    Ok(plan)
}

/// A template is copied to a new name using O_EXCL; symlink templates are
/// refused to avoid surprising reads outside the template directory.
pub fn create_from_template(source: &Path, target: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(source)?;
    if !meta.is_file() { return Err(io::Error::new(io::ErrorKind::InvalidInput, "Modello non regolare")); }
    let mut input = fs::File::open(source)?;
    let mut output = fs::OpenOptions::new().write(true).create_new(true).open(target)?;
    let copied = io::copy(&mut input, &mut output);
    if copied.is_err() { drop(output); let _ = fs::remove_file(target); }
    copied.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    struct Tmp(PathBuf);
    impl Tmp { fn new()->Self { let p=std::env::temp_dir().join(format!("melania-nautilus-{}-{}",std::process::id(),N.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&p).unwrap();Self(p) } }
    impl Drop for Tmp { fn drop(&mut self){ let _=fs::remove_dir_all(&self.0); } }
    #[test] fn cancelled_recursive_search_stops_immediately() {
        let t=Tmp::new();
        fs::create_dir(t.0.join("sub")).unwrap();
        let error=search_tree_cancellable(&t.0,"anything",SearchLimits::default(),||true).unwrap_err();
        assert_eq!(error.kind(),io::ErrorKind::Interrupted);
    }
    #[test] fn search_nested_and_hidden() {
        let t=Tmp::new(); fs::create_dir(t.0.join("deep")).unwrap();
        fs::write(t.0.join("deep/Relazione.PDF"),b"x").unwrap();
        fs::write(t.0.join(".Relazione"),b"x").unwrap();
        let r=search_tree(&t.0,"relazione",SearchLimits::default()).unwrap();
        assert_eq!(r.paths.len(),1);
        let r=search_tree(&t.0,"relazione",SearchLimits{show_hidden:true,..Default::default()}).unwrap();
        assert_eq!(r.paths.len(),2);
    }
    #[cfg(unix)]
    #[test] fn search_never_follows_directory_symlink() {
        let t=Tmp::new(); std::os::unix::fs::symlink("/etc",t.0.join("system")).unwrap();
        let r=search_tree(&t.0,"passwd",SearchLimits::default()).unwrap();
        assert!(r.paths.is_empty());
    }
    #[cfg(unix)]
    #[test] fn symlink_as_search_root_rejected() {
        let t=Tmp::new();let dst=t.0.join("real");fs::create_dir(&dst).unwrap();
        let link=t.0.join("link");std::os::unix::fs::symlink(&dst,&link).unwrap();
        assert!(search_tree(&link,"anything",SearchLimits::default()).is_err());
    }
    #[cfg(unix)]
    #[test] fn batch_plan_rejects_broken_symlink_target() {
        let t=Tmp::new();let source=t.0.join("a.txt");fs::write(&source,b"a").unwrap();
        std::os::unix::fs::symlink("missing",t.0.join("Test 001.txt")).unwrap();
        assert_eq!(batch_rename_plan(&[source],"Test",1).unwrap_err().kind(),io::ErrorKind::AlreadyExists);
    }
    #[test] fn search_is_bounded() {
        let t=Tmp::new(); for i in 0..10 {fs::create_dir(t.0.join(format!("folder{i}"))).unwrap();}
        let r=search_tree(&t.0,"folder",SearchLimits{max_results:3,..Default::default()}).unwrap();
        assert_eq!(r.paths.len(),3); assert!(r.truncated);
    }
    #[test] fn batch_rename_is_non_destructive_plan() {
        let t=Tmp::new();let a=t.0.join("a.pdf");let b=t.0.join("b.pdf");
        fs::write(&a,b"a").unwrap();fs::write(&b,b"b").unwrap();
        let plan=batch_rename_plan(&[a.clone(),b.clone()],"Fattura",1).unwrap();
        assert_eq!(plan[0].1.file_name().unwrap().to_string_lossy(),"Fattura 001.pdf");
        assert!(a.exists() && b.exists() && !plan[0].1.exists());
        fs::write(&plan[1].1,b"occupied").unwrap();
        assert_eq!(batch_rename_plan(&[a,b],"Fattura",1).unwrap_err().kind(),io::ErrorKind::AlreadyExists);
    }
    #[test] fn template_does_not_overwrite() {
        let t=Tmp::new();let a=t.0.join("template.txt");let b=t.0.join("new.txt");
        fs::write(&a,b"source").unwrap();create_from_template(&a,&b).unwrap();
        assert_eq!(fs::read(&b).unwrap(),b"source");
        assert!(create_from_template(&a,&b).is_err());
    }
    #[cfg(unix)]
    #[test] fn symlink_properties_do_not_follow() {
        let t=Tmp::new();let a=t.0.join("link");std::os::unix::fs::symlink("/etc",&a).unwrap();
        let p=properties(&a).unwrap();assert_eq!(p.kind,"Collegamento simbolico");
        assert_eq!(p.symlink_target,Some(PathBuf::from("/etc")));
    }
}
