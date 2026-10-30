//! CLI test harness for Melania's actual Rust transfer engine.
//! Without --apply, it only previews the operation: no files are changed.
use std::{env, error::Error, path::PathBuf, process};
use melania_transfer::{copy_sources, move_local, plan_copy};

fn usage() -> &'static str {
    "Uso: melania-filectl [--apply] copy DESTINAZIONE ORIGINE...\n     melania-filectl [--apply] move DESTINAZIONE ORIGINE\nSenza --apply: solo anteprima, senza modifiche. MOVE è atomico, solo nello stesso filesystem."
}
fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let (apply, rest): (bool, &[String]) = if args.first().map(String::as_str) == Some("--apply") {
        (true, &args[1..])
    } else { (false, &args) };
    if rest.len() < 3 { return Err(usage().into()); }
    let operation = rest[0].as_str();
    let dest = PathBuf::from(&rest[1]);
    let sources: Vec<PathBuf> = rest[2..].iter().map(PathBuf::from).collect();
    if operation == "move" && sources.len() != 1 {
        return Err("MOVE accetta esattamente un'origine per evitare trasferimenti parziali".into());
    }
    if operation != "copy" && operation != "move" {return Err(usage().into());}
    let plan = plan_copy(&sources, &dest)?;
    for (source, target) in &plan.entries {
        println!("{}  {}  ->  {}", operation.to_uppercase(),source.display(),target.display());
    }
    if !apply {println!("ANTEPRIMA: nessun dato modificato. Aggiungi --apply per eseguire."); return Ok(());}
    match operation {
        "copy" => {let created = copy_sources(&sources, &dest)?;println!("Copiati {} elementi",created.len());}
        "move" => {let moved = move_local(&sources[0], &dest)?;println!("Spostato in {}",moved.display());}
        _ => unreachable!(),
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {eprintln!("melania-filectl: {error}");process::exit(2);}
}
