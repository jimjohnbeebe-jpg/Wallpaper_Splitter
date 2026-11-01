// settings: %APPDATA%\WallpaperSplitter\settings.json, written to a temporary name and renamed so a reader
// never sees half a file (pattern: LrC-AVG hud/src-tauri/src/store.rs). A missing file reads as the
// defaults; a corrupt one also reads as the defaults and is kept beside it as settings.json.bad.
#![allow(dead_code)] // called from commands.rs once the pass-1 branches are merged
use crate::model::Settings;
use std::path::{Path, PathBuf};

fn env_dir(var: &str) -> PathBuf {
    std::env::var_os(var)
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join("WallpaperSplitter")
}

fn path() -> PathBuf {
    env_dir("APPDATA").join("settings.json")
}

/// Where decoded proxies and other throwaway files go: %LOCALAPPDATA%\WallpaperSplitter\cache.
pub fn cache_dir() -> PathBuf {
    env_dir("LOCALAPPDATA").join("cache")
}

pub fn load() -> Settings {
    load_from(&path())
}

pub fn save(s: &Settings) -> Result<(), String> {
    save_to(&path(), s)
}

fn load_from(file: &Path) -> Settings {
    let Ok(text) = std::fs::read_to_string(file) else {
        return Settings::default();
    };
    serde_json::from_str(&text).unwrap_or_else(|_| {
        let _ = std::fs::rename(file, file.with_extension("json.bad"));
        Settings::default()
    })
}

fn save_to(file: &Path, s: &Settings) -> Result<(), String> {
    let fail = |e: std::io::Error| format!("Could not save settings to {}: {e}", file.display());
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(fail)?;
    }
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    let temp = file.with_extension(format!("json.{}.tmp", std::process::id()));
    std::fs::write(&temp, text).map_err(fail)?;
    std::fs::rename(&temp, file).map_err(fail) // MoveFileExW with MOVEFILE_REPLACE_EXISTING
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ws-settings-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn missing_file_is_defaults_and_round_trip_keeps_everything() {
        let dir = temp_dir("rt");
        let file = dir.join("settings.json");
        assert_eq!(load_from(&file), Settings::default());
        let mut s = Settings::default();
        s.prefs.format = ExportFormat::Jpg { quality: 92 };
        s.prefs.sdr_white_nits = Some(240.0);
        s.last = Some(Session {
            image: r"C:\a.jpg".into(),
            framing: Framing::default(),
        });
        s.calibrations.insert(
            "a|b".into(),
            vec![PlacedMonitor {
                id: "a".into(),
                px_w: 3840,
                px_h: 2160,
                mm: Rect {
                    x: 0.0,
                    y: 10.0,
                    w: 698.0,
                    h: 393.0,
                },
                nudge_px: Point { x: 2.0, y: -1.0 },
            }],
        );
        save_to(&file, &s).unwrap();
        save_to(&file, &s).unwrap(); // replacing an existing file works too
        assert_eq!(load_from(&file), s);
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            1,
            "no temp file left"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_file_is_defaults_and_kept_as_bad() {
        let dir = temp_dir("bad");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        std::fs::write(&file, "{ not json").unwrap();
        assert_eq!(load_from(&file), Settings::default());
        assert!(!file.exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("settings.json.bad")).unwrap(),
            "{ not json"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn partial_file_fills_in_defaults() {
        let dir = temp_dir("partial");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("settings.json");
        std::fs::write(&file, r#"{"prefs":{"span":true}}"#).unwrap();
        let s = load_from(&file);
        assert!(s.prefs.span);
        assert_eq!(s.prefs.template, Prefs::default().template);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
