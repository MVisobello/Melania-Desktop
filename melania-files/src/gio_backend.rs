//! GIO/GVfs backend shared by remote and local file operations.
//! Only URI strings and plain metadata are exchanged across worker threads.
//! Every GIO object is instantiated and used on the thread that owns it.
//! All COPY/MOVE operations refuse overwrites; failures are surfaced.
use gtk::gio;
use gtk::gio::prelude::*;
use std::{sync::{Arc, atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering}},path::Path,time::{Duration,SystemTime,UNIX_EPOCH},io,collections::VecDeque};
use melania_files_core as model;

const MAX_ITEMS: usize = 2_000;
const MAX_COPY_ENTRIES: usize = 10_000;
const MAX_COPY_DEPTH: u32 = 24;

/// GIO's cancellable is explicitly designed for cross-thread cancellation.
/// The existing UI owns an Arc<AtomicBool>; a single scoped watcher bridges
/// that flag to the provider's native cancellable *while* I/O is blocked.
/// A single observer serves a whole listing/recursive copy, never per entry.
/// Always join the observer; it cannot leak after the provider returns.
fn with_gio_cancel<T>(
    cancelled:&Arc<AtomicBool>,
    work:impl FnOnce(&gio::Cancellable)->Result<T,String>,
)->Result<T,String>{
    if cancelled.load(Ordering::Acquire){
        return Err(error("Operazione annullata prima dell'accesso al provider"));
    }
    let cancellable=gio::Cancellable::new();
    std::thread::scope(|scope|{
        let (done_tx,done_rx)=std::sync::mpsc::sync_channel::<()>(1);
        let watcher=&cancellable;
        scope.spawn(move ||{
            loop {
                if cancelled.load(Ordering::Acquire){
                    watcher.cancel();
                    break;
                }
                match done_rx.recv_timeout(Duration::from_millis(25)){
                    Ok(())|Err(std::sync::mpsc::RecvTimeoutError::Disconnected)=>break,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout)=>{},
                }
            }
        });
        let result=work(&cancellable);
        let _=done_tx.send(());
        result
    })
}


/// Progress shared with the GTK main thread without sending a GTK/GIO object
/// across threads. A byte count is honest even for GVfs streams that cannot
/// determine the total size in advance: no fabricated percentage or ETA.
#[derive(Default,Debug)]
pub struct CopyProgress {
    bytes: AtomicU64,
    finished_items: AtomicUsize,
}
impl CopyProgress {
    pub fn snapshot(&self)->(u64,usize){
        (self.bytes.load(Ordering::Relaxed),
         self.finished_items.load(Ordering::Relaxed))
    }
}


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
    list_uri_sorted(uri,text_filter,show_hidden,model::SortBy::Name,false,cancelled)
}

/// Same GIO metadata, sort semantics and folder-first ordering on any provider.
/// No extra network queries are made when the user changes the sort mode.
pub fn sort_items(items: &mut [Item], by:model::SortBy, descending:bool) {
    items.sort_by(|a,b|{
        if a.directory!=b.directory{return b.directory.cmp(&a.directory);}
        let normal=a.name.to_lowercase().cmp(&b.name.to_lowercase());
        let order=match by {
            model::SortBy::Name=>normal,
            model::SortBy::Size=>a.size.cmp(&b.size).then(normal),
            model::SortBy::Modified=>a.modified.cmp(&b.modified).then(normal),
            model::SortBy::Type=>a.content_type.cmp(&b.content_type).then(normal),
        };
        if descending{order.reverse()}else{order}
    });
}

