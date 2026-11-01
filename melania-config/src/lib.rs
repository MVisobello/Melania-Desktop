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
    /// Fixed workspace count; applied when the compositor and panel start.
    pub workspace_count: u8,
    /// Group taskbar buttons by application ID; disabled on upgrades only
    /// when a user explicitly saved false in their preferences.
    pub group_windows: bool,
    /// Reduce GTK animations in Melania applications and shell panels.
    pub reduce_motion: bool,
    /// Increase contrast of Melania shell and Settings (not host GTK themes).
    pub high_contrast: bool,
    /// Icon family used by every Melania GTK process (never changes host GTK).
    pub icon_theme: String,
    /// Panel geometry; these preferences affect only the Melania layer shell.
    pub panel_height: u16,
    pub panel_position: String,
    pub panel_auto_hide: bool,
    /// Percentage of panel surface opacity, 40–100; 94 matches Aero default.
    pub panel_opacity: u8,
}
impl Default for Preferences {
    fn default() -> Self { Self {
        dark_mode: false, aero_glass: true, animations: true,
        snap_windows: true, preview_windows: true,
        do_not_disturb: false, blur_strength: 70, workspace_count: 4,
        group_windows: false, reduce_motion: false, high_contrast: false,
        icon_theme: "Melania-Fluent-Light".into(),
        panel_height: 52, panel_position: "bottom".into(),
        panel_auto_hide: false, panel_opacity: 94,
    } }
}
/// Per-user recent Start launches shared between Panel and Settings.
/// This does not touch GNOME/XFCE/KDE recent documents or system settings.
pub fn recent_apps_path() -> Option<PathBuf> {
    let root=std::env::var_os("XDG_STATE_HOME").filter(|v|!v.is_empty())
        .map(PathBuf::from)
        .or_else(||std::env::var_os("HOME").filter(|v|!v.is_empty())
            .map(|v|PathBuf::from(v).join(".local/state")))?;
    Some(root.join("melania").join("recent-applications"))
}

