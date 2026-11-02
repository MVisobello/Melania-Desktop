//! Run potentially slow NetworkManager/BlueZ subprocesses away from GTK.
//! Workers transfer only Send data; GTK widgets are touched on main context.
use gtk::glib;
use std::{sync::mpsc::{self,TryRecvError},time::{Duration,Instant}};

/// A user-triggered operation gets one worker and one short-lived GLib poll.
/// When a service hangs, stop waiting on the UI after four seconds; the
/// bounded worker continues independently but cannot later mutate widgets.
pub fn run<T:Send+'static>(
    service:&'static str,
    operation:impl FnOnce()->T+Send+'static,
    completed:impl FnOnce(Option<T>)+'static,
) {
    let (tx,rx)=mpsc::sync_channel(1);
    std::thread::spawn(move ||{
        let _=tx.send(operation());
    });
    let started=Instant::now();
    let mut completed=Some(completed);
    glib::timeout_add_local(Duration::from_millis(40),move ||{
        match rx.try_recv() {
            Ok(value)=>{
                if std::env::var_os("MELANIA_PROFILE_SETTINGS").is_some() {
                    eprintln!("MELANIA_SETTINGS_SERVICE_MS={} service={service} outcome=ok",
                        started.elapsed().as_millis());
                }
                if let Some(callback)=completed.take(){callback(Some(value));}
                glib::ControlFlow::Break
            },
            Err(TryRecvError::Disconnected)=>{
                if let Some(callback)=completed.take(){callback(None);}
                glib::ControlFlow::Break
            },
            Err(TryRecvError::Empty) if started.elapsed()>=Duration::from_secs(4)=>{
                if std::env::var_os("MELANIA_PROFILE_SETTINGS").is_some() {
                    eprintln!("MELANIA_SETTINGS_SERVICE_MS={} service={service} outcome=timeout",
                        started.elapsed().as_millis());
                }
                if let Some(callback)=completed.take(){callback(None);}
                glib::ControlFlow::Break
            },
            Err(TryRecvError::Empty)=>glib::ControlFlow::Continue,
        }
    });
}
