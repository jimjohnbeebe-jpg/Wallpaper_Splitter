// The Tauri commands (the contract in ui/ipc.ts). Each is a thin wrapper over its module; errors reach
// the UI as plain sentences it can show as they are.
use crate::model::*;

type Res<T> = Result<T, String>;

fn todo<T>(what: &str) -> Res<T> {
    Err(format!("{what} is not built yet"))
}

#[tauri::command]
pub fn list_monitors() -> Res<Vec<MonitorInfo>> {
    todo("Monitor detection")
}

#[tauri::command]
pub fn open_image(_app: tauri::AppHandle, _path: String) -> Res<ImageInfo> {
    todo("Opening images")
}

#[tauri::command]
pub async fn pick_image(_app: tauri::AppHandle) -> Res<Option<String>> {
    todo("The file picker")
}

#[tauri::command]
pub fn compute_layout(
    width: u32,
    height: u32,
    monitors: Vec<PlacedMonitor>,
    framing: Framing,
) -> Res<Layout> {
    let _ = (width, height, monitors, framing);
    todo("Layout")
}

#[tauri::command]
pub fn get_settings() -> Res<Settings> {
    Ok(Settings::default())
}

#[tauri::command]
pub fn save_settings(_settings: Settings) -> Res<()> {
    todo("Saving settings")
}

#[tauri::command]
pub fn set_hdr(_monitor_id: String, _on: bool) -> Res<HdrState> {
    todo("Switching HDR")
}

#[tauri::command]
pub fn preview_open(_app: tauri::AppHandle, _job: ExportJob) -> Res<()> {
    todo("The full-screen preview")
}

#[tauri::command]
pub fn preview_close(_app: tauri::AppHandle) -> Res<()> {
    todo("The full-screen preview")
}

#[tauri::command]
pub fn identify(_app: tauri::AppHandle) -> Res<()> {
    todo("Identify")
}

#[tauri::command]
pub async fn export(_job: ExportJob) -> Res<ExportResult> {
    todo("Export")
}

#[tauri::command]
pub async fn apply(_job: ExportJob) -> Res<ExportResult> {
    todo("Set wallpaper")
}

#[tauri::command]
pub fn open_folder(path: String) -> Res<()> {
    std::process::Command::new("explorer.exe")
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open {path}: {e}"))
}
