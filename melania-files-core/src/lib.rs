//! Filesystem model for Melania Files: bounded directory listings, predictable
//! filtering/sorting, and durable bookmarks. No GTK and no compositor required.
//! Never follow symlinks during directory classification or metadata inspection.
pub mod nautilus_features;
use std::{collections::HashSet, fs, io, path::{Path, PathBuf}, time::SystemTime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortBy { Name, Modified, Size, Type }
impl SortBy { pub fn from_index(i: u32) -> Self { match i { 1 => Self::Modified, 2 => Self::Size, 3 => Self::Type, _ => Self::Name } } }
#[derive(Clone, Copy, Debug)]
pub struct ListingOptions {
    pub show_hidden: bool,
    pub pictures_only: bool,
    pub sort_by: SortBy,
    pub descending: bool,
    pub max_entries: usize,
}
impl Default for ListingOptions {
    fn default() -> Self { Self { show_hidden: false, pictures_only: false, sort_by: SortBy::Name, descending: false, max_entries: 20_000 } }
}
#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub is_directory: bool,
    pub is_picture: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}
pub fn is_picture(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str())
        .is_some_and(|s| matches!(s.to_ascii_lowercase().as_str(), "png"|"jpg"|"jpeg"|"webp"|"gif"|"bmp"|"tiff"|"svg"|"avif"))
}
/// Files can disappear between read_dir and symlink_metadata (watchers,
/// concurrent renames and removable media). Skipping those entries prevents
/// one vanished file from blanking the whole directory listing.
fn entry_from_path(path:PathBuf,name:String)->io::Result<Option<Entry>> {
    let meta=match fs::symlink_metadata(&path) {
        Ok(metadata)=>metadata,
        Err(error) if matches!(error.kind(),
            io::ErrorKind::NotFound|io::ErrorKind::PermissionDenied)=>return Ok(None),
        Err(error)=>return Err(error),
    };
    let is_directory=meta.is_dir();
    Ok(Some(Entry {
        is_picture: !is_directory && is_picture(&path),
        is_symlink: meta.file_type().is_symlink(),
        size: meta.len(),
        modified: meta.modified().ok(),
        path,name,is_directory,
    }))
}

/// Returns only direct children; no unbounded recursive scan or symlink traversal.
/// An upper bound is essential for UI responsiveness and hostile directories.
pub fn list_directory(dir: &Path, query: &str, options: ListingOptions) -> io::Result<Vec<Entry>> {
    if options.max_entries == 0 { return Ok(Vec::new()); }
    let mut out = Vec::new();
    let q = query.trim().to_lowercase();
    for item in fs::read_dir(dir)? {
        let item=match item {
            Ok(entry)=>entry,
            Err(error) if matches!(error.kind(),
                io::ErrorKind::NotFound|io::ErrorKind::PermissionDenied)=>continue,
            Err(error)=>return Err(error),
        };
        let name = item.file_name().to_string_lossy().to_string();
        if !options.show_hidden && name.starts_with('.') { continue; }
        if !q.is_empty() && !name.to_lowercase().contains(&q) { continue; }
        let Some(entry)=entry_from_path(item.path(),name)? else {continue;};
        if options.pictures_only && !entry.is_directory && !entry.is_picture { continue; }
        out.push(entry);
        if out.len() >= options.max_entries { break; }
    }
    out.sort_by(|a,b| {
        if a.is_directory != b.is_directory { return b.is_directory.cmp(&a.is_directory); }
        let normal = a.name.to_lowercase().cmp(&b.name.to_lowercase());
        let order = match options.sort_by {
            SortBy::Name => normal,
            SortBy::Size => a.size.cmp(&b.size).then(normal),
            SortBy::Modified => a.modified.cmp(&b.modified).then(normal),
            SortBy::Type => {
                let suffix = |e:&Entry| {
                    if e.is_symlink {return String::from("collegamento");}
                    e.path.extension().and_then(|x|x.to_str())
                        .unwrap_or("").to_ascii_lowercase()
                };
                suffix(a).cmp(&suffix(b)).then(normal)
            },
        };
        if options.descending { order.reverse() } else { order }
    });
    Ok(out)
}

