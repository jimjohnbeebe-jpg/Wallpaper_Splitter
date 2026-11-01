// preview_win: the borderless per-monitor windows behind preview.rs (full-screen preview and identify),
// placed in physical pixels so mixed-DPI desks line up, plus the HDR switching around the preview.
// Everything here runs on helper threads: building a webview from a synchronous command deadlocks.
use crate::model::{HdrState, Rect};
use crate::{hdr, preview};
use std::sync::{Mutex, MutexGuard};
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

/// The monitors whose HDR the preview switched on and that are not confirmed off yet, each with the
/// preview generation that owns it. The lock is held across every switch, so a restore (closing the
/// preview, or app exit) waits for a switch-on in progress instead of overtaking it, and the next preview
/// reads a monitor's state only after an earlier restore has finished.
type Owned = Mutex<Vec<(String, u64)>>;
static HDR_ON: Owned = Mutex::new(Vec::new());

fn guard(list: &Owned) -> MutexGuard<'_, Vec<(String, u64)>> {
    list.lock().unwrap_or_else(|e| e.into_inner())
}

/// For preview `gen`: a monitor still on from an earlier preview (its restore has not run yet) is handed
/// over to this one; a monitor that supports HDR and has it off is recorded, then switched on. Does
/// nothing once the preview is gone (`current` false; returns false).
fn switch_on(
    list: &Owned,
    gdi: &str,
    gen: u64,
    current: impl Fn() -> bool,
    get: impl Fn(&str) -> HdrState,
    set: impl Fn(&str, bool) -> bool,
) -> bool {
    let mut l = guard(list);
    if !current() {
        return false;
    }
    if let Some(owned) = l.iter_mut().find(|(g, _)| g == gdi) {
        owned.1 = gen;
        return true;
    }
    let before = get(gdi);
    if before.supported && !before.enabled {
        l.push((gdi.to_string(), gen)); // kept even if switching fails: switching off an off monitor is a no-op
        set(gdi, true);
    }
    true
}

/// Switches off the monitors owned by previews up to `upto`, and forgets only those confirmed off, so the
/// exit cleanup tries again for any that failed.
fn switch_off(list: &Owned, upto: u64, set: impl Fn(&str, bool) -> bool) {
    guard(list).retain(|(gdi, gen)| *gen > upto || !set(gdi, false));
}

/// Whether the monitor ended up in the requested state.
fn set_hdr(gdi: &str, on: bool) -> bool {
    hdr::set(gdi, on).is_ok_and(|s| s.enabled == on)
}

/// Switches HDR back off wherever previews up to `upto` switched it on (`u64::MAX`: all). Blocks until done.
pub fn hdr_restore(upto: u64) {
    switch_off(&HDR_ON, upto, set_hdr);
}

fn current(app: &AppHandle, gen: u64) -> bool {
    preview::lock(app)
        .preview
        .as_ref()
        .is_some_and(|p| p.gen == gen)
}

/// Switches HDR on for the preview's monitors that support it and have it off, while the preview is open.
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
        if !switch_on(&HDR_ON, &gdi, gen, || current(app, gen), hdr::get, set_hdr) {
            return;
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
    use std::sync::Arc;

    const OFF: HdrState = HdrState {
        supported: true,
        enabled: false,
    };

    #[test]
    fn a_restore_waits_for_a_switch_on_in_progress() {
        let list: Arc<Owned> = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::new(Mutex::new(Vec::<(String, bool)>::new()));
        let (l, g) = (list.clone(), log.clone());
        let slow_on = std::thread::spawn(move || {
            switch_on(
                &l,
                "A",
                1,
                || true,
                |_| OFF,
                |gdi, on| {
                    std::thread::sleep(Duration::from_millis(200));
                    g.lock().unwrap().push((gdi.into(), on));
                    true
                },
            )
        });
        std::thread::sleep(Duration::from_millis(50)); // the switch-on is under way
        switch_off(&list, 1, |gdi, on| {
            log.lock().unwrap().push((gdi.into(), on));
            true
        });
        assert!(slow_on.join().unwrap());
        assert_eq!(
            *log.lock().unwrap(),
            [("A".to_string(), true), ("A".to_string(), false)]
        );
        assert!(list.lock().unwrap().is_empty());
        // Once the preview is gone, nothing is switched on or recorded.
        let never = |_: &str, _: bool| -> bool { panic!("switched") };
        assert!(!switch_on(&list, "B", 2, || false, |_| OFF, never));
        assert!(list.lock().unwrap().is_empty());
    }

    #[test]
    fn a_reopened_preview_takes_over_hdr_the_last_one_has_not_restored() {
        let list: Owned = Mutex::new(vec![("A".to_string(), 1)]);
        let on = HdrState {
            supported: true,
            enabled: true,
        };
        let never = |_: &str, _: bool| -> bool { panic!("switched") };
        assert!(switch_on(&list, "A", 2, || true, |_| on, never));
        switch_off(&list, 1, never); // preview 1's late restore leaves it alone
        assert_eq!(*list.lock().unwrap(), [("A".to_string(), 2)]);
        switch_off(&list, 2, |_, _| true);
        assert!(list.lock().unwrap().is_empty());
    }

    #[test]
    fn a_failed_restore_is_kept_for_the_exit_cleanup() {
        let list: Owned = Mutex::new(vec![("A".to_string(), 1), ("B".to_string(), 1)]);
        switch_off(&list, 1, |gdi, _| gdi == "B");
        assert_eq!(*list.lock().unwrap(), [("A".to_string(), 1)]);
        switch_off(&list, u64::MAX, |_, _| true);
        assert!(list.lock().unwrap().is_empty());
    }

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
