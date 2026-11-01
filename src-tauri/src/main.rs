// Wallpaper Splitter: the Tauri shell. Module map (one job each; see CLAUDE.md):
//   model      contract types shared with ui/ipc.ts
//   commands   the Tauri commands, thin wrappers over the modules below
//   geometry   desk millimetres -> per-monitor crop rectangles and the span canvas (the only layout maths)
//   monitors   connected monitors, EDID physical size, ids
//   edid       EDID parsing: monitor name and size in mm (pure)
//   decode     every source format -> linear BT.709 f32 (+ SDR rendition)
//   color      transfer functions, gamut matrices, tone mapping
//   export     resize, PNG/JPG/JXR writing, file naming
//   wallpaper  IDesktopWallpaper, JPEGImportQuality
//   hdr        DisplayConfig HDR state per monitor
//   settings   %APPDATA%\WallpaperSplitter\settings.json, atomic writes
//   preview    the open full-screen preview and identify state (Tauri managed state)
//   preview_win  the per-monitor borderless windows and the HDR switch around the preview
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod color;
mod commands;
mod decode;
mod edid;
mod export;
mod geometry;
mod hdr;
mod model;
mod monitors;
mod preview;
mod preview_win;
mod settings;
mod wallpaper;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_monitors,
            commands::open_image,
            commands::pick_image,
            commands::compute_layout,
            commands::default_placement,
            commands::preview_state,
            commands::preview_nudge,
            commands::preview_grid,
            commands::get_settings,
            commands::save_settings,
            commands::set_hdr,
            commands::preview_open,
            commands::preview_close,
            commands::identify,
            commands::export,
            commands::apply,
            commands::open_folder,
        ])
        .run(tauri::generate_context!())
        .expect("Wallpaper Splitter failed to start");
}
