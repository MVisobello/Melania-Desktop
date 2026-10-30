//! Safe, conservative local filesystem transfers for Melania Files.
//!
//! No overwrites, no recursive self-copy, no following symlinks when copying.
//! All transfer entries are preflighted; newly created destinations are rolled
//! back on error. This crate intentionally doesn't handle remote GVfs URLs yet.
use std::{collections::HashSet, fs, io::{self, Write}, path::{Path, PathBuf}};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferPlan {
    pub entries: Vec<(PathBuf, PathBuf)>,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

/// Validate all sources/destinations before touching user data.
/// `destination` must already exist and be a directory.
pub fn plan_copy(sources: &[PathBuf], destination: &Path) -> io::Result<TransferPlan> {
    if sources.is_empty() { return Err(invalid("Nessun file da trasferire")); }
    if !destination.is_dir() { return Err(invalid("La destinazione non è una cartella")); }
    let dest_real = destination.canonicalize()?;
    let mut target_set = HashSet::new();
    let mut entries = Vec::with_capacity(sources.len());

    for source in sources {
        let meta = fs::symlink_metadata(source)?;
        if !(meta.is_dir() || meta.is_file() || meta.file_type().is_symlink()) {
            return Err(invalid("Tipo di file non supportato"));
        }
        let name = source.file_name().ok_or_else(|| invalid("Impossibile copiare la radice del filesystem"))?;
        let target = destination.join(name);
        // Resolve the source parent without following a symlink at the source itself.
        let parent = source.parent().filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new(".")).canonicalize()?;
        let real_source = parent.join(name);
        if meta.is_dir() && dest_real.starts_with(&real_source) {
            return Err(invalid("Impossibile copiare una cartella dentro se stessa"));
        }
        if target == *source || dest_real.join(name) == real_source {
            return Err(invalid("Origine e destinazione coincidono"));
        }
        if fs::symlink_metadata(&target).is_ok() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, format!("Esiste già: {}", target.display())));
        }
        if !target_set.insert(target.clone()) {
            return Err(invalid("Due origini producono lo stesso nome nella destinazione"));
        }
        entries.push((source.clone(), target));
    }
    Ok(TransferPlan { entries })
}

fn copy_tree(source: &Path, target: &Path, created_root: &mut bool) -> io::Result<()> {
    let meta = fs::symlink_metadata(source)?;
    if meta.file_type().is_symlink() {
        #[cfg(unix)] {
            let link = fs::read_link(source)?;
            std::os::unix::fs::symlink(link, target)?;
            *created_root = true;
            return Ok(());
        }
        #[cfg(not(unix))] {
            return Err(invalid("Collegamenti simbolici non supportati su questa piattaforma"));
        }
    }
    if meta.is_dir() {
        fs::create_dir(target)?;
        *created_root = true;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree(&entry.path(), &target.join(entry.file_name()), created_root)?;
        }
        fs::set_permissions(target, meta.permissions())?;
    } else if meta.is_file() {
        // CREATE_NEW prevents data loss even if another process creates the
        // destination after our preflight check.
        let mut input = fs::File::open(source)?;
        let mut output = fs::OpenOptions::new().write(true).create_new(true).open(target)?;
        *created_root = true;
        io::copy(&mut input, &mut output)?;
        output.flush()?;
        fs::set_permissions(target, meta.permissions())?;
    } else { return Err(invalid("File speciale non supportato")); }
    Ok(())
}

/// Synchronous copy, only suitable for small files during early prototype.
/// Returns paths of created entries; failure rolls back entries created by us.
pub fn copy_sources(sources: &[PathBuf], destination: &Path) -> io::Result<Vec<PathBuf>> {
    let plan = plan_copy(sources, destination)?;
    let mut created: Vec<PathBuf> = Vec::new();
    for (source, target) in plan.entries {
        // Only cleanup a target which we can establish did not exist before.
        let mut created_here = false;
        let copied = copy_tree(&source, &target, &mut created_here);
        if let Err(error) = copied {
            if created_here { remove_created(&target); }
            for path in created.iter().rev() { remove_created(path); }
            return Err(error);
        }
        created.push(target);
    }
    Ok(created)
}

