// App state: monitors, the placed calibration, the open image, framing, layout, settings. One store, one
// emit; pages subscribe and sync. Settings are saved 500 ms after the last change, and at once when the
// window closes (flush). No DOM in this file.
import type { Align, api as realApi, ExportJob, Framing, ImageInfo, Layout, MonitorInfo, PlacedMonitor, Prefs, Session, Settings } from "./ipc.ts";
import { clampZoom } from "./geom.ts";

export type Api = typeof realApi;
export type Busy = "apply" | "export" | "preview" | null;
export type Result = { kind: "ok" | "error"; text: string; folder?: string };
export type AppState = {
  monitors: MonitorInfo[];
  placed: PlacedMonitor[];
  image: ImageInfo | null;
  framing: Framing;
  layout: Layout | null;
  settings: Settings;
  result: Result | null;
  busy: Busy;
  selected: number; // selected pane index, -1 for none
  packaged: boolean; // the Store (MSIX) build
};

export const defaultFraming = (): Framing => ({ fit: "cover", zoom: 1, offsetMm: { x: 0, y: 0 } });
export const defaultPrefs = (): Prefs => ({
  format: { kind: "png" }, folder: "", template: "{image}_{monitor}_{w}x{h}", span: false,
  fill: "#000000", jpegImportFix: false, hdrOutput: true, autoHdrPreview: true, sdrWhiteNits: null, hdrPreview: true, reopenLast: true,
});
const defaultSettings = (): Settings => ({ version: 1, prefs: defaultPrefs(), last: null, recent: [], calibrations: {} });

/** The calibration key for a set of connected monitors: sorted ids joined by "|". */
export const calibrationKey = (ids: string[]): string => [...ids].sort().join("|");

const samePath = (a: string, b: string): boolean => a.toLowerCase() === b.toLowerCase(); // Windows paths
/** Puts `s` first, drops any older entry for the same image, keeps at most `max`. */
export const updateRecent = (recent: Session[], s: Session, max = 8): Session[] =>
  [s, ...recent.filter((r) => !samePath(r.image, s.image))].slice(0, max);

export const basename = (path: string): string => path.split(/[\\/]/).pop() ?? path;
export const stem = (path: string): string => basename(path).replace(/\.[^.]*$/, "");
export const errText = (e: unknown): string => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));
const plural = (n: number, one: string): string => `${n} ${one}${n === 1 ? "" : "s"}`;

/** The file name the export template gives, with the extension (spaces become hyphens, as in the plan's example). */
export function renderTemplate(template: string, v: { image: string; monitor: string; index: number; w: number; h: number }, ext: string): string {
  const dash = (s: string): string => s.trim().replace(/\s+/g, "-");
  const name = template
    .replaceAll("{image}", dash(v.image)).replaceAll("{monitor}", dash(v.monitor))
    .replaceAll("{index}", String(v.index)).replaceAll("{w}", String(v.w)).replaceAll("{h}", String(v.h));
  return `${name}.${ext}`;
}
export const extFor = (p: Prefs): string => p.format.kind;

/** `p` with the monitor's current pixel size; the mm size turns with it when the monitor was rotated
 *  (the same rule as geometry::physical_mm for EDID sizes). */
export function withPx(p: PlacedMonitor, w: number, h: number): PlacedMonitor {
  const turned = (p.mm.w > p.mm.h) !== (w > h) && p.mm.w !== p.mm.h && w !== h;
  return { ...p, pxW: w, pxH: h, mm: turned ? { ...p.mm, w: p.mm.h, h: p.mm.w } : p.mm };
}

const raf: (f: () => void) => void = (f) =>
  typeof requestAnimationFrame === "function" ? void requestAnimationFrame(() => f()) : void setTimeout(f, 16);

