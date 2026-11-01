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
export type Layout = { tiles: Tile[] };
export type Align = "top" | "center" | "bottom";

/** What a secondary window (label preview-<i> or identify-<i>, page preview.html) shows. */
export type PreviewState =
  | {
      kind: "preview";
      previewPath: string;
      imageW: number;
      imageH: number;
      tile: Tile;
      monitor: PlacedMonitor;
      fill: string;
      grid: boolean;
      selected: boolean; // Tab cycles the selected monitor; arrow keys nudge it
    }
  | { kind: "identify"; number: number; name: string; pxW: number; pxH: number };

export type ExportFormat = { kind: "png" } | { kind: "jpg"; quality: number } | { kind: "jxr" };

export type Prefs = {
  format: ExportFormat;
  folder: string; // "" = Pictures\Wallpaper Splitter\<image>
  template: string; // tokens: {image} {monitor} {index} {w} {h}
  span: boolean; // also write one spanning image
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

/** Events from the backend: "preview-changed" (payload PlacedMonitor[]: the nudged calibration, for the
 *  main window to keep and save; preview windows refetch their state), "preview-closed" (no payload),
 *  and Tauri's own drag-drop event on the main window. */
type Tauri = {
  core: { invoke: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>; convertFileSrc: (path: string) => string };
  event: { listen: <T>(event: string, cb: (e: { payload: T }) => void) => Promise<() => void> };
  window: { getCurrentWindow: () => { label: string; onCloseRequested: (cb: () => Promise<void>) => Promise<() => void> } };
  webviewWindow: {
    getCurrentWebviewWindow: () => { onDragDropEvent: (cb: (e: { payload: DragDrop }) => void) => Promise<() => void> };
  };
  dialog?: { open: (o: { directory?: boolean; title?: string }) => Promise<string | string[] | null> };
};
const tauri = (): Tauri => (globalThis as unknown as { __TAURI__: Tauri }).__TAURI__;
const call = <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => tauri().core.invoke<T>(cmd, args);

export const fileUrl = (path: string): string => tauri().core.convertFileSrc(path);
export const listen = <T>(event: string, cb: (payload: T) => void): Promise<() => void> =>
  tauri().event.listen<T>(event, (e) => cb(e.payload));
export const windowLabel = (): string => tauri().window.getCurrentWindow().label;
/** Runs `before` when this window is asked to close, and closes it once that has finished. */
export const onCloseRequested = (before: () => Promise<void>): Promise<() => void> =>
  tauri().window.getCurrentWindow().onCloseRequested(before);

/** Tauri's file drag-and-drop on the current window (UI helper, not a backend command). */
export type DragDrop =
  | { type: "enter" | "drop"; paths: string[]; position: { x: number; y: number } }
  | { type: "over"; position: { x: number; y: number } }
  | { type: "leave" };
export const onDragDrop = (cb: (e: DragDrop) => void): Promise<() => void> =>
  tauri().webviewWindow.getCurrentWebviewWindow().onDragDropEvent((e) => cb(e.payload));
/** The dialog plugin's folder picker, when its global script is present (withGlobalTauri). */
export const hasFolderPicker = (): boolean => tauri().dialog !== undefined;
export const pickFolder = async (): Promise<string | null> => {
  const r = await tauri().dialog?.open({ directory: true, title: "Choose the export folder" });
  return typeof r === "string" ? r : null;
};

export const api = {
  listMonitors: () => call<MonitorInfo[]>("list_monitors"),
  openImage: (path: string) => call<ImageInfo>("open_image", { path }),
  pickImage: () => call<string | null>("pick_image"),
  computeLayout: (width: number, height: number, monitors: PlacedMonitor[], framing: Framing) =>
    call<Layout>("compute_layout", { width, height, monitors, framing }),
  defaultPlacement: (monitors: MonitorInfo[], align: Align, bezelMm: number) =>
    call<PlacedMonitor[]>("default_placement", { monitors, align, bezelMm }),
  previewState: (label: string) => call<PreviewState>("preview_state", { label }),
  /** Nudges the selected monitor (or cycles selection with dx=dy=0 and next=true); emits "preview-changed" with PlacedMonitor[]. */
  previewNudge: (dx: number, dy: number, next: boolean) => call<void>("preview_nudge", { dx, dy, next }),
  previewGrid: () => call<void>("preview_grid"),
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