#[derive(Clone, Debug, Default)]
pub struct Bookmarks { paths: Vec<PathBuf> }
impl Bookmarks {
    pub fn paths(&self) -> &[PathBuf] { &self.paths }
    pub fn contains(&self, path: &Path) -> bool { self.paths.iter().any(|p| p == path) }
    pub fn add(&mut self, path: &Path) -> io::Result<bool> {
        if !path.is_dir() { return Err(io::Error::new(io::ErrorKind::InvalidInput,"Il preferito deve essere una cartella esistente")); }
        let absolute = path.canonicalize()?;
        if self.contains(&absolute) { return Ok(false); }
        if self.paths.len() >= 200 { return Err(io::Error::new(io::ErrorKind::InvalidInput,"Troppi preferiti")); }
        self.paths.push(absolute);
        Ok(true)
    }
    pub fn remove(&mut self,path:&Path) -> bool {
        let before=self.paths.len();self.paths.retain(|p|p!=path);before!=self.paths.len()
    }
    pub fn read_from(path:&Path) -> io::Result<Self> {
        let data = match fs::read(path) { Ok(d) => d, Err(e) if e.kind()==io::ErrorKind::NotFound => return Ok(Self::default()), Err(e)=>return Err(e) };
        if data.len()>128*1024 { return Err(io::Error::new(io::ErrorKind::InvalidData,"Preferiti troppo grandi")); }
        let content=std::str::from_utf8(&data).map_err(|_|io::Error::new(io::ErrorKind::InvalidData,"Preferiti non validi"))?;
        let mut paths=Vec::new(); let mut seen=HashSet::new();
        for line in content.lines().take(201) {
            if line.starts_with('#') || line.is_empty() {continue;}
            let Some(payload)=line.strip_prefix("hex:") else {continue;};
            let Some(bytes)=hex_decode(payload) else {continue;};
            let Some(p)=path_from_bytes(bytes) else {continue;};
            if p.is_absolute() && seen.insert(p.clone()) && paths.len()<200 {paths.push(p);}
        }
        Ok(Self { paths })
    }
    pub fn write_to(&self,path:&Path) -> io::Result<()> {
        let dir=path.parent().ok_or_else(||io::Error::new(io::ErrorKind::InvalidInput,"Percorso preferiti non valido"))?;
        fs::create_dir_all(dir)?;
        let temp=path.with_extension(format!("tmp.{}",std::process::id()));
        let result=(|| -> io::Result<()> {
            use io::Write;
            let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
            #[cfg(unix)] {use std::os::unix::fs::PermissionsExt;file.set_permissions(fs::Permissions::from_mode(0o600))?;}
            file.write_all(b"# Melania bookmarks v1\n")?;
            for path in &self.paths { if let Some(bytes)=path_bytes(path) {file.write_all(format!("hex:{}\n",hex_encode(&bytes)).as_bytes())?;} }
            file.sync_all()?;
            fs::rename(&temp,path)
        })();
        if result.is_err() { let _=fs::remove_file(&temp); }
        result
    }
}

fn hex_encode(bytes:&[u8])->String {
    const DIGITS:&[u8;16]=b"0123456789abcdef";
    let mut s=String::with_capacity(bytes.len()*2);
    for b in bytes {s.push(DIGITS[(b>>4) as usize] as char);s.push(DIGITS[(b&0xf) as usize] as char);}s
}
fn hex_decode(s:&str)->Option<Vec<u8>> {
    let bytes=s.as_bytes(); if bytes.len()%2!=0 || bytes.len()>16_384 {return None;}
    let digit=|b:u8|->Option<u8>{match b{b'0'..=b'9'=>Some(b-b'0'),b'a'..=b'f'=>Some(b-b'a'+10),b'A'..=b'F'=>Some(b-b'A'+10),_=>None}};
    bytes.chunks_exact(2).map(|p| Some(digit(p[0])?*16+digit(p[1])?)).collect()
}
#[cfg(unix)]
fn path_bytes(p:&Path)->Option<Vec<u8>> {use std::os::unix::ffi::OsStrExt;Some(p.as_os_str().as_bytes().to_vec())}
#[cfg(unix)]
fn path_from_bytes(p:Vec<u8>)->Option<PathBuf> {use std::{ffi::OsString,os::unix::ffi::OsStringExt};Some(PathBuf::from(OsString::from_vec(p)))}
#[cfg(not(unix))]
fn path_bytes(p:&Path)->Option<Vec<u8>> {Some(p.to_str()?.as_bytes().to_vec())}
#[cfg(not(unix))]
fn path_from_bytes(p:Vec<u8>)->Option<PathBuf> {String::from_utf8(p).ok().map(PathBuf::from)}

