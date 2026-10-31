//! Read-only, fail-closed detection of iio-sensor-proxy accelerometers.
//! Do not rotate the host or advertise an enabled switch until the Smithay
//! output transforms and touchscreen coordinates are wired end-to-end.
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accelerometer { Present, Absent, Unknown }

fn parse_busctl_boolean(output: &str) -> Accelerometer {
    match output.trim() {
        "b true" => Accelerometer::Present,
        "b false" => Accelerometer::Absent,
        _ => Accelerometer::Unknown,
    }
}

pub fn accelerometer() -> Accelerometer {
    let result=Command::new("busctl").args([
        "--system", "get-property", "net.hadess.SensorProxy",
        "/net/hadess/SensorProxy", "net.hadess.SensorProxy",
        "HasAccelerometer",
    ]).output();
    match result {
        Ok(output) if output.status.success() =>
            parse_busctl_boolean(&String::from_utf8_lossy(&output.stdout)),
        _ => Accelerometer::Unknown,
    }
}

pub fn explanation(state: Accelerometer) -> &'static str {
    match state {
        Accelerometer::Present =>
            "Accelerometro disponibile tramite iio-sensor-proxy. La rotazione richiede ancora il backend di orientamento del compositore Melania: nessun cambiamento è stato applicato.",
        Accelerometer::Absent =>
            "iio-sensor-proxy non rileva accelerometri. La rotazione automatica non è disponibile su questo dispositivo.",
        Accelerometer::Unknown =>
            "Impossibile interrogare iio-sensor-proxy. Controlla l'installazione del servizio e la disponibilità dei sensori.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_busctl_boolean_without_false_positives() {
        assert_eq!(parse_busctl_boolean("b true\n"), Accelerometer::Present);
        assert_eq!(parse_busctl_boolean("b false\n"), Accelerometer::Absent);
        for invalid in ["", "true", "b True", "s \"true\"", "b false extra", "b true\nb false"] {
            assert_eq!(parse_busctl_boolean(invalid), Accelerometer::Unknown);
        }
    }
}
