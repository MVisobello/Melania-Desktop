//! Small, non-privileged adapters for standard Linux desktop services.
//! No shell interpolation: all commands and arguments are provided separately.
//! Backends depend on installed commands (nmcli, bluetoothctl, wpctl,
//! brightnessctl, powerprofilesctl) and the user's permissions.
use std::fmt;
use std::process::Command;

#[derive(Debug)]
pub enum ServiceError {
    MissingCommand(String),
    Failed(String),
    Unexpected(String),
    InvalidValue(String),
}
impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand(c) => write!(f, "Comando non disponibile: {c}"),
            Self::Failed(e) => write!(f, "Operazione non riuscita: {e}"),
            Self::Unexpected(e) => write!(f, "Risposta di sistema inattesa: {e}"),
            Self::InvalidValue(e) => write!(f, "Valore non valido: {e}"),
        }
    }
}
impl std::error::Error for ServiceError {}
pub type Result<T> = std::result::Result<T, ServiceError>;

fn command(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program).args(args).output().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            ServiceError::MissingCommand(program.to_owned())
        } else { ServiceError::Failed(err.to_string()) }
    })?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(ServiceError::Failed(if message.is_empty() {
            format!("{program}: codice {}", output.status)
        } else { message }));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn parse_nmcli_enabled(text: &str) -> Result<bool> {
    match text.trim() {
        "enabled" => Ok(true),
        "disabled" => Ok(false),
        other => Err(ServiceError::Unexpected(other.to_owned())),
    }
}
pub fn wifi_enabled() -> Result<bool> { parse_nmcli_enabled(&command("nmcli", &["radio", "wifi"])?) }
pub fn set_wifi(on: bool) -> Result<()> {
    command("nmcli", &["radio", "wifi", if on { "on" } else { "off" }])?;
    Ok(())
}

pub fn parse_bluetooth_enabled(text: &str) -> Result<bool> {
    text.lines()
        .find_map(|line| line.trim().strip_prefix("Powered:"))
        .map(|v| v.trim() == "yes")
        .ok_or_else(|| ServiceError::Unexpected("BlueZ: stato Powered assente".into()))
}
pub fn bluetooth_enabled() -> Result<bool> { parse_bluetooth_enabled(&command("bluetoothctl", &["show"])?) }
pub fn set_bluetooth(on: bool) -> Result<()> {
    command("bluetoothctl", &["power", if on { "on" } else { "off" }])?;
    Ok(())
}

pub fn parse_wpctl_volume(text: &str) -> Result<u8> {
    let s = text.trim().strip_prefix("Volume:")
        .ok_or_else(|| ServiceError::Unexpected("PipeWire: volume assente".into()))?;
    let fraction: f32 = s.split_whitespace().next().unwrap_or("").parse()
        .map_err(|_| ServiceError::Unexpected(text.into()))?;
    if !fraction.is_finite() || fraction < 0.0 {
        return Err(ServiceError::Unexpected(text.into()));
    }
    Ok(((fraction * 100.0).round() as u32).min(100) as u8)
}
pub fn volume() -> Result<u8> {
    parse_wpctl_volume(&command("wpctl", &["get-volume", "@DEFAULT_AUDIO_SINK@"])?)
}
pub fn set_volume(pct: u8) -> Result<()> {
    if pct > 100 { return Err(ServiceError::InvalidValue("volume deve essere 0-100".into())); }
    let value = format!("{:.2}", pct as f32 / 100.0);
    command("wpctl", &["set-volume", "@DEFAULT_AUDIO_SINK@", &value])?;
    Ok(())
}

pub fn parse_brightness(text: &str) -> Result<u8> {
    // brightnessctl -m: device,class,current,max,percent%
    let value = text.trim().split(',').next_back().unwrap_or("").trim().trim_end_matches('%');
    let pct = value.parse::<u8>().map_err(|_| ServiceError::Unexpected(text.into()))?;
    if pct > 100 { return Err(ServiceError::Unexpected(text.into())); }
    Ok(pct)
}
pub fn brightness() -> Result<u8> { parse_brightness(&command("brightnessctl", &["-m"])?) }
pub fn set_brightness(pct: u8) -> Result<()> {
    if pct > 100 { return Err(ServiceError::InvalidValue("luminosità deve essere 0-100".into())); }
    let value = format!("{pct}%");
    command("brightnessctl", &["set", &value])?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerProfile { PowerSaver, Balanced, Performance }
impl PowerProfile {
    pub fn as_str(self) -> &'static str {
        match self { Self::PowerSaver => "power-saver", Self::Balanced => "balanced", Self::Performance => "performance" }
    }
}
pub fn power_profile() -> Result<PowerProfile> {
    match command("powerprofilesctl", &["get"])?.as_str() {
        "power-saver" => Ok(PowerProfile::PowerSaver),
        "balanced" => Ok(PowerProfile::Balanced),
        "performance" => Ok(PowerProfile::Performance),
        x => Err(ServiceError::Unexpected(x.into())),
    }
}
pub fn set_power_profile(profile: PowerProfile) -> Result<()> {
    command("powerprofilesctl", &["set", profile.as_str()])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn wifi_parsing() {
        assert!(parse_nmcli_enabled("enabled\n").unwrap());
        assert!(!parse_nmcli_enabled("disabled").unwrap());
        assert!(parse_nmcli_enabled("unavailable").is_err());
    }
    #[test] fn bluez_parsing() {
        assert!(parse_bluetooth_enabled("Controller 01:02\n Powered: yes\n Discoverable: no").unwrap());
        assert!(!parse_bluetooth_enabled("Powered: no").unwrap());
        assert!(parse_bluetooth_enabled("Controller XX").is_err());
    }
    #[test] fn volume_parsing() {
        assert_eq!(parse_wpctl_volume("Volume: 0.34 [MUTED]").unwrap(),34);
        assert_eq!(parse_wpctl_volume("Volume: 1.40").unwrap(),100);
        assert!(parse_wpctl_volume("Volume: NaN").is_err());
    }
    #[test] fn brightness_parsing() {
        assert_eq!(parse_brightness("intel_backlight,backlight,200,1200,17%").unwrap(),17);
        assert!(parse_brightness("no device").is_err());
    }
}