pub const ICON_THEMES: [&str; 6] = [
    "Melania-Fluent-Light", "Melania-Fluent-Dark", "Melania-Fluent-Light-on-Dark-Panel",
    "Melania-Light", "Melania-Dark", "Melania-Light-on-Dark-Panel"
];
/// The bundled theme is added to GTK's per-process icon search paths.
/// Installed user icon themes remain discoverable in ~/.local/share/icons.
pub fn bundled_icon_paths() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    vec![
        root.join("dist/melania-icon-themes"),
        root.join("dist/melania-fluent-icons"),
        root.join("branding"),
    ]
}
pub fn icon_theme_available(name: &str) -> bool {
    if !ICON_THEMES.contains(&name) {return false;}
    if bundled_icon_paths().iter().any(|p|p.join(name).join("index.theme").is_file()) {
        return true;
    }
    let user = std::env::var_os("XDG_DATA_HOME").filter(|s| !s.is_empty())
        .map(PathBuf::from).or_else(||std::env::var_os("HOME")
            .map(|h|PathBuf::from(h).join(".local/share")));
    user.is_some_and(|base|base.join("icons").join(name).join("index.theme").is_file())
}
pub fn resolved_icon_theme(prefs:&Preferences) -> &str {
    if icon_theme_available(&prefs.icon_theme) {&prefs.icon_theme}
    // Never silently fall back to the previous glossy Aero family when the
    // selected Fluent theme has not yet been installed. Adwaita is a coherent
    // standard GTK fallback; explicit legacy icon choices remain available.
    else {"Adwaita"}
}
impl Preferences {
    /// The three mutually exclusive Melania interface themes. Legacy config
    /// booleans remain on disk so existing installations keep their choices.
    pub fn appearance_theme(&self) -> &'static str {
        if self.dark_mode { "dark" }
        else if self.aero_glass { "aero" }
        else { "light" }
    }
    pub fn set_appearance_theme(&mut self, name: &str) -> bool {
        match name {
            "light" => { self.dark_mode = false; self.aero_glass = false; true }
            "dark" => { self.dark_mode = true; self.aero_glass = false; true }
            "aero" => { self.dark_mode = false; self.aero_glass = true; true }
            _ => false,
        }
    }
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
                "workspace_count" => if let Ok(n) = v.parse::<u8>() { if (1..=8).contains(&n) { prefs.workspace_count = n; } },
                "group_windows" => if let Some(b) = as_bool() { prefs.group_windows = b },
                "reduce_motion" => if let Some(b) = as_bool() { prefs.reduce_motion = b },
                "high_contrast" => if let Some(b) = as_bool() { prefs.high_contrast = b },
                "icon_theme" if ICON_THEMES.contains(&v) => prefs.icon_theme = v.to_string(),
                "panel_height" => if let Ok(n)=v.parse::<u16>() {
                    if (52..=96).contains(&n){prefs.panel_height=n;}
                },
                "panel_position" if matches!(v,"bottom"|"top") =>
                    prefs.panel_position=v.to_owned(),
                "panel_auto_hide" => if let Some(b)=as_bool(){prefs.panel_auto_hide=b;},
                "panel_opacity" => if let Ok(n)=v.parse::<u8>() {
                    if (40..=100).contains(&n){prefs.panel_opacity=n;}
                },
                _ => {}, // future versions can read older/newer files
            }
        }
        prefs
    }
    pub fn to_text(&self) -> String { format!(
        "# Melania Desktop preferences v1\ndark_mode={}\naero_glass={}\nanimations={}\nsnap_windows={}\npreview_windows={}\ndo_not_disturb={}\nblur_strength={}\nworkspace_count={}\ngroup_windows={}\nreduce_motion={}\nhigh_contrast={}\nicon_theme={}\npanel_height={}\npanel_position={}\npanel_auto_hide={}\npanel_opacity={}\n",
        self.dark_mode, self.aero_glass, self.animations, self.snap_windows,
        self.preview_windows, self.do_not_disturb, self.blur_strength, self.workspace_count,
        self.group_windows, self.reduce_motion, self.high_contrast, self.icon_theme,
        self.panel_height, self.panel_position, self.panel_auto_hide, self.panel_opacity)
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
    #[test] fn aero_light_is_the_default_for_new_installations() {
        let p = Preferences::default();
        assert!(p.aero_glass);
        assert!(!p.dark_mode);
        assert_eq!(p.blur_strength, 70);
    }
    #[test] fn light_dark_aero_are_exclusive_and_backward_compatible() {
        let mut p = Preferences::default();
        assert_eq!(p.appearance_theme(), "aero");
        for name in ["light", "dark", "aero"] {
            assert!(p.set_appearance_theme(name));
            assert_eq!(p.appearance_theme(), name);
            assert_eq!(Preferences::from_text(&p.to_text()).appearance_theme(), name);
        }
        assert!(!p.set_appearance_theme("invalid"));
        assert_eq!(p.appearance_theme(), "aero");
        assert_eq!(Preferences::from_text("aero_glass=false\ndark_mode=false").appearance_theme(), "light");
        assert_eq!(Preferences::from_text("aero_glass=false\ndark_mode=true").appearance_theme(), "dark");
    }
    #[test] fn round_trip() {
        let mut p = Preferences::default();
        p.dark_mode = false; p.do_not_disturb = true;
        assert_eq!(Preferences::from_text(&p.to_text()), p);
    }
    #[test] fn panel_controls_validate_and_keep_approved_defaults() {
        let default=Preferences::default();
        assert_eq!(default.panel_height,52);
        assert_eq!(default.panel_position,"bottom");
        assert!(!default.panel_auto_hide);
        assert_eq!(default.panel_opacity,94);
        let edited=Preferences::from_text("panel_height=72\npanel_position=top\npanel_auto_hide=true\npanel_opacity=70");
        assert_eq!(edited.panel_height,72);
        assert_eq!(edited.panel_position,"top");
        assert!(edited.panel_auto_hide);
        assert_eq!(edited.panel_opacity,70);
        assert_eq!(edited,Preferences::from_text(&edited.to_text()));
        let invalid=Preferences::from_text("panel_height=200\npanel_position=else\npanel_auto_hide=maybe\npanel_opacity=0");
        assert_eq!(invalid,default);
        let old=Preferences::from_text("snap_windows=false");
        assert_eq!(old.panel_height,52);
        assert_eq!(old.panel_position,"bottom");
    }
    #[test] fn workspace_count_is_validated_and_old_configs_keep_default() {
        assert_eq!(Preferences::from_text("workspace_count=1").workspace_count, 1);
        assert_eq!(Preferences::from_text("workspace_count=8").workspace_count, 8);
        for invalid in ["workspace_count=0", "workspace_count=9", "workspace_count=-1", "workspace_count=abc"] {
            assert_eq!(Preferences::from_text(invalid).workspace_count, 4);
        }
        assert_eq!(Preferences::from_text("animations=false").workspace_count, 4);
        let mut p = Preferences::default();
        p.workspace_count = 6;
        assert_eq!(Preferences::from_text(&p.to_text()), p);
    }
    #[test] fn compatible_productivity_and_accessibility_preferences() {
        let old = Preferences::from_text("workspace_count=4\nsnap_windows=true");
        assert!(!old.group_windows);
        assert!(!old.reduce_motion);
        assert!(!old.high_contrast);
        let p = Preferences::from_text(
            "group_windows=true\nreduce_motion=true\nhigh_contrast=true\n"
        );
        assert!(p.group_windows && p.reduce_motion && p.high_contrast);
        assert_eq!(Preferences::from_text(&p.to_text()), p);
        let bad = Preferences::from_text("group_windows=probably\nreduce_motion=1\nhigh_contrast=");
        assert!(!bad.group_windows && !bad.reduce_motion && !bad.high_contrast);
    }
    #[test] fn icon_themes_are_validated_and_backward_compatible() {
        assert_eq!(Preferences::default().icon_theme,"Melania-Fluent-Light");
        for name in ICON_THEMES {
            let p=Preferences::from_text(&format!("icon_theme={name}"));
            assert_eq!(p.icon_theme,name);
            assert_eq!(Preferences::from_text(&p.to_text()),p);
        }
        assert_eq!(Preferences::from_text("icon_theme=../../hijack").icon_theme,"Melania-Fluent-Light");
        assert_eq!(Preferences::from_text("aero_glass=true").icon_theme,"Melania-Fluent-Light");
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
