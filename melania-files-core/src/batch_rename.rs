//! Safe, user-confirmed multi-file rename for Melania Files.
//! The GUI supplies an atomic no-replace rename (Linux renameat2) callback.
//! This module never silently overwrites, and rolls back completed names on
//! a later error. Rollback failures are explicitly reported to the caller.
use std::{collections::HashSet, fs, io, path::{Path, PathBuf}};
use crate::nautilus_features::batch_rename_plan;

/// Re-check a preview before applying anything; views can become stale.
fn validate_plan(plan: &[(PathBuf, PathBuf)]) -> io::Result<()> {
    if plan.is_empty() || plan.len() > 500 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Seleziona da 1 a 500 elementi"));
    }
    let mut sources = HashSet::new();
    let mut destinations = HashSet::new();
    for (source, target) in plan {
        if source == target || source.parent() != target.parent()
            || !sources.insert(source.clone()) || !destinations.insert(target.clone()) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Piano di rinomina non valido"));
        }
        fs::symlink_metadata(source)?;
        // symlink_metadata also rejects occupied targets that are broken links.
        match fs::symlink_metadata(target) {
            Ok(_) => return Err(io::Error::new(io::ErrorKind::AlreadyExists,
                format!("Il nome {} è già occupato", target.display()))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Apply the same plan the user previewed. The callback MUST use a same-folder
/// atomic NOREPLACE operation, not std::fs::rename (which may overwrite).
/// Completed renames are reversed after failure, in the opposite order.
/// If another process creates a conflicting name during rollback, the error
/// includes the remaining paths and must be shown to the user.
pub fn apply_plan<F>(plan: &[(PathBuf, PathBuf)], mut rename_noreplace: F) -> io::Result<usize>
where F: FnMut(&Path, &Path) -> io::Result<()> {
    validate_plan(plan)?;
    let mut completed: Vec<(PathBuf, PathBuf)> = Vec::new();
    for (source, target) in plan {
        match rename_noreplace(source, target) {
            Ok(()) => completed.push((source.clone(), target.clone())),
            Err(error) => {
                let mut unresolved = Vec::new();
                let restored_count = completed.len();
                for (old, new) in completed.into_iter().rev() {
                    if let Err(rollback_error) = rename_noreplace(&new, &old) {
                        unresolved.push(format!("{} → {} ({rollback_error})", new.display(), old.display()));
                    }
                }
                let detail = if unresolved.is_empty() {
                    format!("{restored_count} elementi ripristinati; nessun file perso")
                } else {
                    format!("RIPRISTINO INCOMPLETO: {}. Non rinominare nuovamente prima di controllare i file", unresolved.join("; "))
                };
                return Err(io::Error::new(error.kind(),
                    format!("Rinomina interrotta su {}: {error}. {detail}", source.display())));
            }
        }
    }
    Ok(plan.len())
}

/// Re-plan from the actual files immediately before execution, never trust
/// the filenames cached when the confirmation window was opened.
pub fn rename_numbered<F>(sources: &[PathBuf], prefix: &str, start: u32, callback: F) -> io::Result<usize>
where F: FnMut(&Path, &Path) -> io::Result<()> {
    let plan = batch_rename_plan(sources, prefix, start)?;
    apply_plan(&plan, callback)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64,Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("melania-batch-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
            fs::create_dir(&dir).unwrap(); Self(dir)
        }
    }
    impl Drop for Temp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    fn no_replace(a: &Path, b: &Path) -> io::Result<()> {
        if fs::symlink_metadata(b).is_ok() {return Err(io::Error::new(io::ErrorKind::AlreadyExists,"occupied"));}
        fs::rename(a,b)
    }
    #[test] fn renames_preserve_file_contents_and_extensions() {
        let t=Temp::new();
        let a=t.0.join("a.pdf");let b=t.0.join("b.txt");
        fs::write(&a,b"first").unwrap();fs::write(&b,b"second").unwrap();
        let n=rename_numbered(&[a,b],"Fattura",1,no_replace).unwrap();
        assert_eq!(n,2);
        assert_eq!(fs::read(t.0.join("Fattura 001.pdf")).unwrap(),b"first");
        assert_eq!(fs::read(t.0.join("Fattura 002.txt")).unwrap(),b"second");
    }
    #[test] fn failure_reverses_all_prior_moves() {
        let t=Temp::new();
        let a=t.0.join("a.txt");let b=t.0.join("b.txt");
        fs::write(&a,b"one").unwrap();fs::write(&b,b"two").unwrap();
        let mut count=0;
        let err=rename_numbered(&[a.clone(),b.clone()],"Numero",1,|src,dst| {
            count+=1;
            if count==2 {return Err(io::Error::new(io::ErrorKind::PermissionDenied,"injected"));}
            no_replace(src,dst)
        }).unwrap_err();
        assert!(err.to_string().contains("ripristinati"));
        assert_eq!(fs::read(a).unwrap(),b"one");
        assert_eq!(fs::read(b).unwrap(),b"two");
        assert!(!t.0.join("Numero 001.txt").exists());
    }
    #[test] fn late_collision_is_not_overwritten() {
        let t=Temp::new();
        let a=t.0.join("a.txt");let b=t.0.join("b.txt");
        fs::write(&a,b"one").unwrap();fs::write(&b,b"two").unwrap();
        let plan=batch_rename_plan(&[a.clone(),b.clone()],"Numero",1).unwrap();
        fs::write(&plan[1].1,b"other").unwrap();
        assert_eq!(apply_plan(&plan,no_replace).unwrap_err().kind(),io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&plan[1].1).unwrap(),b"other");
        assert!(a.exists() && b.exists());
    }
    #[test] fn rollback_collision_is_reported() {
        let t=Temp::new();
        let a=t.0.join("a");let b=t.0.join("b");
        fs::write(&a,b"one").unwrap();fs::write(&b,b"two").unwrap();
        let mut attempt=0;
        let err=rename_numbered(&[a.clone(),b],"F",1,|src,dst| {
            attempt+=1;
            if attempt>1 { return Err(io::Error::new(io::ErrorKind::PermissionDenied,"blocked")); }
            no_replace(src,dst)
        }).unwrap_err();
        assert!(err.to_string().contains("RIPRISTINO INCOMPLETO"));
        assert!(t.0.join("F 001").exists());
        assert!(!a.exists());
    }
    #[cfg(unix)]
    #[test] fn broken_symlink_conflicts_block_rename() {
        let t=Temp::new();let a=t.0.join("a.txt");
        fs::write(&a,b"one").unwrap();
        std::os::unix::fs::symlink("does-not-exist",t.0.join("F 001.txt")).unwrap();
        assert_eq!(rename_numbered(&[a.clone()],"F",1,no_replace).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists);
        assert!(a.exists());
    }
}
