// wallpaper: sets per-monitor or spanning wallpaper through IDesktopWallpaper, and the JPEGImportQuality
// registry value that stops Windows recompressing JPG wallpapers. Monitor IDs are the device-interface
// paths monitors.rs reports; when an ID does not match (rare), the monitor's pixel rectangle does.
#![allow(dead_code)] // called from commands.rs once the pass-1 branches are merged
use crate::model::Rect;
use windows::core::{HSTRING, PCWSTR, PWSTR};
use windows::Win32::Foundation::RPC_E_CHANGED_MODE;
use windows::Win32::System::Com::*;
use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD};
use windows::Win32::UI::Shell::{
    DesktopWallpaper, IDesktopWallpaper, DESKTOP_WALLPAPER_POSITION, DWPOS_FILL, DWPOS_SPAN,
};

/// COM, apartment-threaded on this thread for as long as the guard lives.
struct Com(bool);

impl Com {
    fn init() -> Result<Com, String> {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if hr == RPC_E_CHANGED_MODE {
            return Ok(Com(false)); // already multithreaded here; IDesktopWallpaper works in either
        }
        hr.ok().map_err(|e| format!("Could not start COM: {e}"))?;
        Ok(Com(true))
    }

    fn wallpaper(&self) -> Result<IDesktopWallpaper, String> {
        unsafe { CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL) }
            .map_err(|e| format!("Windows' wallpaper service is not available: {e}"))
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

fn take(p: PWSTR) -> String {
    let s = unsafe { p.to_string() }.unwrap_or_default();
    unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
    s
}

/// (device path, rectangle) of every monitor that is attached; detached ones fail GetMonitorRECT.
fn attached(dw: &IDesktopWallpaper) -> Vec<(String, Rect)> {
    let n = unsafe { dw.GetMonitorDevicePathCount() }.unwrap_or(0);
    (0..n)
        .filter_map(|i| {
            let id = take(unsafe { dw.GetMonitorDevicePathAt(i) }.ok()?);
            let r = unsafe { dw.GetMonitorRECT(&HSTRING::from(&id)) }.ok()?;
            let rect = Rect {
                x: r.left as f64,
                y: r.top as f64,
                w: (r.right - r.left) as f64,
                h: (r.bottom - r.top) as f64,
            };
            Some((id, rect))
        })
        .collect()
}

/// The attached monitor for `id`: same path ignoring case, else the same pixel rectangle.
fn resolve<'a>(id: &str, rect: Option<Rect>, attached: &'a [(String, Rect)]) -> Option<&'a str> {
    attached
        .iter()
        .find(|(a, _)| a.eq_ignore_ascii_case(id))
        .or_else(|| attached.iter().find(|(_, r)| Some(*r) == rect))
        .map(|(a, _)| a.as_str())
}

/// Sets each (monitor id, file) pair with the FILL position (the tiles are native size, so no rescale).
pub fn set_per_monitor(files: &[(String, String)]) -> Result<(), String> {
    let com = Com::init()?;
    let dw = com.wallpaper()?;
    let attached = attached(&dw);
    let mut known: Option<Vec<crate::model::MonitorInfo>> = None;
    let mut targets = Vec::new();
    for (id, path) in files {
        let mut target = resolve(id, None, &attached);
        if target.is_none() {
            // Same monitor under a differently spelled path: match its pixel rectangle instead.
            let list = known.get_or_insert_with(crate::monitors::list);
            let rect = list
                .iter()
                .find(|m| m.id.eq_ignore_ascii_case(id))
                .map(|m| m.px);
            target = resolve(id, rect, &attached);
        }
        let target = target.ok_or_else(|| format!("The monitor {id} is not connected"))?;
        targets.push((target.to_string(), path));
    }
    unsafe { dw.SetPosition(DWPOS_FILL) }.map_err(|e| format!("Could not set Fill: {e}"))?;
    for (target, path) in targets {
        unsafe { dw.SetWallpaper(&HSTRING::from(target), &HSTRING::from(path.as_str())) }
            .map_err(|e| format!("Could not set {path} as wallpaper: {e}"))?;
    }
    Ok(())
}

