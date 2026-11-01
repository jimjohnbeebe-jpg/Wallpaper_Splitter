// The contract between the UI and the Rust backend (src-tauri/src/model.rs mirrors these types in
// camelCase; src-tauri/src/commands.rs registers the commands). Change both sides together.

export type Rect = { x: number; y: number; w: number; h: number };
export type HdrState = { supported: boolean; enabled: boolean };

/** One connected monitor, as Windows reports it. `px` is in physical pixels of the virtual desktop. */
export type MonitorInfo = {
  id: string; // monitor device-interface path; also the IDesktopWallpaper monitor ID
  gdiName: string; // \\.\DISPLAYn
  name: string; // friendly model name from EDID, e.g. "BenQ EW3270U"
  px: Rect;
  scale: number; // Windows display scale, 1.5 = 150 %
  primary: boolean;
  edidMm: { w: number; h: number } | null; // physical size from EDID, null when missing or bogus
  hdr: HdrState;
};

/** Where a monitor physically sits on the desk, in millimetres (the user's calibration). */
export type PlacedMonitor = {
  id: string;
  pxW: number;
  pxH: number;
  mm: Rect; // x, y of the top-left corner on the desk and the visible panel size
  nudgePx: { x: number; y: number }; // fine-tune, in output pixels
};

export type Fit = "cover" | "contain";
/** How the image sits on the desk: zoom multiplies the fit; offset moves the image centre, in mm. */
export type Framing = { fit: Fit; zoom: number; offsetMm: { x: number; y: number } };

export type ImageInfo = {
  path: string;
  previewPath: string; // the file the <img> shows: the original, or a proxy for formats WebView2 cannot read
  width: number;
  height: number;
  format: string; // "jpeg" | "ultrahdr" | "png" | "tiff" | "webp" | "avif" | "jxl"
  hdr: boolean;
};

export type Tile = {
  monitorId: string;
  crop: Rect; // in source-image pixels; may reach outside the image (filled with the fill colour)
  outW: number;
  outH: number;
  upscale: number; // outW / crop.w; above 1 means the source is enlarged
};
export type Layout = { tiles: Tile[]; span: { w: number; h: number; originX: number; originY: number } };

export type ExportFormat = { kind: "png" } | { kind: "jpg"; quality: number } | { kind: "jxr" };

export type Prefs = {
  format: ExportFormat;
  folder: string; // "" = Pictures\Wallpaper Splitter\<image>
  template: string; // tokens: {image} {monitor} {index} {w} {h}
  span: boolean; // also write one spanning image
  applyAfterExport: boolean;
  fill: string; // #rrggbb for uncovered areas
  jpegImportFix: boolean; // HKCU\Control Panel\Desktop\JPEGImportQuality = 100
  hdrOutput: boolean; // write JXR when the source is HDR
  autoHdrPreview: boolean; // switch HDR on for the full-screen preview, restore after
  sdrWhiteNits: number | null; // null = the Windows setting
  hdrPreview: boolean; // in-app dynamic-range-limit: no-limit
  reopenLast: boolean;
};

export type Session = { image: string; framing: Framing };
export type Settings = {
  version: 1;
  prefs: Prefs;
  last: Session | null;
  recent: Session[]; // newest first, at most 8
  calibrations: Record<string, PlacedMonitor[]>; // key: sorted monitor ids joined by "|"
};

export type ExportJob = { image: string; monitors: PlacedMonitor[]; framing: Framing; prefs: Prefs };
export type ExportResult = { files: { monitorId: string; path: string }[]; spanFile: string | null; folder: string };

type Tauri = {
  core: { invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>; convertFileSrc: (path: string) => string };
};
const tauri = (): Tauri => (globalThis as unknown as { __TAURI__: Tauri }).__TAURI__;
const call = <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => tauri().core.invoke<T>(cmd, args);

export const fileUrl = (path: string): string => tauri().core.convertFileSrc(path);

export const api = {
  listMonitors: () => call<MonitorInfo[]>("list_monitors"),
  openImage: (path: string) => call<ImageInfo>("open_image", { path }),
  pickImage: () => call<string | null>("pick_image"),
  computeLayout: (width: number, height: number, monitors: PlacedMonitor[], framing: Framing) =>
    call<Layout>("compute_layout", { width, height, monitors, framing }),
  getSettings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<void>("save_settings", { settings }),
  setHdr: (monitorId: string, on: boolean) => call<HdrState>("set_hdr", { monitorId, on }),
  previewOpen: (job: ExportJob) => call<void>("preview_open", { job }),
  previewClose: () => call<void>("preview_close"),
  identify: () => call<void>("identify"),
  exportTiles: (job: ExportJob) => call<ExportResult>("export", { job }),
  apply: (job: ExportJob) => call<ExportResult>("apply", { job }),
  openFolder: (path: string) => call<void>("open_folder", { path }),
};
