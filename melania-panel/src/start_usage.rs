//! Local launch frequencies for the classic Start menu. No telemetry, command
//! lines, file paths, history of documents or network access is recorded.
use std::{collections::HashMap, fs, io::{self, Write}, path::Path, time::{SystemTime, UNIX_EPOCH}};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const MAX_BYTES:u64=32*1024;
const MAX_ENTRIES:usize=256;

pub fn load(path:&Path)->Vec<(String,u32)> {
    let Ok(meta)=fs::metadata(path) else{return Vec::new()};
    if !meta.is_file() || meta.len()>MAX_BYTES{return Vec::new();}
    let Ok(s)=fs::read_to_string(path) else{return Vec::new()};
    let mut entries=HashMap::new();
    for line in s.lines().take(MAX_ENTRIES+1) {
        let Some((id,number))=line.split_once('\t') else{continue};
        if !crate::start_history::valid_id(id){continue;}
        let Ok(n)=number.parse::<u32>() else{continue};
        if n>0 {entries.insert(id.to_owned(),n);}
        if entries.len()>=MAX_ENTRIES {break;}
    }
    let mut result:Vec<_>=entries.into_iter().collect();
    result.sort_by(|a,b|b.1.cmp(&a.1).then_with(||a.0.cmp(&b.0)));
    result
}
pub fn popular()->Vec<(String,u32)>{
    melania_config::app_usage_path().map(|p|load(&p)).unwrap_or_default()
}
pub fn record_at(path:&Path,id:&str)->io::Result<()>{
    if !crate::start_history::valid_id(id){
        return Err(io::Error::new(io::ErrorKind::InvalidInput,"Invalid application ID"));
    }
    let mut ranking=load(path);
    if let Some((_,count))=ranking.iter_mut().find(|(name,_)|name==id){
        *count=count.saturating_add(1);
    } else {ranking.push((id.to_owned(),1));}
    ranking.sort_by(|a,b|b.1.cmp(&a.1).then_with(||a.0.cmp(&b.0)));
    ranking.truncate(MAX_ENTRIES);
    let parent=path.parent().ok_or_else(||io::Error::new(io::ErrorKind::InvalidInput,"Missing parent"))?;
    fs::create_dir_all(parent)?;
    #[cfg(unix)]
    fs::set_permissions(parent,fs::Permissions::from_mode(0o700))?;
    let now=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let tmp=path.with_extension(format!("tmp-{}-{now}",std::process::id()));
    let result=(|| ->io::Result<()>{
        let mut opts=fs::OpenOptions::new();opts.write(true).create_new(true);
        #[cfg(unix)]
        opts.mode(0o600);
        let mut output=opts.open(&tmp)?;
        for (id,count) in ranking { writeln!(output,"{id}\t{count}")?; }
        output.sync_all()?;
        fs::rename(&tmp,path)?;
        Ok(())
    })();
    if result.is_err(){let _=fs::remove_file(&tmp);}
    result
}
pub fn record_launch(id:&str){
    if let Some(path)=melania_config::app_usage_path(){
        if let Err(error)=record_at(&path,id){
            eprintln!("Melania Start launch frequency: {error}");
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frequent_apps_sort_by_count_then_name(){
        let dir=std::env::temp_dir().join(format!("melania-frequency-{}-{:?}",std::process::id(),std::thread::current().id()));
        let file=dir.join("usage");
        for id in ["z.desktop","a.desktop","z.desktop","a.desktop","b.desktop","z.desktop"] {
            record_at(&file,id).unwrap();
        }
        assert_eq!(load(&file),vec![("z.desktop".into(),3),("a.desktop".into(),2),("b.desktop".into(),1)]);
        #[cfg(unix)]{
            assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777,0o600);
            assert_eq!(fs::metadata(&dir).unwrap().permissions().mode() & 0o777,0o700);
        }
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn invalid_ids_and_oversized_file_fail_closed(){
        let dir=std::env::temp_dir().join(format!("melania-frequency-bad-{}",std::process::id()));
        let path=dir.join("usage");
        assert!(record_at(&path,"/path/to/executable").is_err());
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path,"a".repeat((MAX_BYTES+1) as usize)).unwrap();
        assert!(load(&path).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }
}