fn remove_created(path: &Path) {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.is_dir() { let _ = fs::remove_dir_all(path); }
        else { let _ = fs::remove_file(path); }
    }
}

/// Capture identity before DnD starts, so an accepted MOVE never deletes a
/// file that was replaced or modified while it was being dragged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    size: u64,
    modified: Option<std::time::SystemTime>,
    #[cfg(unix)] device: u64,
    #[cfg(unix)] inode: u64,
}
pub fn source_identity(path: &Path) -> io::Result<SourceIdentity> {
    let meta = fs::symlink_metadata(path)?;
    #[cfg(unix)] use std::os::unix::fs::MetadataExt;
    Ok(SourceIdentity {
        size: meta.len(), modified: meta.modified().ok(),
        #[cfg(unix)] device: meta.dev(),
        #[cfg(unix)] inode: meta.ino(),
    })
}
/// Only call after the DnD protocol explicitly confirms a successful MOVE.
/// Does not follow symlinks when deleting symlink sources.
pub fn remove_confirmed_source(path: &Path, initial: &SourceIdentity) -> io::Result<()> {
    if &source_identity(path)? != initial {
        return Err(io::Error::new(io::ErrorKind::Other,
            "L'origine è cambiata durante il trascinamento; nessuna cancellazione"));
    }
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) }
}