pub fn list_uri_sorted(uri:&str,text_filter:&str,show_hidden:bool,
    sort:model::SortBy,descending:bool,cancelled:&Arc<AtomicBool>)->Result<Listing,String>{
    let mut listing=list_uri_bounded(uri,text_filter,show_hidden,MAX_ITEMS,cancelled)?;
    sort_items(&mut listing.items,sort,descending);
    Ok(listing)
}
fn list_uri_bounded(uri:&str,text_filter:&str,show_hidden:bool,max_items:usize,
                    cancelled:&Arc<AtomicBool>)->Result<Listing,String> {
    validate_uri(uri)?;
    // A superseded request must not even start a blocking GVfs call.
    if cancelled.load(Ordering::Acquire){
        return Err(error("Lettura annullata"));
    }
    with_gio_cancel(cancelled, |provider| {
    let location = gio::File::for_uri(uri);
    let enumerator = location.enumerate_children(
        "standard::name,standard::display-name,standard::type,standard::size,standard::is-hidden,standard::content-type,time::modified",
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        Some(provider)
    ).map_err(|e|format!("Impossibile aprire la posizione: {e}. Controlla GVfs, rete e credenziali."))?;
    let filter = text_filter.trim().to_lowercase();
    let mut items = Vec::new();
    let mut truncated = false;
    loop {
        if cancelled.load(Ordering::Acquire) {return Err(error("Lettura annullata"));}
        let info = enumerator.next_file(Some(provider))
            .map_err(|e|format!("Errore durante la lettura dei file: {e}"))?;
        let Some(info) = info else {break};
        if !show_hidden && (info.is_hidden() || info.name().to_string_lossy().starts_with('.')) {continue;}
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
    sort_items(&mut items,model::SortBy::Name,false);
    Ok(Listing{items,truncated})
    })
}

const MAX_QUICKLOOK_BYTES:usize=64*1024;
#[derive(Debug)]
pub struct TextQuickLook {
    pub content:String,
    pub mime:String,
    pub bytes_read:usize,
    pub truncated:bool,
}
fn supported_text_mime(mime:&str)->bool{
    mime.starts_with("text/") ||
    matches!(mime,"application/json"|"application/xml"|
        "application/javascript"|"application/x-yaml"|"application/yaml"|
        "application/x-shellscript"|"application/x-javascript")
}
/// Read-only, explicitly requested Quick Look. Never follow symlinks, load
/// arbitrary binary files or fetch more than 64 KiB + 1 byte over GVfs.
/// All provider calls accept the same cancellation used by the remote UI.
pub fn text_quicklook(uri:&str,cancelled:&Arc<AtomicBool>)
    ->Result<TextQuickLook,String>{
    validate_uri(uri)?;
    with_gio_cancel(cancelled,|provider|{
        let file=gio::File::for_uri(uri);
        let info=file.query_info("standard::type,standard::content-type",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,Some(provider))
            .map_err(|e|format!("Anteprima non disponibile: {e}"))?;
        if info.file_type()!=gio::FileType::Regular{
            return Err(error("L'anteprima supporta solo file di testo regolari, non cartelle o collegamenti."));
        }
        let mime=info.content_type().map(|value|value.to_string())
            .unwrap_or_default();
        if !supported_text_mime(&mime){
            return Err(error("Anteprima non disponibile per questo tipo di file: aprilo con l'app appropriata."));
        }
        let stream=file.read(Some(provider))
            .map_err(|e|format!("Impossibile leggere l'anteprima: {e}"))?;
        let mut bytes=Vec::with_capacity(MAX_QUICKLOOK_BYTES+1);
        loop {
            if cancelled.load(Ordering::Acquire) || provider.is_cancelled(){
                return Err(error("Anteprima annullata"));
            }
            if bytes.len()>=MAX_QUICKLOOK_BYTES+1{break;}
            let remaining=MAX_QUICKLOOK_BYTES+1-bytes.len();
            let mut block=[0u8;8192];
            let count=stream.read(&mut block[..remaining.min(block.len())],Some(provider))
                .map_err(|e|format!("Anteprima interrotta: {e}"))?;
            if count==0{break;}
            bytes.extend_from_slice(&block[..count]);
        }
        stream.close(Some(provider))
            .map_err(|e|format!("Impossibile chiudere il flusso di anteprima: {e}"))?;
        if bytes.contains(&0u8){
            return Err(error("Il file contiene dati binari: anteprima testuale rifiutata"));
        }
        let truncated=bytes.len()>MAX_QUICKLOOK_BYTES;
        let length=bytes.len().min(MAX_QUICKLOOK_BYTES);
        // A 64 KiB boundary can land mid-codepoint; lossiness is confined to
        // displayed text. The original file is never modified.
        let content=String::from_utf8_lossy(&bytes[..length]).into_owned();
        Ok(TextQuickLook{content,mime,bytes_read:length,truncated})
    })
}

#[derive(Clone,Debug)]
pub struct FileDetails {
    pub name:String,
    pub content_type:String,
    pub size:Option<u64>,
    pub modified:Option<SystemTime>,
    pub is_directory:bool,
    pub symlink:bool,
    pub can_read:bool,
    pub can_write:bool,
    pub can_execute:bool,
}

/// Metadata-only properties for both local and remote Gio::File resources.
/// No file contents or recursive directory enumeration are needed.
pub fn query_details(uri:&str)->Result<FileDetails,String> {
    validate_uri(uri)?;
    let file=gio::File::for_uri(uri);
    let info=file.query_info(
        "standard::display-name,standard::type,standard::size,standard::content-type,standard::is-symlink,time::modified,access::can-read,access::can-write,access::can-execute",
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,None::<&gio::Cancellable>)
        .map_err(|e|format!("Impossibile leggere le proprietà: {e}"))?;
    let dir=info.file_type()==gio::FileType::Directory;
    Ok(FileDetails {
        name:info.display_name().to_string(),
        content_type:info.content_type().map(|s|s.to_string())
            .unwrap_or_else(||"Sconosciuto".to_owned()),
        size:if dir{None}else{Some(info.size().max(0) as u64)},
        modified:match info.attribute_uint64("time::modified"){
            0=>None,secs=>UNIX_EPOCH.checked_add(Duration::from_secs(secs))
        },
        is_directory:dir,
        symlink:info.file_type()==gio::FileType::SymbolicLink || info.is_symlink(),
        can_read:info.boolean("access::can-read"),
        can_write:info.boolean("access::can-write"),
        can_execute:info.boolean("access::can-execute"),
    })
}

const MAX_REMOTE_SEARCH_DEPTH:u8=8;
const MAX_REMOTE_SEARCH_ITEMS:usize=5_000;
const MAX_REMOTE_SEARCH_FOLDERS:usize=300;
const MAX_REMOTE_SEARCH_RESULTS:usize=300;

#[derive(Clone,Debug)]
pub struct SearchHit {
    pub uri:String,
    pub name:String,
    pub directory:bool,
}
#[derive(Debug)]
pub struct SearchReport {
    pub hits:Vec<SearchHit>,
    pub scanned:usize,
    pub skipped_folders:usize,
    pub limited:bool,
}
#[derive(Debug,Default)]
pub struct SearchProgress {
    scanned:AtomicUsize,
    found:AtomicUsize,
}
impl SearchProgress {
    pub fn snapshot(&self)->(usize,usize){
        (self.scanned.load(Ordering::Relaxed),self.found.load(Ordering::Relaxed))
    }
}

/// Explicit, name-only remote search. The server sees only bounded GIO
/// directory enumerations; file bodies are never read. A single Gio::Cancellable
/// spans the entire operation and can interrupt a blocked remote read.
pub fn search_tree(uri:&str,query:&str,hidden:bool,
    cancelled:&Arc<AtomicBool>,progress:Option<&SearchProgress>)->Result<SearchReport,String>{
    validate_uri(uri)?;
    let needle=query.trim().to_lowercase();
    if needle.chars().count()<2 || needle.chars().count()>120{
        return Err(error("Cerca un nome di almeno 2 e massimo 120 caratteri"));
    }
    with_gio_cancel(cancelled,|provider|{
        let mut queue=VecDeque::from([(gio::File::for_uri(uri),0u8)]);
        let mut report=SearchReport {
            hits:Vec::new(),scanned:0,skipped_folders:0,limited:false
        };
        let mut folders=0usize;
        'walk: while let Some((folder,depth))=queue.pop_front() {
            if cancelled.load(Ordering::Acquire) || provider.is_cancelled(){
                return Err(error("Ricerca GVfs annullata"));
            }
            if folders>=MAX_REMOTE_SEARCH_FOLDERS{
                report.limited=true;
                break;
            }
            folders+=1;
            let listing=folder.enumerate_children(
                "standard::name,standard::display-name,standard::type,standard::is-hidden",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,Some(provider));
            let enumerator=match listing{
                Ok(value)=>value,
                Err(e) if folders==1=>return Err(format!(
                    "Impossibile cercare nella cartella iniziale: {e}")),
                Err(_)=>{report.skipped_folders+=1;continue;}
            };
            loop {
                if cancelled.load(Ordering::Acquire) || provider.is_cancelled(){
                    return Err(error("Ricerca GVfs annullata"));
                }
                let info=match enumerator.next_file(Some(provider)){
                    Ok(Some(item))=>item,
                    Ok(None)=>break,
                    Err(e)=>return Err(format!("Ricerca interrotta dal provider: {e}")),
                };
                if report.scanned>=MAX_REMOTE_SEARCH_ITEMS{
                    report.limited=true;
                    break 'walk;
                }
                report.scanned+=1;
                if let Some(p)=progress {p.scanned.store(report.scanned,Ordering::Relaxed);}
                if !hidden && (info.is_hidden() || info.name().to_string_lossy().starts_with('.')){
                    continue;
                }
                // NOFOLLOW_SYMLINKS applies to both traversal and metadata;
                // never descend into symlinks even if their target is a folder.
                let is_directory=info.file_type()==gio::FileType::Directory;
                let child=enumerator.child(&info);
                let name=info.display_name().to_string();
                if name.to_lowercase().contains(&needle){
                    if report.hits.len()>=MAX_REMOTE_SEARCH_RESULTS{
                        report.limited=true;
                        break 'walk;
                    }
                    report.hits.push(SearchHit {
                        uri:child.uri().to_string(),name,directory:is_directory,
                    });
                    if let Some(p)=progress {p.found.store(report.hits.len(),Ordering::Relaxed);}
                }
                if is_directory{
                    if depth<MAX_REMOTE_SEARCH_DEPTH{
                        queue.push_back((child,depth+1));
                    }else{
                        report.limited=true;
                    }
                }
            }
        }
        Ok(report)
    })
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
        .map_err(|e|{
            let kind=if cancel.load(Ordering::Acquire){
                io::ErrorKind::Interrupted
            }else{
                io::ErrorKind::Other
            };
            io::Error::new(kind,e)
        })?;
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
             provider:&gio::Cancellable,count:&mut usize, depth:u32,
             tracked:Option<&CopyProgress>) -> Result<(),String> {
    if cancelled.load(Ordering::Acquire) {return Err(error("Copia annullata; potrebbero rimanere elementi parziali alla destinazione"));}
    if depth > MAX_COPY_DEPTH || *count >= MAX_COPY_ENTRIES {
        return Err(error("Limiti di copia raggiunti; verifica la destinazione prima di riprovare"));
    }
    *count += 1;
    let kind = src.query_file_type(gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,Some(provider));
    if kind == gio::FileType::SymbolicLink {
        return Err(error("Copia di collegamenti simbolici non abilitata: nessuna dereferenziazione implicita"));
    }
    if dst.query_exists(Some(provider)) {
        return Err(format!("Destinazione già presente: {}",dst.uri()));
    }
    if kind == gio::FileType::Directory {
        dst.make_directory(Some(provider))
            .map_err(|e|format!("Creazione cartella fallita {}: {e}",dst.uri()))?;
        let children=src.enumerate_children("standard::name,standard::type",
            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,Some(provider))
            .map_err(|e|format!("Impossibile leggere la sottocartella {}: {e}",src.uri()))?;
        while let Some(info)=children.next_file(Some(provider))
            .map_err(|e|format!("Lettura incompleta: {e}"))? {
            let name=info.name();
            let Some(name)=name.to_str() else {
                return Err(error("Nome file non UTF-8: copia interrotta senza modificare l'origine"));
            };
            let from=children.child(&info);
            let to=dst.child(name);
            copy_tree(&from,&to,cancelled,provider,count,depth+1,tracked)?;
        }
    } else if kind == gio::FileType::Regular {
        let mut last_bytes=0u64;
        let mut progress=|current:i64,_total:i64| {
            let done=current.max(0) as u64;
            let delta=done.saturating_sub(last_bytes);
            last_bytes=last_bytes.max(done);
            if let Some(counter)=tracked {
                counter.bytes.fetch_add(delta,Ordering::Relaxed);
            }
        };
        src.copy(dst,gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
            Some(provider),Some(&mut progress))
            .map_err(|e|format!("Copia di {} non riuscita: {e}",src.uri()))?;
    } else {
        return Err(error("Tipo di file speciale non supportato dalla copia remota"));
    }
    if let Some(counter)=tracked {
        counter.finished_items.fetch_add(1,Ordering::Relaxed);
    }
    Ok(())
}

