//! Config persistence for Melania-specific preferences.
//! System-wide device settings are deliberately not stored here.
use std::{fs, io, path::{Path, PathBuf}};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub dark_mode: bool,
    pub aero_glass: bool,
    pub animations: bool,
    pub snap_windows: bool,
    pub preview_windows: bool,
    pub do_not_disturb: bool,
    pub blur_strength: u8,
}
impl Default for Preferences {
    fn default() -> Self { Self {
        dark_mode: true, aero_glass: true, animations: true,
        snap_windows: true, preview_windows: true,
        do_not_disturb: false, blur_strength: 70,
    } }
}
impl Preferences {
    pub fn from_text(data: &str) -> Self {
        let mut prefs = Self::default();
        for line in data.lines() {
            let Some((k, v)) = line.trim().split_once('=') else { continue; };
            let k = k.trim(); let v = v.trim();
            let as_bool = || match v { "true" => Some(true), "false" => Some(false), _ => None };
            match k {
                "dark_mode" => if let Some(b) = as_bool() { prefs.dark_mode = b },
                "aero_glass" => if let Some(b) = as_bool() { prefs.aero_glass = b },
                "animations" => if let Some(b) = as_bool() { prefs.animations = b },
                "snap_windows" => if let Some(b) = as_bool() { prefs.snap_windows = b },
                "preview_windows" => if let Some(b) = as_bool() { prefs.preview_windows = b },
                "do_not_disturb" => if let Some(b) = as_bool() { prefs.do_not_disturb = b },
                "blur_strength" => if let Ok(n) = v.parse::<u8>() { prefs.blur_strength = n.min(100) },
                _ => {}, // future versions can read older/newer files
            }
        }
        prefs
    }
    pub fn to_text(&self) -> String { format!(
        "# Melania Desktop preferences v1\ndark_mode={}\naero_glass={}\nanimations={}\nsnap_windows={}\npreview_windows={}\ndo_not_disturb={}\nblur_strength={}\n",
        self.dark_mode, self.aero_glass, self.animations, self.snap_windows,
        self.preview_windows, self.do_not_disturb, self.blur_strength)
    }
    pub fn read_from(path: &Path) -> io::Result<Self> {
        match fs::read_to_string(path) {
            Ok(s) => Ok(Self::from_text(&s)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }
    pub fn write_to(&self, path: &Path) -> io::Result<()> {
        let parent = path.parent().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path without parent"))?;
        fs::create_dir_all(parent)?;
        let temp = path.with_extension(format!("conf.tmp.{}", std::process::id()));
        let result = (|| -> io::Result<()> {
            let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
            use std::io::Write;
            f.write_all(self.to_text().as_bytes())?;
            f.sync_all()?;
            fs::rename(&temp, path)?;
            Ok(())
        })();
        if result.is_err() { let _ = fs::remove_file(&temp); }
        result
    }
}
pub fn config_path() -> io::Result<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME").filter(|s| !s.is_empty()) {
        Some(x) => PathBuf::from(x),
        None => PathBuf::from(std::env::var_os("HOME").ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "HOME missing"))?).join(".config"),
    };
    Ok(base.join("melania").join("settings.conf"))
}
pub fn load() -> io::Result<Preferences> { Preferences::read_from(&config_path()?) }
pub fn save(p: &Preferences) -> io::Result<()> { p.write_to(&config_path()?) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn retains_defaults_for_unknown_or_bad_values() {
        let p = Preferences::from_text("dark_mode=false\nunknown_key=7\nblur_strength=500\nanimations=maybe");
        assert!(!p.dark_mode);
        assert_eq!(p.blur_strength, 70);
        assert!(p.animations);
    }
    #[test] fn round_trip() {
        let mut p = Preferences::default();
        p.dark_mode = false; p.do_not_disturb = true;
        assert_eq!(Preferences::from_text(&p.to_text()), p);
    }
    #[test] fn reads_and_writes_file() {
        let dir = std::env::temp_dir().join(format!("melania-test-{}-{:?}", std::process::id(), std::thread::current().id()));
        let path = dir.join("settings.conf");
        let mut p = Preferences::default();
        p.blur_strength = 32;
        p.write_to(&path).unwrap();
        assert_eq!(Preferences::read_from(&path).unwrap(), p);
        fs::remove_dir_all(dir).unwrap();
    }
}