/// Move local paths with atomic rename on the same filesystem; fails without
/// deleting originals when crossing devices. Cross-device moves are handled
/// by the DnD protocol as copy + deletion at the *source* after success.
pub fn move_local(source: &Path, destination: &Path) -> io::Result<PathBuf> {
    let plan = plan_copy(&[source.to_path_buf()], destination)?;
    let (_, target) = &plan.entries[0];
    #[cfg(target_os = "linux")]
    {
        use std::{ffi::CString, os::{raw::{c_char, c_int, c_uint}, unix::ffi::OsStrExt}};
        unsafe extern "C" {
            fn renameat2(oldfd: c_int, oldpath: *const c_char,
                         newfd: c_int, newpath: *const c_char,
                         flags: c_uint) -> c_int;
        }
        const AT_FDCWD: c_int = -100;
        const RENAME_NOREPLACE: c_uint = 1;
        let old = CString::new(source.as_os_str().as_bytes()).map_err(|_| invalid("Percorso origine non valido"))?;
        let new = CString::new(target.as_os_str().as_bytes()).map_err(|_| invalid("Percorso destinazione non valido"))?;
        // Atomic no-replace, unlike std::fs::rename which may overwrite.
        if unsafe { renameat2(AT_FDCWD, old.as_ptr(), AT_FDCWD, new.as_ptr(), RENAME_NOREPLACE) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(target.clone())
    }
    #[cfg(not(target_os = "linux"))]
    { Err(io::Error::new(io::ErrorKind::Unsupported, "Movimento atomico solo su Linux")) }
}

/// Rename in the same directory without replacing existing files.
/// In contrast to std::fs::rename, this operation is non-overwriting on Linux.
pub fn rename_local(source: &Path, new_name: &str) -> io::Result<PathBuf> {
    let name = new_name.trim();
    if name.is_empty() || matches!(name, "."|"..") || name.contains('/') || name.contains('\0') {
        return Err(invalid("Nome non valido"));
    }
    fs::symlink_metadata(source)?;
    let parent = source.parent().ok_or_else(|| invalid("Origine senza cartella"))?;
    let target = parent.join(name);
    if target == source { return Ok(target); }
    #[cfg(target_os = "linux")]
    {
        use std::{ffi::CString, os::{raw::{c_char,c_int,c_uint},unix::ffi::OsStrExt}};
        unsafe extern "C" {
            fn renameat2(oldfd:c_int,oldpath:*const c_char,newfd:c_int,newpath:*const c_char,flags:c_uint)->c_int;
        }
        const AT_FDCWD:c_int = -100;
        const RENAME_NOREPLACE:c_uint = 1;
        let old=CString::new(source.as_os_str().as_bytes()).map_err(|_|invalid("Origine non valida"))?;
        let new=CString::new(target.as_os_str().as_bytes()).map_err(|_|invalid("Destinazione non valida"))?;
        if unsafe {renameat2(AT_FDCWD,old.as_ptr(),AT_FDCWD,new.as_ptr(),RENAME_NOREPLACE)} != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(target)
    }
    #[cfg(not(target_os = "linux"))]
    { Err(io::Error::new(io::ErrorKind::Unsupported,"Rinomina atomica disponibile solo su Linux")) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::atomic::{AtomicUsize, Ordering}, time::{SystemTime, UNIX_EPOCH}};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let p = std::env::temp_dir().join(format!("melania-transfer-{}-{n}-{t}", std::process::id()));
            fs::create_dir(&p).unwrap(); Self(p)
        }
    }
    impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    #[test] fn copies_files_and_preserves_source() {
        let t = Temp::new(); let source = t.0.join("hello.txt");
        fs::write(&source, b"melania").unwrap(); let dest = t.0.join("dest"); fs::create_dir(&dest).unwrap();
        let created = copy_sources(&[source.clone()], &dest).unwrap();
        assert_eq!(created, vec![dest.join("hello.txt")]);
        assert_eq!(fs::read(&source).unwrap(), b"melania");
        assert_eq!(fs::read(&created[0]).unwrap(), b"melania");
    }
    #[test] fn refuses_overwrite_without_touching_any_source() {
        let t = Temp::new(); let src = t.0.join("x"); fs::write(&src,b"original").unwrap();
        let dest = t.0.join("dest"); fs::create_dir(&dest).unwrap(); fs::write(dest.join("x"),b"existing").unwrap();
        assert_eq!(copy_sources(&[src.clone()], &dest).unwrap_err().kind(),io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(dest.join("x")).unwrap(),b"existing");
        assert_eq!(fs::read(src).unwrap(),b"original");
    }
    #[test] fn refuses_copy_into_own_subdirectory() {
        let t=Temp::new(); let src=t.0.join("folder");fs::create_dir(&src).unwrap();
        let child=src.join("child"); fs::create_dir(&child).unwrap();
        assert!(plan_copy(&[src],&child).is_err());
    }
    #[test] fn copies_directory_recursively() {
        let t=Temp::new(); let src=t.0.join("folder");fs::create_dir(&src).unwrap();
        fs::create_dir(src.join("child")).unwrap();fs::write(src.join("child").join("note"),b"ok").unwrap();
        let dst=t.0.join("dest");fs::create_dir(&dst).unwrap();
        copy_sources(&[src],&dst).unwrap();
        assert_eq!(fs::read(dst.join("folder/child/note")).unwrap(),b"ok");
    }
    #[cfg(unix)]
    #[test] fn symlink_copied_without_following() {
        let t=Temp::new();let src=t.0.join("link");
        std::os::unix::fs::symlink("/etc",&src).unwrap();
        let dst=t.0.join("dest");fs::create_dir(&dst).unwrap();
        copy_sources(&[src],&dst).unwrap();
        assert_eq!(fs::read_link(dst.join("link")).unwrap(),PathBuf::from("/etc"));
    }
    #[test] fn duplicate_names_rejected_before_any_copy() {
        let t=Temp::new();let a=t.0.join("a");let b=t.0.join("b");let dest=t.0.join("dest");
        fs::create_dir(&a).unwrap();fs::create_dir(&b).unwrap();fs::create_dir(&dest).unwrap();
        fs::write(a.join("file"),"a").unwrap();fs::write(b.join("file"),"b").unwrap();
        assert!(copy_sources(&[a.join("file"),b.join("file")],&dest).is_err());
        assert!(!dest.join("file").exists());
    }
    #[test] fn confirmed_move_refuses_modified_origin() {
        let t=Temp::new(); let origin=t.0.join("test.txt");
        fs::write(&origin,"original").unwrap();
        let before=source_identity(&origin).unwrap();
        fs::write(&origin,"contenuto modificato").unwrap();
        assert!(remove_confirmed_source(&origin,&before).is_err());
        assert!(origin.exists());
    }
    #[test] fn no_source_deleted_on_copy() {
        let t=Temp::new(); let origin=t.0.join("source.txt");
        let dest=t.0.join("dest");fs::create_dir(&dest).unwrap();
        fs::write(&origin,"intatto").unwrap();
        let identity=source_identity(&origin).unwrap();
        copy_sources(&[origin.clone()],&dest).unwrap();
        assert!(origin.exists());
        // Only a drag-end MOVE would explicitly invoke remove_confirmed_source.
        assert_eq!(identity,source_identity(&origin).unwrap());
    }
    #[test] fn local_move_is_non_overwriting() {
        let t=Temp::new();let src=t.0.join("one");let dest=t.0.join("dest");fs::create_dir(&dest).unwrap();
        fs::write(&src,"ok").unwrap();let moved=move_local(&src,&dest).unwrap();
        assert_eq!(moved,dest.join("one"));assert!(!src.exists());
        assert_eq!(fs::read_to_string(moved).unwrap(),"ok");
    }
    #[test] fn move_refuses_overwrite_and_preserves_both_files() {
        let t=Temp::new();let src=t.0.join("note.txt");let dest=t.0.join("dest");
        fs::create_dir(&dest).unwrap();
        fs::write(&src,"original").unwrap();
        fs::write(dest.join("note.txt"),"existing").unwrap();
        assert_eq!(move_local(&src,&dest).unwrap_err().kind(),io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(&src).unwrap(),"original");
        assert_eq!(fs::read_to_string(dest.join("note.txt")).unwrap(),"existing");
    }
    #[test] fn refuses_move_folder_into_its_own_descendant() {
        let t=Temp::new();let src=t.0.join("folder");fs::create_dir(&src).unwrap();
        let child=src.join("child");fs::create_dir(&child).unwrap();
        assert!(move_local(&src,&child).is_err());
        assert!(src.exists());
    }
    #[test] fn refuses_missing_sources_without_touching_destination() {
        let t=Temp::new();let dest=t.0.join("dest");fs::create_dir(&dest).unwrap();
        assert!(copy_sources(&[t.0.join("missing")],&dest).is_err());
        assert_eq!(fs::read_dir(dest).unwrap().count(),0);
    }

    #[cfg(target_os="linux")]
    #[test] fn rename_is_atomic_and_preserves_destination() {
        let t=Temp::new();let from=t.0.join("a.txt");let existing=t.0.join("b.txt");
        fs::write(&from,b"old").unwrap();fs::write(&existing,b"keep").unwrap();
        assert_eq!(rename_local(&from,"b.txt").unwrap_err().kind(),io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&existing).unwrap(),b"keep");
        assert_eq!(fs::read(&from).unwrap(),b"old");
        assert_eq!(rename_local(&from,"c.txt").unwrap(),t.0.join("c.txt"));
        assert!(!from.exists());
        assert_eq!(fs::read(t.0.join("c.txt")).unwrap(),b"old");
    }
    #[test] fn rename_rejects_path_traversal() {
        let t=Temp::new();let f=t.0.join("file");fs::write(&f,b"safe").unwrap();
        for name in ["../danger","/tmp/danger","..",".",""] { assert!(rename_local(&f,name).is_err()); }
        assert_eq!(fs::read(&f).unwrap(),b"safe");
    }

}

pub mod background;