/// GIO works on both local files and GVfs remote providers. Never delete
/// source files, even when copying across devices; partial destinations are
/// deliberately retained for inspection rather than pretending to roll back.
pub fn copy_uris(sources:&[String],destination_uri:&str,
                 cancelled:&Arc<AtomicBool>) -> Result<usize,String> {
    copy_uris_with_progress(sources,destination_uri,cancelled,None)
}

/// Optional live progress is used by the remote GTK copy surface. Existing
/// call sites keep their previous interface and safety semantics.
pub fn copy_uris_with_progress(sources:&[String],destination_uri:&str,
                 cancelled:&Arc<AtomicBool>,tracked:Option<&CopyProgress>) -> Result<usize,String> {
    validate_uri(destination_uri)?;
    if sources.is_empty() || sources.len()>100 {
        return Err(error("Seleziona da 1 a 100 elementi per trasferimento"));
    }
    with_gio_cancel(cancelled, |provider|{
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
        copy_tree(&src,&dst,cancelled,provider,&mut count,0,tracked)?;
    }
    Ok(count)
    })
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

/// A single reversible rename only, in memory. The token comes from the
/// provider's id::file and id::filesystem attributes, and must survive the
/// original rename. Never infer identity from names, sizes or timestamps.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct RenameReceipt {
    pub from_uri:String,
    pub to_uri:String,
    file_id:String,
    filesystem_id:String,
}
fn rename_identity(uri:&str)->Option<(String,String)> {
    let file=gio::File::for_uri(uri);
    let info=file.query_info("id::file,id::filesystem",
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        None::<&gio::Cancellable>).ok()?;
    let id=info.attribute_string("id::file")?.to_string();
    let fs=info.attribute_string("id::filesystem")?.to_string();
    if id.is_empty() || fs.is_empty(){return None;}
    Some((id,fs))
}

