//! Host-safe session controls for the Start menu.
//! Melania currently runs as a nested compositor. Never execute a host-wide
//! power command from that session, including in graphical smoke tests.
//! A future display-manager-owned session must explicitly opt in.
use std::{env, path::PathBuf, process::Command};
use melania_control::{request, Command as ControlCommand, SOCKET_ENV};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action { PowerOff, Restart, Hibernate, Suspend, Logout, Lock }

impl Action {
    pub const ALL: [Self; 6] = [
        Self::PowerOff, Self::Restart, Self::Hibernate,
        Self::Suspend, Self::Logout, Self::Lock,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::PowerOff => "Spegni",
            Self::Restart => "Riavvia",
            Self::Hibernate => "Iberna",
            Self::Suspend => "Sospendi",
            Self::Logout => "Disconnetti",
            Self::Lock => "Blocca schermo",
        }
    }
    fn logind_method(self) -> Option<&'static str> {
        match self {
            Self::PowerOff => Some("CanPowerOff"),
            Self::Restart => Some("CanReboot"),
            Self::Hibernate => Some("CanHibernate"),
            Self::Suspend => Some("CanSuspend"),
            Self::Logout | Self::Lock => None,
        }
    }
    fn systemctl_command(self) -> Option<&'static str> {
        match self {
            Self::PowerOff => Some("poweroff"),
            Self::Restart => Some("reboot"),
            Self::Hibernate => Some("hibernate"),
            Self::Suspend => Some("suspend"),
            Self::Logout | Self::Lock => None,
        }
    }
}

/// Only a future standalone session, explicitly launched with this flag by
/// its display-manager integration, may request changes to the host state.
fn host_actions_enabled() -> bool {
    env::var("MELANIA_ALLOW_HOST_POWER_ACTIONS").as_deref() == Ok("1")
        && env::var("MELANIA_NESTED_SESSION").as_deref() != Ok("1")
        && env::var("MELANIA_SMOKE_OPEN_START").as_deref() != Ok("1")
}

fn parse_logind_capability(answer: &str) -> Result<(), String> {
    // busctl prints e.g. `s "yes"`, `s "challenge"`, `s "no"`, `s "na"`.
    match answer.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["s", "\"yes\""] | ["s", "yes"] => Ok(()),
        ["s", "\"challenge\""] | ["s", "challenge"] =>
            Err("Il sistema richiede un'autorizzazione aggiuntiva.".into()),
        ["s", "\"no\""] | ["s", "no"] =>
            Err("Operazione non autorizzata per questa sessione.".into()),
        ["s", "\"na\""] | ["s", "na"] =>
            Err("Funzione non supportata da questo computer.".into()),
        _ => Err("Impossibile verificare il supporto tramite logind.".into()),
    }
}

pub fn availability(action: Action) -> Result<(), String> {
    if action == Action::Logout {
        return env::var_os(SOCKET_ENV)
            .filter(|path| !path.is_empty())
            .map(|_| ())
            .ok_or_else(|| "Controllo della sessione Melania non disponibile.".into());
    }
    if action == Action::Lock {
        return Err("Blocco schermo non ancora implementato nel compositore Melania.".into());
    }
    if !host_actions_enabled() {
        return Err("Disponibile soltanto in una futura sessione Melania nativa; l'anteprima annidata non modifica il computer ospitante.".into());
    }
    let method = action.logind_method().ok_or("Azione sconosciuta.")?;
    let result = Command::new("busctl").args([
        "--system", "call", "org.freedesktop.login1", "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager", method,
    ]).output().map_err(|e| format!("Impossibile interrogare logind: {e}"))?;
    if !result.status.success() {
        return Err("Il servizio systemd-logind non risponde.".into());
    }
    parse_logind_capability(&String::from_utf8_lossy(&result.stdout))
}

/// Call only after an explicit user click AND a confirmation dialog.
pub fn perform(action: Action) -> Result<(), String> {
    availability(action)?;
    if action == Action::Logout {
        let path = env::var_os(SOCKET_ENV)
            .map(PathBuf::from).ok_or("Socket della sessione mancante.")?;
        return request(&path, ControlCommand::Quit)
            .map(|_| ()).map_err(|e| format!("Disconnessione non riuscita: {e}"));
    }
    let command = action.systemctl_command().ok_or("Azione non ancora supportata.")?;
    let result = Command::new("systemctl").arg(command).output()
        .map_err(|e| format!("Impossibile avviare systemctl: {e}"))?;
    if result.status.success() { Ok(()) }
    else {
        let details = String::from_utf8_lossy(&result.stderr);
        Err(format!("Operazione rifiutata: {}", details.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_action_has_a_distinct_label() {
        let names: Vec<_> = Action::ALL.iter().map(|a| a.label()).collect();
        let mut unique = names.clone(); unique.sort_unstable(); unique.dedup();
        assert_eq!(unique.len(), 6);
    }
    #[test]
    fn logind_capability_is_strict_and_fail_closed() {
        assert!(parse_logind_capability("s \"yes\"\n").is_ok());
        for answer in ["s \"no\"", "s \"na\"", "s \"challenge\"", "", "b true"] {
            assert!(parse_logind_capability(answer).is_err(), "{answer}");
        }
    }
    #[test]
    fn no_power_actions_without_explicit_native_opt_in() {
        assert!(Action::Lock.systemctl_command().is_none());
        assert!(Action::Logout.systemctl_command().is_none());
        assert_eq!(Action::PowerOff.systemctl_command(), Some("poweroff"));
        assert_eq!(Action::Hibernate.logind_method(), Some("CanHibernate"));
    }
}
