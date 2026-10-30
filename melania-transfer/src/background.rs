//! Non-blocking, cancellable, no-overwrite local COPY jobs.
//!
//! A worker never holds a GTK object. Only progress events cross the channel.
//! Cancelling COPY removes the job's *new* output roots; originals are untouched.
//! Atomic Linux MOVE stays a separate operation: do not advertise MOVE over DnD
//! until the inter-application protocol is properly tested.
use crate::plan_copy;
use std::{fs, io::{self, Read, Write}, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, Ordering}, mpsc::{self, Receiver}}, thread};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Progress {
    pub completed_bytes: u64,
    pub total_bytes: u64,
    pub completed_items: u64,
    pub total_items: u64,
}
impl Progress {
    pub fn fraction(&self) -> f64 {
        if self.total_bytes > 0 {
            (self.completed_bytes as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
        } else if self.total_items > 0 {
            (self.completed_items as f64 / self.total_items as f64).clamp(0.0, 1.0)
        } else { 1.0 }
    }
}

#[derive(Debug)]
pub enum Event {
    Progress(Progress),
    Finished(Vec<PathBuf>),
    Cancelled,
    Failed(String),
}

pub struct Job {
    pub events: Receiver<Event>,
    pub cancelled: Arc<AtomicBool>,
}
impl Job {
    pub fn cancel(&self) { self.cancelled.store(true, Ordering::Release); }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn cancelled() -> io::Error { io::Error::new(io::ErrorKind::Interrupted, "Copia annullata") }
fn check(stop: &AtomicBool) -> io::Result<()> {
    if stop.load(Ordering::Acquire) { Err(cancelled()) } else { Ok(()) }
}

fn count_tree(source: &Path, total: &mut Progress, stop: &AtomicBool) -> io::Result<()> {
    check(stop)?;
    total.total_items = total.total_items.checked_add(1).ok_or_else(|| invalid("Troppi elementi"))?;
    if total.total_items > 200_000 { return Err(invalid("Limite di 200.000 elementi superato")); }
    let meta = fs::symlink_metadata(source)?;
    if meta.file_type().is_symlink() { return Ok(()); }
    if meta.is_file() {
        total.total_bytes = total.total_bytes.checked_add(meta.len()).ok_or_else(|| invalid("Dimensione troppo grande"))?;
    } else if meta.is_dir() {
        for entry in fs::read_dir(source)? { count_tree(&entry?.path(), total, stop)?; }
    } else { return Err(invalid("File speciale non supportato")); }
    Ok(())
}

fn emit(progress: &Progress, tx: &mpsc::Sender<Event>) { let _ = tx.send(Event::Progress(progress.clone())); }

fn copy_tree(
    source: &Path, destination: &Path, progress: &mut Progress,
    stop: &AtomicBool, tx: &mpsc::Sender<Event>, root_created: &mut bool,
) -> io::Result<()> {
    check(stop)?;
    let meta = fs::symlink_metadata(source)?;
    if meta.file_type().is_symlink() {
        #[cfg(unix)] {
            let link = fs::read_link(source)?;
            std::os::unix::fs::symlink(&link, destination)?;
            *root_created = true;
        }
        #[cfg(not(unix))] { return Err(invalid("Link simbolici non supportati")); }
    } else if meta.is_dir() {
        fs::create_dir(destination)?;
        *root_created = true;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree(&entry.path(), &destination.join(entry.file_name()), progress, stop, tx, root_created)?;
        }
        fs::set_permissions(destination, meta.permissions())?;
    } else if meta.is_file() {
        let mut input = fs::File::open(source)?;
        // create_new is essential: a concurrent process cannot be overwritten.
        let mut output = fs::OpenOptions::new().write(true).create_new(true).open(destination)?;
        *root_created = true;
        let mut buf = [0u8; 128 * 1024];
        let mut last_reported = progress.completed_bytes;
        loop {
            check(stop)?;
            let n = input.read(&mut buf)?;
            if n == 0 { break; }
            output.write_all(&buf[..n])?;
            progress.completed_bytes += n as u64;
            // Coalesce progress updates: avoid an unbounded queue of tiny
            // notifications for multi-gigabyte copies.
            if progress.completed_bytes.saturating_sub(last_reported) >= 1024 * 1024 {
                emit(progress, tx);
                last_reported = progress.completed_bytes;
            }
        }
        output.flush()?;
        fs::set_permissions(destination, meta.permissions())?;
    } else { return Err(invalid("File speciale non supportato")); }
    progress.completed_items += 1;
    emit(progress, tx);
    Ok(())
}

fn remove_new_root(root: &Path) {
    if let Ok(meta) = fs::symlink_metadata(root) {
        if meta.is_dir() { let _ = fs::remove_dir_all(root); }
        else { let _ = fs::remove_file(root); }
    }
}