export function createApp(api: Api, saveDelay = 500) {
  const st: AppState = {
    monitors: [], placed: [], image: null, framing: defaultFraming(), layout: null,
    settings: defaultSettings(), result: null, busy: null, selected: -1, packaged: false,
  };
  const subs = new Set<() => void>();
  const emit = (): void => subs.forEach((f) => f());
  const fail = (e: unknown): void => { st.result = { kind: "error", text: errText(e) }; emit(); };

  let saveTimer: ReturnType<typeof setTimeout> | undefined;
  let saving: Promise<void> = Promise.resolve();
  const write = (): Promise<void> => {
    clearTimeout(saveTimer);
    saveTimer = undefined;
    saving = api.saveSettings(st.settings).catch(fail);
    return saving;
  };
  const save = (): void => {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void write(), saveDelay);
  };

  let layoutQueued = false;
  let seq = 0;
  let openSeq = 0; // only the newest open() applies its result
  const requestLayout = (): void => {
    if (!st.image || st.placed.length === 0 || layoutQueued) return;
    layoutQueued = true;
    raf(() => {
      layoutQueued = false;
      const img = st.image;
      if (!img) return;
      const mine = ++seq;
      api.computeLayout(img.width, img.height, st.placed, st.framing).then((l) => {
        if (mine !== seq || img !== st.image) return; // a newer request is on its way
        st.layout = l;
        emit();
      }, fail);
    });
  };

  const rememberSession = (): void => {
    if (!st.image) return;
    const s: Session = { image: st.image.path, framing: st.framing };
    st.settings.last = s;
    st.settings.recent = updateRecent(st.settings.recent, s);
  };

  const app = {
    state: st,
    subscribe(f: () => void): () => void { subs.add(f); return () => void subs.delete(f); },
    emit,
    fail,
    /** Saves a waiting change now (the window is closing); resolves once the save, or one already
     *  under way, has finished. Never rejects. */
    flush: (): Promise<void> => (saveTimer === undefined ? saving : write()),

    async start(): Promise<void> {
      try { st.settings = { ...defaultSettings(), ...(await api.getSettings()) }; } catch (e) { fail(e); }
      try { st.packaged = await api.isPackaged(); } catch { /* not inside Tauri: portable */ }
      try { await app.refreshMonitors(); } catch (e) { fail(e); }
      const last = st.settings.last;
      if (st.settings.prefs.reopenLast && last) await app.open(last.image, last.framing);
      emit();
    },

    /** Reads the monitors again; on a new set of monitors, loads its calibration (or the default one).
     *  Pixel sizes always follow the monitors, so a resolution or orientation change is picked up. */
    async refreshMonitors(): Promise<void> {
      const mons = await api.listMonitors();
      const key = calibrationKey(mons.map((m) => m.id));
      const known = calibrationKey(st.monitors.map((m) => m.id));
      st.monitors = mons;
      const byId = new Map(mons.map((m) => [m.id, m]));
      const fit = (p: PlacedMonitor): PlacedMonitor => { const m = byId.get(p.id); return m ? withPx(p, m.px.w, m.px.h) : p; };
      const before = JSON.stringify(st.placed);
      if (key !== known || st.placed.length === 0) {
        const saved = st.settings.calibrations[key];
        st.placed = saved && saved.every((p) => byId.has(p.id)) ? saved.map(fit) : await api.defaultPlacement(mons, "center", 0);
        st.selected = -1;
      } else st.placed = st.placed.map(fit);
      if (JSON.stringify(st.placed) !== before) requestLayout();
      emit();
    },

    async open(path: string, framing?: Framing): Promise<void> {
      const seq = ++openSeq;
      try {
        const info = await api.openImage(path);
        if (seq !== openSeq) return; // a newer open was started while this file was read
        const recent = st.settings.recent.find((r) => samePath(r.image, path));
        st.image = info;
        st.framing = framing ?? recent?.framing ?? defaultFraming();
        st.layout = null;
        st.result = null;
        rememberSession();
        save();
        requestLayout();
        emit();
      } catch (e) { if (seq === openSeq) fail(e); }
    },

    /** Ctrl+O: the system file picker, then open. */
    async pick(): Promise<void> {
      try {
        const path = await api.pickImage();
        if (path) await app.open(path);
      } catch (e) { fail(e); }
    },

    setFraming(patch: Partial<Framing>): void {
      st.framing = { ...st.framing, ...patch };
      st.framing.zoom = clampZoom(st.framing.zoom);
      rememberSession();
      save();
      requestLayout();
      emit();
    },

    /** Replaces the calibration for the connected monitors and saves it. */
    setPlaced(placed: PlacedMonitor[]): void {
      st.placed = placed;
      st.settings.calibrations = { ...st.settings.calibrations, [calibrationKey(st.monitors.map((m) => m.id))]: placed };
      save();
      requestLayout();
      emit();
    },
    updateMonitor(id: string, patch: (p: PlacedMonitor) => PlacedMonitor): void {
      app.setPlaced(st.placed.map((p) => (p.id === id ? patch(p) : p)));
    },

    setPrefs(patch: Partial<Prefs>): void {
      st.settings.prefs = { ...st.settings.prefs, ...patch };
      save();
      emit();
    },
    setSelected(i: number): void { st.selected = i; emit(); },

    /** The export job. An HDR image exports as JPEG XR when hdrOutput is on; JPEG XR falls back to PNG for an SDR image. */
    job(): ExportJob | null {
      if (!st.image) return null;
      const prefs = st.settings.prefs;
      const hdr = st.image.hdr;
      const format = hdr && prefs.hdrOutput ? { kind: "jxr" as const } : prefs.format.kind === "jxr" && !hdr ? { kind: "png" as const } : prefs.format;
      return { image: st.image.path, monitors: st.placed, framing: st.framing, prefs: { ...prefs, format } };
    },

    async run(kind: "apply" | "export" | "preview"): Promise<void> {
      const job = app.job();
      if (!job || st.busy) return;
      st.busy = kind;
      emit();
      try {
        if (kind === "preview") await api.previewOpen(job);
        else {
          const r = await (kind === "apply" ? api.apply(job) : api.exportTiles(job));
          const n = r.files.length + (r.spanFile ? 1 : 0);
          const text = kind === "apply" ? `Wallpaper set on ${plural(r.files.length, "monitor")}` : `Exported ${plural(n, "file")}`;
          st.result = { kind: "ok", text, folder: r.folder };
        }
      } catch (e) {
        st.result = { kind: "error", text: errText(e) };
      }
      st.busy = null;
      emit();
    },

    openFolder(path: string): void { api.openFolder(path).catch(fail); },
    identify(): void { api.identify().catch(fail); },

    /** Arranges the monitors again from Windows' order. keepSizes passes the current mm sizes in as if the
     *  monitors had reported them, so hand-set sizes and nudges survive an alignment change. */
    async arrange(align: Align, bezelMm: number, keepSizes: boolean): Promise<void> {
      try {
        const mons = keepSizes
          ? st.monitors.map((m) => { const p = st.placed.find((x) => x.id === m.id); return p ? { ...m, edidMm: { w: p.mm.w, h: p.mm.h } } : m; })
          : st.monitors;
        const placed = await api.defaultPlacement(mons, align, bezelMm);
        app.setPlaced(keepSizes ? placed.map((p) => ({ ...p, nudgePx: st.placed.find((x) => x.id === p.id)?.nudgePx ?? p.nudgePx })) : placed);
      } catch (e) { fail(e); }
    },

    async setHdr(id: string, on: boolean): Promise<void> {
      try {
        await api.setHdr(id, on);
        await app.refreshMonitors();
      } catch (e) { fail(e); }
    },
  };
  return app;
}
export type App = ReturnType<typeof createApp>;