pub fn bookmarks_path()->io::Result<PathBuf> {
    let home=std::env::var_os("HOME").ok_or_else(||io::Error::new(io::ErrorKind::NotFound,"HOME mancante"))?;
    let cfg=std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(||PathBuf::from(home).join(".config"));
    Ok(cfg.join("melania").join("bookmarks"))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp { fn new()->Self {use std::sync::atomic::{AtomicU64,Ordering};static N:AtomicU64=AtomicU64::new(1);let p=std::env::temp_dir().join(format!("melania-files-core-{}-{}",std::process::id(),N.fetch_add(1,Ordering::Relaxed)));fs::create_dir(&p).unwrap();Self(p)} }
    impl Drop for Temp { fn drop(&mut self){let _=fs::remove_dir_all(&self.0);} }
    #[test] fn filters_and_folders_first() {let t=Temp::new();fs::create_dir(t.0.join("Z folder")).unwrap();fs::write(t.0.join("b.jpg"),b"x").unwrap();fs::write(t.0.join("a.txt"),b"xx").unwrap();fs::write(t.0.join(".secret"),b"z").unwrap();let l=list_directory(&t.0,"",ListingOptions::default()).unwrap();assert_eq!(l.len(),3);assert_eq!(l[0].name,"Z folder");let l=list_directory(&t.0,"",ListingOptions{pictures_only:true,..Default::default()}).unwrap();assert_eq!(l.len(),2);assert_eq!(l[1].name,"b.jpg");}
    #[test] fn sorts_by_size_desc() {let t=Temp::new();fs::write(t.0.join("small"),b"1").unwrap();fs::write(t.0.join("big"),b"123456").unwrap();let l=list_directory(&t.0,"",ListingOptions{sort_by:SortBy::Size,descending:true,..Default::default()}).unwrap();assert_eq!(l[0].name,"big");}
    #[test] fn type_sorting_uses_extension_without_following_links() {
        let t=Temp::new();
        fs::write(t.0.join("m.pdf"),b"%PDF").unwrap();
        fs::write(t.0.join("z.csv"),b"a,b").unwrap();
        fs::write(t.0.join("a.txt"),b"notes").unwrap();
        let files=list_directory(&t.0,"",ListingOptions {
            sort_by:SortBy::Type, ..Default::default()
        }).unwrap();
        let names:Vec<_>=files.iter().map(|e|e.name.as_str()).collect();
        assert_eq!(names,vec!["z.csv","m.pdf","a.txt"]);
        assert_eq!(SortBy::from_index(3),SortBy::Type);
    }
    #[test] fn query_is_case_insensitive() {let t=Temp::new();fs::write(t.0.join("Relazione.PDF"),b"a").unwrap();assert_eq!(list_directory(&t.0,"rela",ListingOptions::default()).unwrap().len(),1);}
    #[test] fn bounded_listing() {let t=Temp::new();for i in 0..10 {fs::write(t.0.join(i.to_string()),b"a").unwrap();}assert_eq!(list_directory(&t.0,"",ListingOptions{max_entries:3,..Default::default()}).unwrap().len(),3);}
    #[test] fn bookmarks_persist_and_deduplicate() {let t=Temp::new();let folder=t.0.join("la mia cartella");fs::create_dir(&folder).unwrap();let mut bm=Bookmarks::default();assert!(bm.add(&folder).unwrap());assert!(!bm.add(&folder).unwrap());let file=t.0.join("bookmarks");bm.write_to(&file).unwrap();let mut restored=Bookmarks::read_from(&file).unwrap();assert_eq!(restored.paths(),&[folder.clone()]);assert!(restored.remove(&folder));assert!(restored.paths().is_empty());}
    #[test] fn rejects_file_bookmark() {let t=Temp::new();let f=t.0.join("documento");fs::write(&f,b"x").unwrap();assert!(Bookmarks::default().add(&f).is_err());}
    #[test] fn corrupted_bookmarks_do_not_inject_paths() {let t=Temp::new();let f=t.0.join("bookmarks");fs::write(&f,"hex:xyz\nhttps://example.com\nhex:2f746d70\n").unwrap();let bm=Bookmarks::read_from(&f).unwrap();assert_eq!(bm.paths(),&[PathBuf::from("/tmp")]);}
    #[test] fn vanished_entry_is_skipped_without_a_directory_error() {
        let t=Temp::new();
        let missing=t.0.join("vanished-document.txt");
        assert!(entry_from_path(missing,"vanished-document.txt".into()).unwrap().is_none());
        fs::write(t.0.join("still-here.txt"),b"content").unwrap();
        let entries=list_directory(&t.0,"",ListingOptions::default()).unwrap();
        assert_eq!(entries.len(),1);
        assert_eq!(entries[0].name,"still-here.txt");
    }
    #[test] fn symlink_not_followed() {#[cfg(unix)]{let t=Temp::new();std::os::unix::fs::symlink("/tmp",t.0.join("pointer")).unwrap();let l=list_directory(&t.0,"",ListingOptions::default()).unwrap();assert!(l[0].is_symlink);assert!(!l[0].is_directory);}}
}
