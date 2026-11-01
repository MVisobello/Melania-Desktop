//! GIO/GVfs backend shared by remote and local file operations.
//! Only URI strings and plain metadata are exchanged across worker threads.
//! Every GIO object is instantiated and used on the thread that owns it.
//! All COPY/MOVE operations refuse overwrites; failures are surfaced.
use gtk::gio;
use gtk::gio::prelude::*;
use std::{sync::{Arc, atomic::{AtomicBool, Ordering}},path::Path,time::{Duration,SystemTime,UNIX_EPOCH},io};
use melania_files_core as model;

const MAX_ITEMS: usize = 2_000;
const MAX_COPY_ENTRIES: usize = 10_000;
const MAX_COPY_DEPTH: u32 = 24;

#[derive(Clone, Debug)]
pub struct Item {
    pub uri: String,
    pub name: String,
    pub directory: bool,
    pub size: u64,
    pub content_type: String,
    pub modified: Option<SystemTime>,
    pub symlink: bool,
}

#[derive(Debug)]
pub struct Listing {
    pub items: Vec<Item>,
    pub truncated: bool,
}

fn error(message: impl Into<String>) -> String { message.into() }

/// Do not accept shell-like paths, arbitrary URI schemes, control characters,
/// or inline passwords in locations entered by the user. GVfs handles
/// credentials through the desktop authentication agent, never in the URL.
pub fn validate_uri(uri: &str) -> Result<(), String> {
    if uri.len() > 8_192 || uri.is_empty() || uri.chars().any(char::is_control) {
        return Err(error("Indirizzo non valido o troppo lungo"));
    }
    let (scheme, remainder) = uri.split_once("://")
        .ok_or_else(||error("Usa un URI, per esempio sftp://server/cartella"))?;
    if !matches!(scheme, "file"|"network"|"smb"|"sftp"|"dav"|"davs"|"ftp"|"mtp"|"gphoto2"|"afp"|"nfs") {
        return Err(error("Protocollo non supportato da Melania Files"));
    }
    if remainder.is_empty() || (!matches!(scheme,"file"|"network") && remainder.starts_with('/')) {
        return Err(error("Indirizzo di rete incompleto"));
    }
    let authority = remainder.split('/').next().unwrap_or("");
    if let Some((credentials, _)) = authority.rsplit_once('@') {
        if credentials.contains(':') {
            return Err(error("Non inserire password nell'indirizzo: usa l'autenticazione GVfs"));
        }
    }
    if !matches!(scheme,"file"|"network") && authority.is_empty() {
        return Err(error("Specifica un server o un dispositivo"));
    }
    Ok(())
}

