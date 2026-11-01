// preview: the open full-screen preview (job, tiles, selection, grid, HDR we switched on) and the identify
// labels, as Tauri managed state. The pure part lives here; the windows themselves are in preview_win.rs.
#![allow(dead_code)] // called from commands.rs once the pass-1 branches are merged
use crate::model::*;
use crate::preview_win as win;
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Emitter, Manager};

pub struct Preview {
    /// Bumped on every open, so late window events and threads of an older preview do nothing.
    pub gen: u64,
    pub job: ExportJob,
    pub image: ImageInfo,
    pub monitors: Vec<MonitorInfo>,
    pub tiles: Vec<Tile>,
    /// Indices into job.monitors that are connected and get a window, label `preview-<index>`.
    pub windows: Vec<usize>,
    pub selected: usize,
    pub grid: bool,
    /// gdi names of the monitors whose HDR this preview switched on, to switch off again.
    pub hdr_on: Vec<String>,
}

impl Preview {
    pub fn new(gen: u64, job: ExportJob, image: ImageInfo, monitors: Vec<MonitorInfo>) -> Preview {
        let windows: Vec<usize> = (0..job.monitors.len())
            .filter(|&i| connected(&monitors, &job.monitors[i].id).is_some())
            .collect();
        let mut p = Preview {
            gen,
            selected: windows.first().copied().unwrap_or(0),
            windows,
            job,
            image,
            monitors,
            tiles: Vec::new(),
            grid: false,
            hdr_on: Vec::new(),
        };
        p.retile();
        p
    }

    fn retile(&mut self) {
        let j = &self.job;
        let w = (self.image.width, self.image.height);
        self.tiles = crate::geometry::compute_layout(w.0, w.1, &j.monitors, &j.framing).tiles;
    }

    /// `next` moves the selection to the next window; otherwise the selected monitor moves by (dx, dy).
    pub fn nudge(&mut self, dx: f64, dy: f64, next: bool) {
        if next {
            let at = self.windows.iter().position(|&i| i == self.selected);
            let n = self.windows.len().max(1);
            self.selected = self
                .windows
                .get(at.map_or(0, |a| (a + 1) % n))
                .copied()
                .unwrap_or(0);
            return;
        }
        if let Some(m) = self.job.monitors.get_mut(self.selected) {
            m.nudge_px.x += dx;
            m.nudge_px.y += dy;
            self.retile();
        }
    }

    /// The connected monitor behind window `i`; its `px` is where the window goes (physical pixels).
    pub fn window_monitor(&self, i: usize) -> Option<&MonitorInfo> {
        connected(&self.monitors, &self.job.monitors.get(i)?.id)
    }

    pub fn state_for(&self, i: usize) -> Option<PreviewState> {
        let monitor = self.job.monitors.get(i)?.clone();
        let tile = self
            .tiles
            .iter()
            .find(|t| t.monitor_id == monitor.id)?
            .clone();
        Some(PreviewState::Preview {
            preview_path: self.image.preview_path.clone(),
            image_w: self.image.width,
            image_h: self.image.height,
            tile,
            monitor,
            fill: self.job.prefs.fill.clone(),
            grid: self.grid,
            selected: self.selected == i,
        })
    }
}

fn connected<'a>(monitors: &'a [MonitorInfo], id: &str) -> Option<&'a MonitorInfo> {
    monitors.iter().find(|m| m.id.eq_ignore_ascii_case(id))
}

#[derive(Default)]
pub struct Inner {
    pub preview: Option<Preview>,
    pub gen: u64,
    pub identify: Vec<MonitorInfo>,
    pub identify_gen: u64,
}

#[derive(Default)]
struct Shared(Mutex<Inner>);

pub fn lock(app: &AppHandle) -> MutexGuard<'_, Inner> {
    if app.try_state::<Shared>().is_none() {
        app.manage(Shared::default());
    }
    let s = app.state::<Shared>().inner();
    s.0.lock().unwrap_or_else(|e| e.into_inner())
}

/// Opens one borderless window per connected monitor of the job (and switches HDR on first when the
/// preference asks for it and the image is HDR). Windows are created on a helper thread, because on
/// Windows building a webview from a synchronous command deadlocks.
pub fn open(
    app: &AppHandle,
    job: ExportJob,
    image: ImageInfo,
    monitors: Vec<MonitorInfo>,
) -> Result<(), String> {
    close(app)?;
    let auto_hdr = job.prefs.auto_hdr_preview && image.hdr;
    let gen = {
        let mut s = lock(app);
        s.gen += 1;
        let p = Preview::new(s.gen, job, image, monitors);
        if p.windows.is_empty() {
            return Err("None of these monitors is connected now.".into());
        }
        s.preview = Some(p);
        s.gen
    };
    let app = app.clone();
    std::thread::spawn(move || win::open_windows(&app, gen, auto_hdr));
    Ok(())
}

/// Closes every preview window, switches HDR back off where the preview switched it on, and tells the UI.
pub fn close(app: &AppHandle) -> Result<(), String> {
    let gen = lock(app).preview.as_ref().map(|p| p.gen);
    close_gen(app, gen.unwrap_or(0));
    win::close_labeled(app, "preview-"); // and any window an interrupted open left behind
    Ok(())
}

/// Closes the preview only if it is still generation `gen` (window events of an older one are ignored).
pub fn close_gen(app: &AppHandle, gen: u64) {
    let taken = {
        let mut s = lock(app);
        match &s.preview {
            Some(p) if p.gen == gen => s.preview.take(),
            _ => None,
        }
    };
    if let Some(p) = taken {
        win::close_labeled(app, "preview-");
        std::thread::spawn(move || win::hdr_off(&p.hdr_on));
        let _ = app.emit("preview-closed", ());
    }
}

