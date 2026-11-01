// The contract types shared with the UI (ui/ipc.ts mirrors them, camelCase on the wire).
// Change both sides together.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct HdrState {
    pub supported: bool,
    pub enabled: bool,
}

/// One connected monitor as Windows reports it; `px` is in physical pixels of the virtual desktop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub id: String,
    pub gdi_name: String,
    pub name: String,
    pub px: Rect,
    pub scale: f64,
    pub primary: bool,
    pub edid_mm: Option<Size>,
    pub hdr: HdrState,
}

/// Where a monitor physically sits on the desk, in millimetres (the user's calibration).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacedMonitor {
    pub id: String,
    pub px_w: u32,
    pub px_h: u32,
    pub mm: Rect,
    pub nudge_px: Point,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    #[default]
    Cover,
    Contain,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Framing {
    pub fit: Fit,
    pub zoom: f64,
    pub offset_mm: Point,
}

impl Default for Framing {
    fn default() -> Self {
        Framing {
            fit: Fit::Cover,
            zoom: 1.0,
            offset_mm: Point::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    pub path: String,
    pub preview_path: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub hdr: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tile {
    pub monitor_id: String,
    pub crop: Rect,
    pub out_w: u32,
    pub out_h: u32,
    pub upscale: f64,
}

/// The spanning image: the bounding box of every monitor's Windows pixel rectangle. Tile i is pasted at
/// (monitor.px.x - origin_x, monitor.px.y - origin_y). Built by geometry::span_canvas, used by export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub w: u32,
    pub h: u32,
    pub origin_x: i32,
    pub origin_y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub tiles: Vec<Tile>,
}

/// Starting arrangement for the Monitors page: Windows' left-to-right/top-to-bottom order, edges
/// touching plus `bezel_mm`, aligned by top, centre or bottom edge (the old script's three modes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Top,
    #[default]
    Center,
    Bottom,
}

/// What a secondary window (label `preview-<i>` or `identify-<i>`, page `preview.html`) shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PreviewState {
    #[serde(rename_all = "camelCase")]
    Preview {
        preview_path: String,
        image_w: u32,
        image_h: u32,
        tile: Tile,
        monitor: PlacedMonitor,
        fill: String,
        grid: bool,
        selected: bool,
    },
    #[serde(rename_all = "camelCase")]
    Identify {
        number: u32,
        name: String,
        px_w: u32,
        px_h: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ExportFormat {
    Png,
    Jpg { quality: u8 },
    Jxr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Prefs {
    pub format: ExportFormat,
    pub folder: String,
    pub template: String,
    pub span: bool,
    pub apply_after_export: bool,
    pub fill: String,
    pub jpeg_import_fix: bool,
    pub hdr_output: bool,
    pub auto_hdr_preview: bool,
    pub sdr_white_nits: Option<f64>,
    pub hdr_preview: bool,
    pub reopen_last: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            format: ExportFormat::Png,
            folder: String::new(),
            template: "{image}_{monitor}_{w}x{h}".into(),
            span: false,
            apply_after_export: true,
            fill: "#000000".into(),
            jpeg_import_fix: false,
            hdr_output: true,
            auto_hdr_preview: true,
            sdr_white_nits: None,
            hdr_preview: true,
            reopen_last: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub image: String,
    pub framing: Framing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub prefs: Prefs,
    pub last: Option<Session>,
    pub recent: Vec<Session>,
    pub calibrations: BTreeMap<String, Vec<PlacedMonitor>>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: 1,
            prefs: Prefs::default(),
            last: None,
            recent: Vec::new(),
            calibrations: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportJob {
    pub image: String,
    pub monitors: Vec<PlacedMonitor>,
    pub framing: Framing,
    pub prefs: Prefs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedFile {
    pub monitor_id: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub files: Vec<ExportedFile>,
    pub span_file: Option<String>,
    pub folder: String,
}