pub fn list_uri(uri: &str, text_filter: &str, show_hidden: bool,
                cancelled: &Arc<AtomicBool>) -> Result<Listing,String> {
    list_uri_bounded(uri,text_filter,show_hidden,MAX_ITEMS,cancelled)
}
fn list_uri_bounded(uri:&str,text_filter:&str,show_hidden:bool,max_items:usize,
                    cancelled:&Arc<AtomicBool>)->Result<Listing,String> {
    validate_uri(uri)?;
    let location = gio::File::for_uri(uri);
    let enumerator = location.enumerate_children(
        "standard::name,standard::display-name,standard::type,standard::size,standard::is-hidden,standard::content-type,time::modified",
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        None::<&gio::Cancellable>
    ).map_err(|e|format!("Impossibile aprire la posizione: {e}. Controlla GVfs, rete e credenziali."))?;
    let filter = text_filter.trim().to_lowercase();
    let mut items = Vec::new();
    let mut truncated = false;
    loop {
        if cancelled.load(Ordering::Acquire) {return Err(error("Lettura annullata"));}
        let info = enumerator.next_file(None::<&gio::Cancellable>)
            .map_err(|e|format!("Errore durante la lettura dei file: {e}"))?;
        let Some(info) = info else {break};
        if !show_hidden && info.is_hidden() {continue;}
        let name = info.display_name().to_string();
        if !filter.is_empty() && !name.to_lowercase().contains(&filter) {continue;}
        if items.len() >= max_items {truncated=true;break;}
        let child = enumerator.child(&info);
        items.push(Item {
            uri: child.uri().to_string(),
            name,
            directory: info.file_type()==gio::FileType::Directory,
            size: info.size().max(0) as u64,
            content_type: info.content_type().map(|s|s.to_string()).unwrap_or_default(),
            modified: match info.attribute_uint64("time::modified") {
                0 => None,
                seconds => UNIX_EPOCH.checked_add(Duration::from_secs(seconds)),
            },
            symlink: info.file_type()==gio::FileType::SymbolicLink,
        });
    }
    items.sort_by(|a,b|b.directory.cmp(&a.directory)
        .then_with(||a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(Listing{items,truncated})
}

/// Local files and remote files now enter through exactly the same GIO
/// enumeration/metadata model. This adapter only retains a PathBuf for the
/// existing rich local GTK widgets and legacy optimized POSIX file actions.
/// All sorting uses the shared core comparator, so switching backend never
/// silently changes the user's selected sort order.
pub fn list_local_entries(dir:&Path,query:&str,options:model::ListingOptions,
                          cancel:&Arc<AtomicBool>)->io::Result<Vec<model::Entry>> {
    let uri=gio::File::for_path(dir).uri().to_string();
    let result=list_uri_bounded(&uri,query,options.show_hidden,options.max_entries,cancel)
        .map_err(|e|io::Error::new(io::ErrorKind::Other,e))?;
    let mut entries=Vec::with_capacity(result.items.len());
    for item in result.items {
        if cancel.load(Ordering::Acquire) {
            return Err(io::Error::new(io::ErrorKind::Interrupted,"Lettura annullata"));
        }
        let file=gio::File::for_uri(&item.uri);
        let Some(path)=file.path() else {continue;};
        let picture=!item.directory && model::is_picture(&path);
        if options.pictures_only && !item.directory && !picture {continue;}
        entries.push(model::Entry {
            path,
            name:item.name,
            is_directory:item.directory,
            is_picture:picture,
            is_symlink:item.symlink,
            size:item.size,
            modified:item.modified,
        });
    }
    model::sort_entries(&mut entries,options);
    Ok(entries)
}

fn copy_tree(src:&gio::File, dst:&gio::File, cancelled:&Arc<AtomicBool>,
             count:&mut usize, depth:u32) -> Result<(),String> {
    if cancelled.load(Ordering::Acquire) {return Err(error("Copia annullata; potrebbero rimanere elementi parziali alla destinazione"));}
    if depth > MAX_COPY_DEPTH || *count >= MAX_COPY_ENTRIES {
        return Err(error("Limiti di copia raggiunti; verifica la destinazione prima di riprovare"));
    }
    *count += 1;
    let kind = src.query_file_type(gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,None::<&gio::Cancellable>);
    if kind == gio::FileType::SymbolicLink {
        return Err(error("Copia di collegamenti simbolici non abilitata: nessuna dereferenziazione implicita"));
    }
    if dst.query_exists(None::<&gio::Cancellable>) {
        return Err(format!("Destinazione già presente: {}",dst.uri()));
    }
    if kind == gio::FileType::Directory {
        dst.make_directory(None::<&gio::Cancellable>)
            .map_err(|e|format!("Creazione cartella fallita {}: {e}",dst.uri()))?;
        let children=src.enumerate_children("standard::name,standard::type",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,None::<&gio::Cancellable>)
            .map_err(|e|format!("Impossibile leggere la sottocartella {}: {e}",src.uri()))?;
        while let Some(info)=children.next_file(None::<&gio::Cancellable>)
            .map_err(|e|format!("Lettura incompleta: {e}"))? {
            let name=info.name();
            let Some(name)=name.to_str() else {
                return Err(error("Nome file non UTF-8: copia interrotta senza modificare l'origine"));
            };
            let from=children.child(&info);
            let to=dst.child(name);
            copy_tree(&from,&to,cancelled,count,depth+1)?;
        }
    } else if kind == gio::FileType::Regular {
        let cancel=gio::Cancellable::new();
        let mut progress=|_current:i64,_total:i64| {
            if cancelled.load(Ordering::Acquire) {cancel.cancel();}
        };
        src.copy(dst,gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
            Some(&cancel),Some(&mut progress))
            .map_err(|e|format!("Copia di {} non riuscita: {e}",src.uri()))?;
    } else {
        return Err(error("Tipo di file speciale non supportato dalla copia remota"));
    }
    Ok(())
}

/// GIO works on both local files and GVfs remote providers. Never delete
/// source files, even when copying across devices; partial destinations are
/// deliberately retained for inspection rather than pretending to roll back.
pub fn copy_uris(sources:&[String],destination_uri:&str,
                 cancelled:&Arc<AtomicBool>) -> Result<usize,String> {
    validate_uri(destination_uri)?;
    if sources.is_empty() || sources.len()>100 {
        return Err(error("Seleziona da 1 a 100 elementi per trasferimento"));
    }
    let target=gio::File::for_uri(destination_uri);
    let mut count=0;
    for uri in sources {
        validate_uri(uri)?;
        let src=gio::File::for_uri(uri);
        let name=src.basename().ok_or_else(||error("Origine senza nome"))?;
        let name=name.to_str().ok_or_else(||error("Nome origine non UTF-8"))?;
        let dst=target.child(name);
        if dst.equal(&src) || src.has_prefix(&dst) || target.has_prefix(&src) {
            return Err(error("Non è possibile copiare una cartella dentro se stessa"));
        }
        copy_tree(&src,&dst,cancelled,&mut count,0)?;
    }
    Ok(count)
}

/// Exclusive creation on both POSIX and GVfs backends: never truncate or
/// overwrite an existing file, including targets seen through file:// URIs.
pub fn create_blank_file(parent_uri:&str,name:&str)->Result<(),String>{
    validate_uri(parent_uri)?;
    if name.is_empty() || matches!(name,"."|"..") ||
        name.contains('/') || name.chars().any(char::is_control){
        return Err(error("Nome file non valido"));
    }
    let target=gio::File::for_uri(parent_uri).child(name);
    let stream=target.create(gio::FileCreateFlags::NONE,None::<&gio::Cancellable>)
        .map_err(|e|format!("File non creato senza sovrascrivere: {e}"))?;
    stream.close(None::<&gio::Cancellable>)
        .map_err(|e|format!("File creato, ma non è stato possibile chiuderlo: {e}"))?;
    Ok(())
}

pub fn make_folder(parent_uri:&str, name:&str)->Result<(),String> {
    validate_uri(parent_uri)?;
    if name.is_empty() || name=="." || name==".." || name.contains('/')
        || name.contains('\\') || name.chars().any(char::is_control) {
        return Err(error("Nome cartella non valido"));
    }
    gio::File::for_uri(parent_uri).child(name)
        .make_directory(None::<&gio::Cancellable>)
        .map_err(|e|format!("Cartella non creata: {e}"))
}

pub fn rename_item(uri:&str,new_name:&str)->Result<String,String> {
    validate_uri(uri)?;
    if new_name.is_empty() || matches!(new_name,"."|"..")
        || new_name.contains('/') || new_name.chars().any(char::is_control) {
        return Err(error("Nome non valido"));
    }
    let src=gio::File::for_uri(uri);
    // On native Linux local files the existing renameat2(RENAME_NOREPLACE)
    // remains the stronger atomic guarantee. Use GIO URI discovery but do
    // not weaken local safety to a check-then-move sequence.
    if let Some(path)=src.path() {
        let updated=melania_transfer::rename_local(&path,new_name)
            .map_err(|e|format!("Rinomina locale annullata: {e}"))?;
        return Ok(gio::File::for_path(updated).uri().to_string());
    }
    let parent=src.parent().ok_or_else(||error("Posizione senza cartella padre"))?;
    let dst=parent.child(new_name);
    if dst.equal(&src) {return Ok(uri.to_owned());}
    if dst.query_exists(None::<&gio::Cancellable>) {return Err(error("Esiste già un elemento con questo nome"));}
    // FileCopyFlags::NONE never requests overwriting. A provider can still
    // reject a remote rename, in which case we leave the source untouched.
    src.move_(&dst,gio::FileCopyFlags::NONE,None::<&gio::Cancellable>,None)
        .map_err(|e|format!("Rinomina non riuscita: {e}"))?;
    Ok(dst.uri().to_string())
}

pub fn move_to_trash(uri:&str)->Result<(),String> {
    validate_uri(uri)?;
    gio::File::for_uri(uri).trash(None::<&gio::Cancellable>)
        .map_err(|e|format!("Cestino non disponibile per questa posizione: {e}; file non eliminato"))
}

/// GVfs URIs are stored separately from the existing legacy PathBuf
/// bookmarks. Changes never overwrite GNOME/Nautilus GTK bookmarks.
pub fn remote_bookmarks_path() -> Result<std::path::PathBuf,String> {
    let home=std::env::var_os("HOME").ok_or_else(||error("HOME non disponibile"))?;
    let cfg=std::env::var_os("XDG_CONFIG_HOME").filter(|p|!p.is_empty())
        .map(std::path::PathBuf::from).unwrap_or_else(||std::path::PathBuf::from(home).join(".config"));
    Ok(cfg.join("melania").join("gvfs-bookmarks"))
}
pub fn read_bookmarks(path:&std::path::Path)->Result<Vec<String>,String> {
    use std::fs;
    let data=match fs::read(path){
        Ok(data)=>data,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(Vec::new()),
        Err(e)=>return Err(format!("Impossibile leggere i preferiti di rete: {e}")),
    };
    if data.len()>128*1024{return Err(error("File preferiti troppo grande"));}
    let text=std::str::from_utf8(&data).map_err(|_|error("File preferiti non UTF-8"))?;
    let mut saved=Vec::new();
    for entry in text.lines().take(300) {
        if validate_uri(entry).is_ok() && !saved.iter().any(|v|v==entry) && saved.len()<200 {
            saved.push(entry.to_owned());
        }
    }
    Ok(saved)
}
pub fn write_bookmarks(path:&std::path::Path,items:&[String])->Result<(),String> {
    use std::{fs,io::Write};
    if items.len()>200{return Err(error("Troppi preferiti"));}
    for item in items {validate_uri(item)?;}
    let parent=path.parent().ok_or_else(||error("Percorso preferiti non valido"))?;
    fs::create_dir_all(parent).map_err(|e|format!("Cartella preferiti non creata: {e}"))?;
    let tmp=path.with_extension(format!("tmp.{}",std::process::id()));
    let result=(||->Result<(),String>{
        let mut out=fs::OpenOptions::new().write(true).create_new(true)
            .open(&tmp).map_err(|e|format!("Preferiti non salvati: {e}"))?;
        #[cfg(unix)]{
            use std::os::unix::fs::PermissionsExt;
            out.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|e|format!("Protezione preferiti fallita: {e}"))?;
        }
        for uri in items {
            out.write_all(uri.as_bytes()).and_then(|_|out.write_all(b"\n"))
                .map_err(|e|format!("Scrittura preferiti fallita: {e}"))?;
        }
        out.sync_all().map_err(|e|format!("Salvataggio preferiti fallito: {e}"))?;
        fs::rename(&tmp,path).map_err(|e|format!("Preferiti non aggiornati: {e}"))
    })();
    if result.is_err(){let _=fs::remove_file(&tmp);}
    result
}
pub fn add_bookmark(uri:&str)->Result<Vec<String>,String>{
    validate_uri(uri)?;
    let path=remote_bookmarks_path()?;
    let mut list=read_bookmarks(&path)?;
    if !list.iter().any(|v|v==uri){
        list.push(uri.to_owned());
        write_bookmarks(&path,&list)?;
    }
    Ok(list)
}
pub fn remove_bookmark(uri:&str)->Result<Vec<String>,String>{
    validate_uri(uri)?;
    let path=remote_bookmarks_path()?;
    let mut list=read_bookmarks(&path)?;
    list.retain(|v|v!=uri);
    write_bookmarks(&path,&list)?;
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs,path::PathBuf};
    #[test] fn remote_bookmarks_are_deduplicated_and_skip_invalid_entries() {
        let path=std::env::temp_dir().join(format!("melania-gvfs-bookmarks-{}",std::process::id()));
        let _=fs::remove_file(&path);
        write_bookmarks(&path,&["sftp://server/home".into(),"network:///".into()]).unwrap();
        let list=read_bookmarks(&path).unwrap();
        assert_eq!(list.len(),2);
        let _=fs::remove_file(&path);
    }
    #[test] fn local_gui_list_uses_gio_metadata_and_shared_sorting() {
        let dir=std::env::temp_dir().join(format!("melania-gio-local-{}",
            std::process::id()));
        let _=fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        fs::create_dir(dir.join("folder")).unwrap();
        fs::write(dir.join("a.txt"),b"1").unwrap();
        fs::write(dir.join("z.txt"),b"12345").unwrap();
        let stop=Arc::new(AtomicBool::new(false));
        let options=model::ListingOptions {
            sort_by:model::SortBy::Size,descending:true,..Default::default()
        };
        let entries=list_local_entries(&dir,"",options,&stop).unwrap();
        let names:Vec<_>=entries.iter().map(|e|e.name.as_str()).collect();
        assert_eq!(names,vec!["folder","z.txt","a.txt"]);
        #[cfg(unix)]{
            std::os::unix::fs::symlink("missing",dir.join("broken")).unwrap();
            let entries=list_local_entries(&dir,"",options,&stop).unwrap();
            assert!(entries.iter().any(|e|e.name=="broken" && e.is_symlink
                && !e.is_directory));
        }
        let pics=list_local_entries(&dir,"",model::ListingOptions {
            pictures_only:true,..Default::default()
        },&stop).unwrap();
        assert_eq!(pics.len(),1);
        assert_eq!(pics[0].name,"folder");
        let _=fs::remove_dir_all(dir);
    }
    #[test] fn cancelled_local_gio_enumeration_is_interruptible() {
        let dir=std::env::temp_dir().join(format!("melania-gio-cancel-{}",
            std::process::id()));
        let _=fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        let stop=Arc::new(AtomicBool::new(true));
        assert_eq!(list_local_entries(&dir,"",model::ListingOptions::default(),&stop)
            .unwrap_err().kind(),io::ErrorKind::Interrupted);
        let _=fs::remove_dir_all(dir);
    }
    #[test] fn shared_gio_creation_refuses_existing_local_file() {
        let dir=std::env::temp_dir().join(format!("melania-gio-create-{}",std::process::id()));
        let _=fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let uri=gio::File::for_path(&dir).uri().to_string();
        create_blank_file(&uri,"new.txt").unwrap();
        fs::write(dir.join("new.txt"),b"important").unwrap();
        assert!(create_blank_file(&uri,"new.txt").is_err());
        assert_eq!(fs::read(dir.join("new.txt")).unwrap(),b"important");
        make_folder(&uri,"subfolder").unwrap();
        assert!(make_folder(&uri,"subfolder").is_err());
        let _=fs::remove_dir_all(dir);
    }
    #[cfg(target_os="linux")]
    #[test] fn shared_gio_rename_uses_atomic_local_no_replace() {
        let dir=std::env::temp_dir().join(format!("melania-gio-rename-{}",std::process::id()));
        let _=fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("from.txt"),b"original").unwrap();
        fs::write(dir.join("to.txt"),b"keep").unwrap();
        let uri=gio::File::for_path(dir.join("from.txt")).uri().to_string();
        assert!(rename_item(&uri,"to.txt").is_err());
        assert_eq!(fs::read(dir.join("to.txt")).unwrap(),b"keep");
        assert_eq!(fs::read(dir.join("from.txt")).unwrap(),b"original");
        let next=rename_item(&uri,"renamed.txt").unwrap();
        assert_eq!(gio::File::for_uri(&next).path().unwrap(),dir.join("renamed.txt"));
        let _=fs::remove_dir_all(dir);
    }
    #[test] fn accepted_network_protocols_and_credentials_safety() {
        for uri in ["network:///","smb://server/Share","sftp://user@host/home",
                    "davs://example.org/my-folder","file:///tmp","mtp://device/"] {
            assert!(validate_uri(uri).is_ok(),"{uri}");
        }
        for uri in ["", "bash -c rm -rf /", "javascript://invalid",
                    "sftp://user:secret@example.org/", "smb:///nohost", "file:///hi\nthere"] {
            assert!(validate_uri(uri).is_err(),"{uri}");
        }
    }
    #[test] fn can_enumerate_local_through_same_gio_backend() {
        let dir=std::env::temp_dir().join(format!("melania-gio-{}",std::process::id()));
        let _=fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("example.txt"),b"test").unwrap();
        let file=gio::File::for_path(&dir);
        let result=list_uri(&file.uri(),"example",false,&Arc::new(AtomicBool::new(false))).unwrap();
        assert_eq!(result.items.len(),1);
        assert_eq!(result.items[0].name,"example.txt");
        let _=fs::remove_dir_all(dir);
    }
    #[test] fn gio_copy_never_overwrites_existing_local_file() {
        let root:PathBuf=std::env::temp_dir().join(format!("melania-gio-copy-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let from=root.join("from");let to=root.join("to");
        fs::create_dir_all(&from).unwrap();fs::create_dir_all(&to).unwrap();
        fs::write(from.join("a.txt"),b"source").unwrap();
        fs::write(to.join("a.txt"),b"already").unwrap();
        let src=gio::File::for_path(from.join("a.txt")).uri().to_string();
        let dst=gio::File::for_path(&to).uri().to_string();
        let result=copy_uris(&[src],&dst,&Arc::new(AtomicBool::new(false)));
        assert!(result.is_err());
        assert_eq!(fs::read(to.join("a.txt")).unwrap(),b"already");
        let _=fs::remove_dir_all(root);
    }
}
