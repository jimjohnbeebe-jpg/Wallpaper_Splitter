// preview_win: the borderless per-monitor windows behind preview.rs (full-screen preview and identify),
// placed in physical pixels so mixed-DPI desks line up, plus the HDR switching around the preview.
// Everything here runs on helper threads: building a webview from a synchronous command deadlocks.
use crate::model::Rect;
use crate::{hdr, preview};
use std::time::Duration;
use tauri::window::Color;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow};
use tauri::{WebviewWindowBuilder, WindowEvent};

const IDENTIFY_W: f64 = 320.0;
const IDENTIFY_H: f64 = 200.0;
const IDENTIFY_FOR: Duration = Duration::from_millis(2500);

fn build(app: &AppHandle, label: &str, r: Rect) -> tauri::Result<WebviewWindow> {
    let w = WebviewWindowBuilder::new(app, label, WebviewUrl::App("preview.html".into()))
        .title("Wallpaper Splitter")
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(false)
        .focused(false)
        .shadow(false)
        .background_color(Color(0, 0, 0, 255))
        .build()?;
    // Size after the move: crossing onto a monitor with another scale makes Windows rescale the window.
    w.set_position(PhysicalPosition::new(
        r.x.round() as i32,
        r.y.round() as i32,
    ))?;
    w.set_size(PhysicalSize::new(r.w.round() as u32, r.h.round() as u32))?;
    w.show()?;
    Ok(w)
}

pub fn close_labeled(app: &AppHandle, prefix: &str) {
    for (label, w) in app.webview_windows() {
        if label.starts_with(prefix) {
            let _ = w.close();
        }
    }
}

/// Labels are reused, so wait (up to 2 s) for the previous set of windows to be gone.
fn wait_gone(app: &AppHandle, prefix: &str) {
    for _ in 0..40 {
        if !app.webview_windows().keys().any(|l| l.starts_with(prefix)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

pub fn hdr_off(gdi_names: &[String]) {
    for gdi in gdi_names {
        let _ = hdr::set(gdi, false);
    }
}

fn current(app: &AppHandle, gen: u64) -> bool {
    preview::lock(app)
        .preview
        .as_ref()
        .is_some_and(|p| p.gen == gen)
}

/// Switches HDR on for the preview's monitors that support it and have it off, remembering each one so
/// close() can switch it back; if the preview closed meanwhile, switches it straight back.
fn hdr_on(app: &AppHandle, gen: u64) {
    let gdis: Vec<String> = match preview::lock(app).preview.as_ref() {
        Some(p) if p.gen == gen => p
            .windows
            .iter()
            .filter_map(|&i| p.window_monitor(i).map(|m| m.gdi_name.clone()))
            .collect(),
        _ => return,
    };
    for gdi in gdis {
        let before = hdr::get(&gdi);
        if !before.supported || before.enabled {
            continue;
        }
        let _ = hdr::set(&gdi, true); // remembered even on error: switching off an off monitor is a no-op
        let mut s = preview::lock(app);
        match s.preview.as_mut() {
            Some(p) if p.gen == gen => p.hdr_on.push(gdi),
            _ => {
                drop(s);
                hdr_off(&[gdi]);
            }
        }
    }
}

pub fn open_windows(app: &AppHandle, gen: u64, auto_hdr: bool) {
    wait_gone(app, "preview-");
    if auto_hdr {
        hdr_on(app, gen);
    }
    let targets: Vec<(usize, Rect)> = match preview::lock(app).preview.as_ref() {
        Some(p) if p.gen == gen => p
            .windows
            .iter()
            .filter_map(|&i| Some((i, p.window_monitor(i)?.px)))
            .collect(),
        _ => return,
    };
    let mut first = None;
    for (i, rect) in targets {
        if !current(app, gen) {
            break;
        }
        match build(app, &format!("preview-{i}"), rect) {
            Ok(w) => {
                // Alt+F4 (or any close) on one window closes them all and restores HDR.
                let a = app.clone();
                w.on_window_event(move |e| {
                    if matches!(e, WindowEvent::Destroyed) {
                        preview::close_gen(&a, gen);
                    }
                });
                first.get_or_insert(w);
            }
            Err(e) => {
                eprintln!("preview window {i}: {e}");
                preview::close_gen(app, gen);
                return;
            }
        }
    }
    if !current(app, gen) {
        close_labeled(app, "preview-"); // closed while the windows were being built
    } else if let Some(w) = first {
        let _ = w.set_focus();
    }
}

fn centred(m: Rect) -> Rect {
    Rect {
        x: m.x + ((m.w - IDENTIFY_W) / 2.0).round(),
        y: m.y + ((m.h - IDENTIFY_H) / 2.0).round(),
        w: IDENTIFY_W,
        h: IDENTIFY_H,
    }
}

pub fn identify_windows(app: &AppHandle, gen: u64) {
    close_labeled(app, "identify-");
    wait_gone(app, "identify-");
    let list = preview::lock(app).identify.clone();
    for (i, m) in list.iter().enumerate() {
        if let Err(e) = build(app, &format!("identify-{i}"), centred(m.px)) {
            eprintln!("identify window {i}: {e}");
        }
    }
    std::thread::sleep(IDENTIFY_FOR);
    let mut s = preview::lock(app);
    if s.identify_gen == gen {
        s.identify.clear();
        drop(s);
        close_labeled(app, "identify-");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identify_window_is_centred_in_physical_pixels() {
        let m = Rect {
            x: 3840.0,
            y: -100.0,
            w: 2560.0,
            h: 1440.0,
        };
        assert_eq!(
            centred(m),
            Rect {
                x: 4960.0,
                y: 520.0,
                w: 320.0,
                h: 200.0
            }
        );
    }
}