/// Start after validating every destination. The worker performs all filesystem
/// I/O away from GTK's main thread, including pre-counting large directories.
/// Failure/cancellation rolls back only *new outputs created by this job*.
/// Never use this API for MOVE: removing originals after a partial copy is unsafe.
pub fn start_copy(sources: Vec<PathBuf>, destination: PathBuf) -> io::Result<Job> {
    let plan = plan_copy(&sources, &destination)?;
    let cancelled = Arc::new(AtomicBool::new(false));
    let stop = cancelled.clone();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut progress = Progress {completed_bytes:0,total_bytes:0,completed_items:0,total_items:0};
        let counted = plan.entries.iter().try_for_each(|(source, _)| count_tree(source, &mut progress, &stop));
        if let Err(err) = counted {
            let _ = tx.send(if err.kind() == io::ErrorKind::Interrupted { Event::Cancelled } else { Event::Failed(err.to_string()) });
            return;
        }
        emit(&progress, &tx);
        let mut created: Vec<PathBuf> = Vec::new();
        for (source, target) in plan.entries {
            let mut created_here = false;
            let result = copy_tree(&source, &target, &mut progress, &stop, &tx, &mut created_here);
            if let Err(err) = result {
                if created_here { remove_new_root(&target); }
                for p in created.iter().rev() { remove_new_root(p); }
                let _ = tx.send(if err.kind() == io::ErrorKind::Interrupted { Event::Cancelled } else { Event::Failed(err.to_string()) });
                return;
            }
            created.push(target);
        }
        // Cancellation arriving after the final write still cancels before success.
        if stop.load(Ordering::Acquire) {
            for p in created.iter().rev() { remove_new_root(p); }
            let _ = tx.send(Event::Cancelled);
        } else {
            let _ = tx.send(Event::Finished(created));
        }
    });
    Ok(Job { events: rx, cancelled })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    fn temporary() -> PathBuf {
        let n=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let p=std::env::temp_dir().join(format!("melania-bg-{}-{n}",std::process::id()));
        fs::create_dir(&p).unwrap();p
    }
    #[test] fn copies_files_and_reports_complete_progress() {
        let tmp=temporary(); let src=tmp.join("file.txt"); let dst=tmp.join("dest");fs::create_dir(&dst).unwrap();
        fs::write(&src,vec![17u8;250_000]).unwrap();
        let job=start_copy(vec![src.clone()],dst.clone()).unwrap();
        let mut completed=false;
        while let Ok(evt)=job.events.recv() {
            match evt {
                Event::Progress(p) => assert!(p.fraction() <= 1.0),
                Event::Finished(paths) => {assert_eq!(paths.len(),1);completed=true;break;},
                other => panic!("unexpected {other:?}"),
            }
        }
        assert!(completed); assert_eq!(fs::read(src).unwrap(),fs::read(dst.join("file.txt")).unwrap());
        fs::remove_dir_all(tmp).unwrap();
    }
    #[test] fn fails_without_overwriting() {
        let tmp=temporary();let src=tmp.join("x");let dst=tmp.join("dest");fs::create_dir(&dst).unwrap();
        fs::write(&src,b"new").unwrap();fs::write(dst.join("x"),b"original").unwrap();
        assert!(start_copy(vec![src],dst.clone()).is_err());
        assert_eq!(fs::read(dst.join("x")).unwrap(),b"original");fs::remove_dir_all(tmp).unwrap();
    }
    #[test] fn cancellation_preserves_source() {
        let tmp=temporary();let src=tmp.join("x");let dst=tmp.join("dest");fs::create_dir(&dst).unwrap();
        fs::write(&src,vec![27u8;4_000_000]).unwrap();
        let job=start_copy(vec![src.clone()],dst.clone()).unwrap();job.cancel();
        while let Ok(evt)=job.events.recv() {if matches!(evt,Event::Finished(_) | Event::Cancelled | Event::Failed(_)) {break;} }
        assert!(src.exists());
        // A cancellation racing with completion is allowed to leave a *complete* output.
        if dst.join("x").exists() {assert_eq!(fs::metadata(dst.join("x")).unwrap().len(),4_000_000);}
        fs::remove_dir_all(tmp).unwrap();
    }
    #[cfg(unix)]
    #[test] fn does_not_follow_symlink() {
        let tmp=temporary();let src=tmp.join("x");let dst=tmp.join("dest");fs::create_dir(&dst).unwrap();
        std::os::unix::fs::symlink("not-here",&src).unwrap();
        let job=start_copy(vec![src],dst.clone()).unwrap();
        while let Ok(evt)=job.events.recv() {if matches!(evt,Event::Finished(_) | Event::Cancelled | Event::Failed(_)) {break;} }
        assert_eq!(fs::read_link(dst.join("x")).unwrap(),PathBuf::from("not-here"));
        fs::remove_dir_all(tmp).unwrap();
    }
}
