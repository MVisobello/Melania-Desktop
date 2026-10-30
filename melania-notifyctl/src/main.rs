//! CLI controls for the user notification service. No shell commands are run.
use std::{env, error::Error};
use zbus::blocking::{Connection, Proxy};

fn usage() {
    eprintln!("Uso: melania-notifyctl unread | mark-read | close ID");
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 { usage(); std::process::exit(2); }
    let conn = Connection::session()?;
    match args[1].as_str() {
        "unread" if args.len() == 2 => {
            let proxy = Proxy::new(&conn, "org.freedesktop.Notifications",
                "/org/melaniadesktop/NotificationCenter", "org.melaniadesktop.NotificationCenter1")?;
            let count: u32 = proxy.call("UnreadCount", &())?;
            println!("{count}");
        }
        "mark-read" if args.len() == 2 => {
            let proxy = Proxy::new(&conn, "org.freedesktop.Notifications",
                "/org/melaniadesktop/NotificationCenter", "org.melaniadesktop.NotificationCenter1")?;
            let count: u32 = proxy.call("MarkAllRead", &())?;
            println!("{count} notifiche segnate come lette");
        }
        "close" if args.len() == 3 => {
            let id = args[2].parse::<u32>()?;
            if id == 0 { return Err("ID non valido".into()); }
            let proxy = Proxy::new(&conn, "org.freedesktop.Notifications",
                "/org/freedesktop/Notifications", "org.freedesktop.Notifications")?;
            let _: () = proxy.call("CloseNotification", &(id,))?;
        }
        _ => { usage(); std::process::exit(2); }
    }
    Ok(())
}
