//! Freedesktop D-Bus notification server. Run only in a Melania test session.
//! We intentionally do not steal org.freedesktop.Notifications from another DE.
use std::{collections::HashMap, error::Error, sync::{Arc, Mutex}};
use melania_notify_core::{publish, snapshot_path, Store};
use zbus::{blocking::connection::Builder, interface, object_server::SignalEmitter, zvariant::OwnedValue};

#[derive(Clone, Default)]
struct Notifications { state: Arc<Mutex<Store>> }

/// Private Melania API; applications use org.freedesktop.Notifications unchanged.
#[derive(Clone)]
struct NotificationCenter { state: Arc<Mutex<Store>> }

fn save_store(store: &Store) {
    if let Ok(path) = snapshot_path() {
        if let Err(error) = publish(&path, &store.snapshot()) {
            eprintln!("Melania notifyd: {error}");
        }
    }
}

#[interface(name = "org.melaniadesktop.NotificationCenter1")]
impl NotificationCenter {
    /// A read acknowledgment does NOT close a notification and emits no
    /// NotificationClosed signal. The original application owns its lifecycle.
    fn mark_all_read(&self) -> u32 {
        let mut store = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let changed = store.mark_all_read();
        if changed != 0 { save_store(&store); }
        changed
    }

    fn unread_count(&self) -> u32 {
        let store = self.state.lock().unwrap_or_else(|e| e.into_inner());
        store.unread_count() as u32
    }
}

#[interface(name = "org.freedesktop.Notifications")]
impl Notifications {
    fn get_capabilities(&self) -> Vec<String> {
        // No declared action or persistence support until those are implemented.
        vec!["body".into()]
    }
    fn get_server_information(&self) -> (String, String, String, String) {
        ("Melania Notifications".into(),"Melania Desktop".into(),"0.17".into(),"1.2".into())
    }
    fn notify(&self, app_name: &str, replaces_id: u32, _app_icon: &str,
              summary: &str, body: &str, _actions: Vec<String>,
              _hints: HashMap<String, OwnedValue>, _expire_timeout: i32) -> u32 {
        let mut store = self.state.lock().unwrap_or_else(|e|e.into_inner());
        let id = store.notify(replaces_id, app_name, summary, body);
        save_store(&store);
        id
    }
    async fn close_notification(&self, id: u32, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> zbus::fdo::Result<()> {
        let changed = {
            let mut store = self.state.lock().unwrap_or_else(|e|e.into_inner());
            let changed = store.close(id);
            if changed {
                save_store(&store);
            }
            changed
        };
        if changed {
            Self::notification_closed(&emitter, id, 3).await.map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        }
        Ok(())
    }
    #[zbus(signal)]
    async fn notification_closed(emitter: &SignalEmitter<'_>, id: u32, reason: u32) -> zbus::Result<()>;
}

fn main() -> Result<(),Box<dyn Error>> {
    let state = Notifications::default();
    if let Ok(path) = snapshot_path() {publish(&path,&state.state.lock().unwrap().snapshot())?;}
    let center = NotificationCenter { state: state.state.clone() };
    let _conn = Builder::session()?
        .serve_at("/org/melaniadesktop/NotificationCenter",center)?
        .serve_at("/org/freedesktop/Notifications",state)?
        .name("org.freedesktop.Notifications")?
        .build()?;
    println!("Melania notifications: org.freedesktop.Notifications attivo");
    // Keep connection alive; the zbus internal executor dispatches requests.
    loop {std::thread::park();}
}