/// Rename always remains available. Undo is offered only if this GVfs
/// provider reports a stable identity before AND after the operation.
pub fn rename_with_receipt(uri:&str,name:&str)
    ->Result<(String,Option<RenameReceipt>),String>{
    validate_uri(uri)?;
    let before=rename_identity(uri);
    let updated=rename_item(uri,name)?;
    let receipt=before.and_then(|(file_id,filesystem_id)|{
        let (new_id,new_fs)=rename_identity(&updated)?;
        if new_id!=file_id || new_fs!=filesystem_id || updated==uri {
            return None;
        }
        Some(RenameReceipt{
            from_uri:uri.to_string(),to_uri:updated.clone(),
            file_id,filesystem_id,
        })
    });
    Ok((updated,receipt))
}

/// No overwrite is requested. Refuse replay when the source disappeared or
/// now points at another object. On Linux local paths use the existing
/// atomic RENAME_NOREPLACE implementation; GVfs providers are best-effort
/// and should be tested on their actual server implementations.
pub fn replay_rename(receipt:&RenameReceipt,undo:bool)->Result<(),String>{
    validate_uri(&receipt.from_uri)?;
    validate_uri(&receipt.to_uri)?;
    let (source,target)=if undo{
        (&receipt.to_uri,&receipt.from_uri)
    }else{
        (&receipt.from_uri,&receipt.to_uri)
    };
    let identity=rename_identity(source).ok_or_else(||error(
        "Annulla/Ripeti non disponibile: il provider non fornisce un'identità verificabile"))?;
    if identity.0!=receipt.file_id || identity.1!=receipt.filesystem_id{
        return Err(error("Operazione interrotta: il file è cambiato dopo la rinomina"));
    }
    let src=gio::File::for_uri(source);
    let dest=gio::File::for_uri(target);
    if dest.query_exists(None::<&gio::Cancellable>){
        return Err(error("Operazione interrotta: il nome di destinazione è già occupato"));
    }
    let name=dest.basename().and_then(|os|os.to_str().map(str::to_owned))
        .ok_or_else(||error("Nome di destinazione non valido"))?;
    let actual=rename_item(source,&name)?;
    if !gio::File::for_uri(&actual).equal(&dest) {
        return Err(error("Operazione completata dal provider in un percorso inatteso"));
    }
    // If the provider changed its object ID across this rename, the caller
    // discards history rather than blindly enabling another replay.
    if rename_identity(&actual).as_ref()!=Some(&identity){
        return Err(error("Rinomina eseguita ma identità cambiata: ripetizione disabilitata"));
    }
    Ok(())
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
    #[test] fn remote_sort_folders_first_and_all_sort_variants() {
        let mut rows=vec![
            Item{uri:"file:///b".into(),name:"B".into(),directory:false,size:2,
                content_type:"text/plain".into(),modified:None,symlink:false},
            Item{uri:"file:///d".into(),name:"Directory".into(),directory:true,size:0,
                content_type:"inode/directory".into(),modified:None,symlink:false},
            Item{uri:"file:///a".into(),name:"A".into(),directory:false,size:20,
                content_type:"application/zip".into(),modified:None,symlink:false}
        ];
        sort_items(&mut rows,model::SortBy::Size,true);
        assert_eq!(rows.iter().map(|e|e.name.as_str()).collect::<Vec<_>>(),
            vec!["Directory","A","B"]);
        sort_items(&mut rows,model::SortBy::Type,false);
        assert_eq!(rows.iter().map(|e|e.name.as_str()).collect::<Vec<_>>(),
            vec!["Directory","A","B"]);
        sort_items(&mut rows,model::SortBy::Name,false);
        assert_eq!(rows.iter().map(|e|e.name.as_str()).collect::<Vec<_>>(),
            vec!["Directory","A","B"]);
    }
    #[test] fn details_are_non_recursive_and_file_symlink_safe() {
        let dir=std::env::temp_dir().join(format!("melania-gio-details-{}",
            std::process::id()));
        let _=fs::remove_dir_all(&dir);
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("file.txt"),b"hello world").unwrap();
        let details=query_details(&gio::File::for_path(dir.join("file.txt")).uri()).unwrap();
        assert_eq!(details.size,Some(11));
        assert!(!details.is_directory);
        #[cfg(unix)]{
            std::os::unix::fs::symlink("missing",dir.join("dangling")).unwrap();
            let link=query_details(&gio::File::for_path(dir.join("dangling")).uri()).unwrap();
            assert!(link.symlink);
        }
        let folder=query_details(&gio::File::for_path(&dir).uri()).unwrap();
        assert!(folder.is_directory);
        assert_eq!(folder.size,None);
        let _=fs::remove_dir_all(dir);
    }
    #[test] fn bounded_remote_search_finds_nested_names_without_following_links(){
        let dir=std::env::temp_dir().join(format!("melania-gvfs-search-{}",std::process::id()));
        let _=fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("nested")).unwrap();
        fs::write(dir.join("nested").join("needle.txt"),b"ok").unwrap();
        fs::write(dir.join(".needle-secret"),b"hidden").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("nested"),dir.join("shortcut")).unwrap();
        let uri=gio::File::for_path(&dir).uri().to_string();
        let stop=Arc::new(AtomicBool::new(false));
        let prog=SearchProgress::default();
        let report=search_tree(&uri,"needle",false,&stop,Some(&prog)).unwrap();
        assert_eq!(report.hits.len(),1);
        assert_eq!(report.hits[0].name,"needle.txt");
        assert!(!report.limited);
        assert_eq!(prog.snapshot().1,1);
        let hidden=search_tree(&uri,"needle",true,&stop,None).unwrap();
        assert_eq!(hidden.hits.len(),2);
        let _=fs::remove_dir_all(dir);
    }
    #[test] fn remote_search_stops_before_provider_io_when_pre_cancelled(){
        let stop=Arc::new(AtomicBool::new(true));
        let result=search_tree("sftp://offline.invalid/share","target",false,&stop,None);
        assert!(result.unwrap_err().contains("annullata"));
        assert!(search_tree("file:///tmp","a",false,
            &Arc::new(AtomicBool::new(false)),None).is_err());
    }
    /// Requires a dedicated, explicitly mounted SFTP fixture. This is never
    /// run in normal developer builds; CI provides an isolated loopback server.
    #[test]
    #[ignore = "requires an isolated GVfs SFTP server and authenticated session"]
    fn live_gvfs_sftp_loopback_fixture(){
        let root=std::env::var("MELANIA_TEST_SFTP_URI")
            .expect("Test SFTP disabilitato: impostare MELANIA_TEST_SFTP_URI");
        assert!(root.starts_with("sftp://"),"Test richiesto su un backend SFTP reale");
        validate_uri(&root).unwrap();
        let stop=Arc::new(AtomicBool::new(false));
        let listing=list_uri_sorted(&root,"melania-sftp-fixture",false,
            model::SortBy::Name,false,&stop).unwrap();
        assert_eq!(listing.items.len(),1,"La cartella SFTP isolata deve avere il fixture");
        let file=&listing.items[0];
        assert_eq!(file.name,"melania-sftp-fixture.txt");
        assert!(!file.directory);
        let details=query_details(&file.uri).unwrap();
        assert_eq!(details.size,Some(20));
        let target=gio::File::for_uri(&root).child("target").uri().to_string();
        let before=copy_uris(&[file.uri.clone()],&target,&stop).unwrap();
        assert_eq!(before,1);
        let copied=list_uri_sorted(&target,"melania-sftp-fixture",false,
            model::SortBy::Name,false,&stop).unwrap();
        assert_eq!(copied.items.len(),1);
        assert_eq!(query_details(&copied.items[0].uri).unwrap().size,Some(20));
        assert!(copy_uris(&[file.uri.clone()],&target,&stop).is_err(),
            "A second SFTP copy must refuse overwrite");
        assert_eq!(query_details(&file.uri).unwrap().size,Some(20));
    }
    #[test] fn text_quicklook_reads_only_bounded_utf8_and_refuses_binary(){
        let root=std::env::temp_dir().join(format!(
            "melania-quicklook-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("hello.txt"),"Ciao, Procida!\n").unwrap();
        let cancel=Arc::new(AtomicBool::new(false));
        let uri=gio::File::for_path(root.join("hello.txt")).uri().to_string();
        let p=text_quicklook(&uri,&cancel).unwrap();
        assert_eq!(p.content,"Ciao, Procida!\n");
        assert!(!p.truncated);
        fs::write(root.join("large.txt"),vec![b'a';MAX_QUICKLOOK_BYTES+1234]).unwrap();
        let large=text_quicklook(&gio::File::for_path(root.join("large.txt"))
            .uri(),&cancel).unwrap();
        assert_eq!(large.bytes_read,MAX_QUICKLOOK_BYTES);
        assert!(large.truncated);
        fs::write(root.join("binary.txt"),b"AB\\0CD").unwrap();
        assert!(text_quicklook(&gio::File::for_path(root.join("binary.txt"))
            .uri(),&cancel).is_err());
        let _=fs::remove_dir_all(root);
    }
    #[test] fn quicklook_is_cancelled_without_remote_provider_access(){
        let cancelled=Arc::new(AtomicBool::new(true));
        let result=text_quicklook("sftp://unreachable.invalid/example.txt",&cancelled);
        assert!(result.unwrap_err().contains("annullata"));
    }
    #[test] fn local_recorded_rename_can_undo_and_redo_without_overwrite(){
        let root=std::env::temp_dir().join(format!(
            "melania-undo-rename-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("before.txt"),b"original").unwrap();
        let before=gio::File::for_path(root.join("before.txt")).uri().to_string();
        let (_,receipt)=rename_with_receipt(&before,"after.txt").unwrap();
        let receipt=receipt.expect("Local POSIX identity must be stable");
        assert!(root.join("after.txt").exists());
        replay_rename(&receipt,true).unwrap();
        assert_eq!(fs::read(root.join("before.txt")).unwrap(),b"original");
        replay_rename(&receipt,false).unwrap();
        assert_eq!(fs::read(root.join("after.txt")).unwrap(),b"original");
        let _=fs::remove_dir_all(root);
    }
    #[test] fn undo_rename_never_overwrites_a_new_destination(){
        let root=std::env::temp_dir().join(format!(
            "melania-undo-collision-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("before.txt"),b"original").unwrap();
        let uri=gio::File::for_path(root.join("before.txt")).uri().to_string();
        let (_,receipt)=rename_with_receipt(&uri,"after.txt").unwrap();
        let receipt=receipt.expect("Local POSIX identity must be stable");
        fs::write(root.join("before.txt"),b"new content").unwrap();
        assert!(replay_rename(&receipt,true).is_err());
        assert_eq!(fs::read(root.join("before.txt")).unwrap(),b"new content");
        assert_eq!(fs::read(root.join("after.txt")).unwrap(),b"original");
        let _=fs::remove_dir_all(root);
    }
    #[test] fn undo_refuses_changed_identity_at_renamed_path(){
        let root=std::env::temp_dir().join(format!(
            "melania-undo-stale-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("before.txt"),b"original").unwrap();
        let uri=gio::File::for_path(root.join("before.txt")).uri().to_string();
        let (_,receipt)=rename_with_receipt(&uri,"after.txt").unwrap();
        let receipt=receipt.expect("Local POSIX identity must be stable");
        fs::rename(root.join("after.txt"),root.join("elsewhere.txt")).unwrap();
        fs::write(root.join("after.txt"),b"new unrelated file").unwrap();
        assert!(replay_rename(&receipt,true).is_err());
        assert!(!root.join("before.txt").exists());
        assert_eq!(fs::read(root.join("after.txt")).unwrap(),b"new unrelated file");
        let _=fs::remove_dir_all(root);
    }
    #[test] fn native_gio_cancellable_interrupts_a_provider_operation() {
        use std::sync::mpsc;
        let cancelled=Arc::new(AtomicBool::new(false));
        let stop=cancelled.clone();
        let (ready,started)=mpsc::sync_channel(1);
        let worker=std::thread::spawn(move||{
            with_gio_cancel(&stop, |provider|{
                ready.send(()).unwrap();
                let start=std::time::Instant::now();
                while !provider.is_cancelled() &&
                    start.elapsed()<Duration::from_secs(2){
                    std::thread::sleep(Duration::from_millis(5));
                }
                if provider.is_cancelled(){Ok(())}
                else{Err("Provider non interrotto".into())}
            })
        });
        started.recv_timeout(Duration::from_secs(1)).unwrap();
        cancelled.store(true,Ordering::Release);
        worker.join().unwrap().unwrap();
    }
    #[test] fn cancellation_before_recursive_copy_writes_nothing() {
        let dir=std::env::temp_dir().join(format!("melania-cancel-copy-{}",std::process::id()));
        let _=fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("source")).unwrap();
        fs::create_dir_all(dir.join("target")).unwrap();
        fs::write(dir.join("source").join("a.txt"),b"original").unwrap();
        let sources=vec![gio::File::for_path(dir.join("source").join("a.txt")).uri().to_string()];
        let dest=gio::File::for_path(dir.join("target")).uri().to_string();
        let flag=Arc::new(AtomicBool::new(true));
        assert!(copy_uris(&sources,&dest,&flag).is_err());
        assert!(!dir.join("target").join("a.txt").exists());
        assert_eq!(fs::read(dir.join("source").join("a.txt")).unwrap(),b"original");
        let _=fs::remove_dir_all(dir);
    }
    #[test] fn superseded_request_never_starts_remote_enumeration() {
        let stopped=Arc::new(AtomicBool::new(true));
        let result=list_uri_sorted("sftp://unreachable.invalid/share","",false,
            model::SortBy::Name,false,&stopped);
        assert!(result.unwrap_err().contains("annullata"));
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
    #[test] fn gio_copy_reports_real_bytes_and_finished_items() {
        let root=std::env::temp_dir().join(format!("melania-gio-progress-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("from")).unwrap();
        fs::create_dir_all(root.join("to")).unwrap();
        let payload=vec![42u8;1024*128];
        fs::write(root.join("from").join("sample.bin"),&payload).unwrap();
        let src=gio::File::for_path(root.join("from").join("sample.bin")).uri().to_string();
        let dst=gio::File::for_path(root.join("to")).uri().to_string();
        let progress=CopyProgress::default();
        assert_eq!(copy_uris_with_progress(&[src],&dst,
            &Arc::new(AtomicBool::new(false)),Some(&progress)).unwrap(),1);
        assert_eq!(progress.snapshot(),(payload.len() as u64,1));
        assert_eq!(fs::read(root.join("to").join("sample.bin")).unwrap(),payload);
        let _=fs::remove_dir_all(root);
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