/// One image across every monitor (DWPOS_SPAN).
pub fn set_span(path: &str) -> Result<(), String> {
    let com = Com::init()?;
    let dw = com.wallpaper()?;
    unsafe {
        dw.SetPosition(DWPOS_SPAN)
            .map_err(|e| format!("Could not set Span: {e}"))?;
        dw.SetWallpaper(PCWSTR::null(), &HSTRING::from(path))
    }
    .map_err(|e| format!("Could not set {path} as wallpaper: {e}"))
}

/// (monitor id, wallpaper file) for every attached monitor; the file is "" for a solid colour.
pub fn get_current() -> Vec<(String, String)> {
    let Ok(com) = Com::init() else { return vec![] };
    let Ok(dw) = com.wallpaper() else {
        return vec![];
    };
    attached(&dw)
        .into_iter()
        .map(|(id, _)| {
            let file = unsafe { dw.GetWallpaper(&HSTRING::from(&id)) }.map(take);
            (id, file.unwrap_or_default())
        })
        .collect()
}

fn position() -> Option<DESKTOP_WALLPAPER_POSITION> {
    let com = Com::init().ok()?;
    unsafe { com.wallpaper().ok()?.GetPosition() }.ok()
}

fn set_position(p: DESKTOP_WALLPAPER_POSITION) -> Result<(), String> {
    let com = Com::init()?;
    unsafe { com.wallpaper()?.SetPosition(p) }.map_err(|e| e.to_string())
}

/// HKCU\Control Panel\Desktop\JPEGImportQuality = 100, so Windows keeps JPG wallpapers at full quality.
pub fn set_jpeg_import_quality_100() -> Result<(), String> {
    let value = 100u32;
    let r = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(r"Control Panel\Desktop"),
            &HSTRING::from("JPEGImportQuality"),
            REG_DWORD.0,
            Some(&value as *const u32 as *const _),
            4,
        )
    };
    r.ok()
        .map_err(|e| format!("Could not set JPEGImportQuality: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f64) -> Rect {
        Rect {
            x,
            y: 0.0,
            w: 3840.0,
            h: 2160.0,
        }
    }

    #[test]
    fn resolves_by_id_ignoring_case_then_by_rect() {
        let attached = vec![
            (r"\\?\display#sam1015#a".to_string(), r(0.0)),
            (r"\\?\display#bnq7950#b".to_string(), r(3840.0)),
        ];
        assert_eq!(
            resolve(r"\\?\DISPLAY#BNQ7950#B", None, &attached),
            Some(r"\\?\display#bnq7950#b")
        );
        assert_eq!(
            resolve("gone", Some(r(0.0)), &attached),
            Some(r"\\?\display#sam1015#a")
        );
        assert_eq!(resolve("gone", Some(r(100.0)), &attached), None);
        assert_eq!(resolve("gone", None, &attached), None);
    }

    /// Run by hand: `cargo test real_current -- --ignored --nocapture`. Reads only.
    #[test]
    #[ignore = "reads the real desktop"]
    fn real_current_wallpapers() {
        println!("{:?} position {:?}", get_current(), position());
    }

    /// Run by hand: `cargo test real_wallpaper -- --ignored --nocapture`. Sets a grey test image on every
    /// monitor for a moment, then puts the previous wallpapers and position back.
    #[test]
    #[ignore = "changes the real wallpaper (and restores it)"]
    fn real_wallpaper_set_and_restore() {
        let before = get_current();
        let pos = position().expect("read position");
        assert!(
            !before.is_empty() && before.iter().all(|(_, f)| !f.is_empty()),
            "{before:?}"
        );
        let png = std::env::temp_dir().join("ws-wallpaper-test.png");
        image::RgbImage::from_pixel(64, 64, image::Rgb([90, 90, 90]))
            .save(&png)
            .unwrap();
        let files: Vec<_> = before
            .iter()
            .map(|(id, _)| (id.to_uppercase(), png.display().to_string()))
            .collect();
        let set = set_per_monitor(&files);
        let during = get_current();
        set_per_monitor(&before).unwrap();
        set_position(pos).unwrap();
        assert_eq!(get_current(), before, "restore failed");
        set.unwrap();
        assert!(
            during
                .iter()
                .all(|(_, f)| f.eq_ignore_ascii_case(&files[0].1)),
            "{during:?}"
        );
    }
}
