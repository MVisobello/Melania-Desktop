//! Human-friendly command line bridge for quick controls and debugging.
use melania_services as services;
fn usage() {
    eprintln!("Uso: melania-control status | wifi on|off | bluetooth on|off | volume 0..100 | brightness 0..100 | power balanced|power-saver|performance");
}
fn main() {
    if let Err(e) = run() { eprintln!("Melania Control: {e}"); std::process::exit(1); }
}
fn run() -> services::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 && args[0] == "status" {
        println!("Wi-Fi: {:?}", services::wifi_enabled());
        println!("Bluetooth: {:?}", services::bluetooth_enabled());
        println!("Volume: {:?}", services::volume());
        println!("Luminosità: {:?}", services::brightness());
        println!("Energia: {:?}", services::power_profile());
        return Ok(());
    }
    if args.len() != 2 { usage(); return Err(services::ServiceError::InvalidValue("argomenti".into())); }
    match (args[0].as_str(), args[1].as_str()) {
        ("wifi", "on") => services::set_wifi(true),
        ("wifi", "off") => services::set_wifi(false),
        ("bluetooth", "on") => services::set_bluetooth(true),
        ("bluetooth", "off") => services::set_bluetooth(false),
        ("power", "balanced") => services::set_power_profile(services::PowerProfile::Balanced),
        ("power", "power-saver") => services::set_power_profile(services::PowerProfile::PowerSaver),
        ("power", "performance") => services::set_power_profile(services::PowerProfile::Performance),
        ("volume", value) | ("brightness", value) => {
            let pct = value.parse::<u8>().map_err(|_| services::ServiceError::InvalidValue(value.into()))?;
            if args[0] == "volume" { services::set_volume(pct) } else { services::set_brightness(pct) }
        }
        _ => { usage(); Err(services::ServiceError::InvalidValue(args.join(" "))) }
    }
}
