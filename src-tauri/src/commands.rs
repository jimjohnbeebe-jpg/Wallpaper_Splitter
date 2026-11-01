// The Tauri commands (the contract in ui/ipc.ts). Each is a thin wrapper over its module; errors reach
// the UI as plain sentences it can show as they are. Synchronous commands run on the main thread, so
// anything that decodes, writes files, waits on Windows or uses COM is async and runs on a blocking thread.
use crate::model::*;
use crate::{decode, export as exp, geometry, hdr, monitors, preview, settings, wallpaper};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

type Res<T> = Result<T, String>;

/// Runs `f` on Tauri's blocking pool.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| format!("The background task failed: {e}"))?
}

/// Lets the WebView load `path` through the asset protocol (its scope starts empty).
fn allow(app: &AppHandle, path: &str) {
    let _ = app.asset_protocol_scope().allow_file(path);
}

fn open_allowed(app: &AppHandle, path: &str) -> Res<ImageInfo> {
    let info = decode::open(path, &settings::cache_dir())?;
    allow(app, &info.path);
    allow(app, &info.preview_path);
    Ok(info)
}

/// The user's preference, else Windows' SDR brightness on the primary monitor when HDR is on there
/// (it reads 80 while HDR is off, which means "default"), else the 203-nit reference white.
fn sdr_white(prefs: &Prefs, list: &[MonitorInfo]) -> f64 {
    prefs
        .sdr_white_nits
        .or_else(|| {
            let primary = list.iter().find(|m| m.primary)?;
            hdr::sdr_white_nits(&primary.gdi_name).filter(|n| *n > 80.5)
        })
        .unwrap_or(203.0)
}

fn run_export(job: &ExportJob) -> Res<ExportResult> {
    let list = monitors::list();
    exp::export(job, &list, sdr_white(&job.prefs, &list))
}

#[tauri::command]
pub async fn list_monitors() -> Res<Vec<MonitorInfo>> {
    blocking(|| Ok(monitors::list())).await
}

#[tauri::command]
pub async fn open_image(app: AppHandle, path: String) -> Res<ImageInfo> {
    blocking(move || open_allowed(&app, &path)).await
}

#[tauri::command]
pub async fn pick_image(app: AppHandle) -> Res<Option<String>> {
    blocking(move || {
        let picked = app
            .dialog()
            .file()
            .set_title("Open an image")
            .add_filter(
                "Images",
                &["jpg", "jpeg", "png", "tif", "tiff", "webp", "avif", "jxl"],
            )
            .blocking_pick_file();
        Ok(picked.map(|p| p.to_string()))
    })
    .await
}

#[tauri::command]
pub fn compute_layout(
    width: u32,
    height: u32,
    monitors: Vec<PlacedMonitor>,
    framing: Framing,
) -> Res<Layout> {
    Ok(geometry::compute_layout(width, height, &monitors, &framing))
}

#[tauri::command]
pub fn default_placement(
    monitors: Vec<MonitorInfo>,
    align: Align,
    bezel_mm: f64,
) -> Res<Vec<PlacedMonitor>> {
    Ok(geometry::default_placement(&monitors, align, bezel_mm))
}

#[tauri::command]
pub fn preview_state(app: AppHandle, label: String) -> Res<PreviewState> {
    preview::state(&app, &label)
}

#[tauri::command]
pub fn preview_nudge(app: AppHandle, dx: f64, dy: f64, next: bool) -> Res<()> {
    preview::nudge(&app, dx, dy, next)
}

#[tauri::command]
pub fn preview_grid(app: AppHandle) -> Res<()> {
    preview::toggle_grid(&app)
}

/// Also lets the WebView show the recent images' thumbnails.
#[tauri::command]
pub fn get_settings(app: AppHandle) -> Res<Settings> {
    let s = settings::load();
    for r in &s.recent {
        allow(&app, &r.image);
    }
    Ok(s)
}

#[tauri::command]
pub fn save_settings(settings: Settings) -> Res<()> {
    settings::save(&settings)
}

#[tauri::command]
pub async fn set_hdr(monitor_id: String, on: bool) -> Res<HdrState> {
    blocking(move || {
        let list = monitors::list();
        let m = list
            .iter()
            .find(|m| m.id.eq_ignore_ascii_case(&monitor_id))
            .ok_or("That monitor is not connected now.")?;
        hdr::set(&m.gdi_name, on)
    })
    .await
}

#[tauri::command]
pub async fn preview_open(app: AppHandle, job: ExportJob) -> Res<()> {
    blocking(move || {
        let info = open_allowed(&app, &job.image)?;
        preview::open(&app, job, info, monitors::list())
    })
    .await
}

#[tauri::command]
pub fn preview_close(app: AppHandle) -> Res<()> {
    preview::close(&app)
}

#[tauri::command]
pub async fn identify(app: AppHandle) -> Res<()> {
    blocking(move || preview::identify(&app, monitors::list())).await
}

#[tauri::command]
pub async fn export(job: ExportJob) -> Res<ExportResult> {
    blocking(move || run_export(&job)).await
}

/// Exports, then sets the files as the wallpaper (one per monitor, or the span image across them all).
#[tauri::command]
pub async fn apply(job: ExportJob) -> Res<ExportResult> {
    blocking(move || {
        let result = run_export(&job)?;
        if matches!(job.prefs.format, ExportFormat::Jpg { .. }) && job.prefs.jpeg_import_fix {
            wallpaper::set_jpeg_import_quality_100()?;
        }
        match (&result.span_file, job.prefs.span) {
            (Some(span), true) => wallpaper::set_span(span)?,
            _ => {
                let files: Vec<(String, String)> = result
                    .files
                    .iter()
                    .map(|f| (f.monitor_id.clone(), f.path.clone()))
                    .collect();
                wallpaper::set_per_monitor(&files)?;
            }
        }
        Ok(result)
    })
    .await
}

#[tauri::command]
pub fn open_folder(path: String) -> Res<()> {
    std::process::Command::new("explorer.exe")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open {path}: {e}"))
}