/// What window `label` shows: `preview-<i>` or `identify-<i>`.
pub fn state(app: &AppHandle, label: &str) -> Result<PreviewState, String> {
    let s = lock(app);
    let index = |prefix: &str| label.strip_prefix(prefix)?.parse::<usize>().ok();
    if let Some(i) = index("preview-") {
        let p = s.preview.as_ref().ok_or("The preview is closed.")?;
        return p
            .state_for(i)
            .ok_or_else(|| format!("No tile for preview window {i}"));
    }
    let i = index("identify-").ok_or_else(|| format!("Unknown window {label}"))?;
    let m = s.identify.get(i).ok_or("Identify has finished.")?;
    Ok(PreviewState::Identify {
        number: i as u32 + 1,
        name: m.name.clone(),
        px_w: m.px.w as u32,
        px_h: m.px.h as u32,
    })
}

fn changed(app: &AppHandle, f: impl FnOnce(&mut Preview)) -> Result<(), String> {
    let monitors = {
        let mut s = lock(app);
        let p = s.preview.as_mut().ok_or("The preview is closed.")?;
        f(p);
        p.job.monitors.clone()
    };
    app.emit("preview-changed", monitors)
        .map_err(|e| e.to_string())
}

/// Tab (next) or an arrow key (dx, dy in output pixels); emits "preview-changed" with the calibration.
pub fn nudge(app: &AppHandle, dx: f64, dy: f64, next: bool) -> Result<(), String> {
    changed(app, |p| p.nudge(dx, dy, next))
}

pub fn toggle_grid(app: &AppHandle) -> Result<(), String> {
    changed(app, |p| p.grid = !p.grid)
}

/// A large number on each monitor for 2.5 s.
pub fn identify(app: &AppHandle, monitors: Vec<MonitorInfo>) -> Result<(), String> {
    let gen = {
        let mut s = lock(app);
        s.identify_gen += 1;
        s.identify = monitors;
        s.identify_gen
    };
    let app = app.clone();
    std::thread::spawn(move || win::identify_windows(&app, gen));
    Ok(())
}

/// For app exit: switch HDR back off right now, on this thread.
pub fn restore_hdr(app: &AppHandle) {
    let list = lock(app)
        .preview
        .as_mut()
        .map(|p| std::mem::take(&mut p.hdr_on));
    win::hdr_off(&list.unwrap_or_default());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(id: &str, x: f64) -> MonitorInfo {
        MonitorInfo {
            id: id.into(),
            gdi_name: format!(r"\\.\{id}"),
            name: id.into(),
            px: Rect {
                x,
                y: 0.0,
                w: 3840.0,
                h: 2160.0,
            },
            scale: 1.5,
            primary: x == 0.0,
            edid_mm: None,
            hdr: HdrState::default(),
        }
    }

    fn placed(id: &str) -> PlacedMonitor {
        PlacedMonitor {
            id: id.into(),
            px_w: 3840,
            px_h: 2160,
            mm: Rect {
                x: 0.0,
                y: 0.0,
                w: 600.0,
                h: 340.0,
            },
            nudge_px: Point::default(),
        }
    }

    fn preview(connected: &[&str]) -> Preview {
        let job = ExportJob {
            image: "a.jpg".into(),
            monitors: vec![placed("A"), placed("GONE"), placed("B")],
            framing: Framing::default(),
            prefs: Prefs::default(),
        };
        let image = ImageInfo {
            path: "a.jpg".into(),
            preview_path: "a.jpg".into(),
            width: 6000,
            height: 2000,
            format: "jpeg".into(),
            hdr: false,
        };
        let mons = connected
            .iter()
            .enumerate()
            .map(|(i, id)| info(id, i as f64 * 3840.0));
        Preview::new(1, job, image, mons.collect())
    }

    #[test]
    fn only_connected_monitors_get_windows_matched_ignoring_case() {
        let p = preview(&["a", "B"]);
        assert_eq!(p.windows, vec![0, 2]);
        assert_eq!(p.selected, 0);
        assert_eq!(p.window_monitor(2).unwrap().px.x, 3840.0);
        assert_eq!(p.window_monitor(1), None);
    }

    #[test]
    fn tab_cycles_through_windows_only() {
        let mut p = preview(&["A", "B"]);
        p.nudge(0.0, 0.0, true);
        assert_eq!(p.selected, 2);
        p.nudge(0.0, 0.0, true);
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn arrows_nudge_only_the_selected_monitor() {
        let mut p = preview(&["A", "B"]);
        p.nudge(0.0, 0.0, true);
        p.nudge(1.0, 0.0, false);
        p.nudge(1.0, -3.0, false);
        assert_eq!(p.job.monitors[2].nudge_px, Point { x: 2.0, y: -3.0 });
        assert_eq!(p.job.monitors[0].nudge_px, Point::default());
    }

    #[test]
    fn state_needs_a_tile_and_marks_the_selection() {
        let mut p = preview(&["A", "B"]);
        p.tiles = vec![Tile {
            monitor_id: "B".into(),
            crop: Rect {
                x: 3000.0,
                y: 0.0,
                w: 3000.0,
                h: 1687.5,
            },
            out_w: 3840,
            out_h: 2160,
            upscale: 1.28,
        }];
        assert_eq!(p.state_for(0), None);
        let Some(PreviewState::Preview {
            selected,
            tile,
            fill,
            ..
        }) = p.state_for(2)
        else {
            panic!("no state");
        };
        assert!(!selected);
        assert_eq!(tile.out_w, 3840);
        assert_eq!(fill, "#000000");
    }
}
