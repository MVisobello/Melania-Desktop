use std::{env, path::PathBuf};
use melania_control::{request, current_workspace, Command, SOCKET_ENV};
fn main() {
    let args:Vec<_>=env::args().skip(1).collect();
    let cmd=match args.as_slice(){
        [s] if s=="list" => Command::List,
        [s] if s=="status" => Command::Status,
        [s,n] if s=="workspace" => {
            let Ok(n)=n.parse::<u8>() else {usage();return};
            if !(1..=4).contains(&n) {usage();return;}
            Command::Workspace(n)
        }
        [s,id,n] if s=="send" => {
            let (Ok(id),Ok(n))=(id.parse::<u64>(),n.parse::<u8>()) else {usage();return};
            if id==0 || !(1..=4).contains(&n) {usage();return;}
            Command::Send(id,n)
        },
        [s] if s=="quit" => Command::Quit,
        [s,id] if ["activate","minimize","close"].contains(&s.as_str()) => {
            let Ok(id)=id.parse::<u64>() else{usage();return};
            if id==0{usage();return}
            match s.as_str(){"activate"=>Command::Activate(id),"minimize"=>Command::Minimize(id),_=>Command::Close(id)}
        }
        _ => {usage();return}
    };
    let Some(path)=env::var_os(SOCKET_ENV) else{eprintln!("{SOCKET_ENV} non impostato: avvia Melania");std::process::exit(2)};
    if cmd==Command::Status {
        match current_workspace(&PathBuf::from(path)) {
            Ok(n)=>println!("Spazio attivo: {n}"),
            Err(e)=>{eprintln!("Melania IPC: {e}");std::process::exit(1)}
        }
        return;
    }
    match request(&PathBuf::from(path),cmd){
        Ok(rows)=>for row in rows{println!("{}\t{}\t{}\t{}",row.id,if row.active{"attiva"}else if row.minimized{"minimizzata"}else{"aperta"},row.app_id,row.title)},
        Err(e)=>{eprintln!("Melania IPC: {e}");std::process::exit(1)}
    }
}
fn usage(){eprintln!("Uso: melania-winctl list | status | workspace N | send ID N | activate ID | minimize ID | close ID | quit");std::process::exit(2)}
