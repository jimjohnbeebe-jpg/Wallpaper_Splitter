// A fake backend for vitest only: Jim's two 4K monitors, settings in memory, every call logged.
// The layout here is a crude stand-in (10 source px per mm); the real maths is geometry.rs.
import type { Layout, MonitorInfo, PlacedMonitor, Settings } from "./ipc.ts";
import type { Api } from "./state.ts";
import { defaultPrefs } from "./state.ts";

export const benq: MonitorInfo = {
  id: "\\\\?\\DISPLAY#BNQ7950#1", gdiName: "\\\\.\\DISPLAY1", name: "BenQ EW3270U",
  px: { x: 0, y: 0, w: 3840, h: 2160 }, scale: 1.5, primary: true, edidMm: { w: 698, h: 393 }, hdr: { supported: true, enabled: false },
};
export const samsung: MonitorInfo = {
  id: "\\\\?\\DISPLAY#SAM0F12#2", gdiName: "\\\\.\\DISPLAY2", name: "LU28R55",
  px: { x: 3840, y: 0, w: 3840, h: 2160 }, scale: 1.5, primary: false, edidMm: { w: 632, h: 356 }, hdr: { supported: true, enabled: false },
};

export function mockApi(settings?: Partial<Settings>) {
  const calls: [string, unknown[]][] = [];
  const log = <T>(name: string, args: unknown[], value: T): Promise<T> => { calls.push([name, args]); return Promise.resolve(value); };
  let stored: Settings = { version: 1, prefs: defaultPrefs(), last: null, recent: [], calibrations: {}, ...settings };
  const monitors = [benq, samsung];
  const place = (ms: MonitorInfo[]): PlacedMonitor[] => {
    let x = 0;
    return ms.map((m) => {
      const mm = { x, y: 0, w: m.edidMm?.w ?? 600, h: m.edidMm?.h ?? 340 };
      x += mm.w;
      return { id: m.id, pxW: m.px.w, pxH: m.px.h, mm, nudgePx: { x: 0, y: 0 } };
    });
  };
  const layout = (ms: PlacedMonitor[]): Layout => ({
    tiles: ms.map((m) => ({ monitorId: m.id, crop: { x: m.mm.x * 10, y: m.mm.y * 10, w: m.mm.w * 10, h: m.mm.h * 10 }, outW: m.pxW, outH: m.pxH, upscale: m.pxW / (m.mm.w * 10) })),
  });
  const result = { files: monitors.map((m) => ({ monitorId: m.id, path: `C:\\out\\${m.name}.png` })), spanFile: null, folder: "C:\\out" };
  const api: Api = {
    listMonitors: () => log("listMonitors", [], monitors),
    openImage: (path) => log("openImage", [path], { path, previewPath: path, width: 6000, height: 2000, format: "jpeg", hdr: false }),
    pickImage: () => log("pickImage", [], null),
    computeLayout: (w, h, ms, f) => log("computeLayout", [w, h, ms, f], layout(ms)),
    defaultPlacement: (ms, a, b) => log("defaultPlacement", [ms, a, b], place(ms)),
    previewState: (label) => log("previewState", [label], { kind: "identify" as const, number: 1, name: benq.name, pxW: 3840, pxH: 2160 }),
    previewNudge: (dx, dy, next) => log("previewNudge", [dx, dy, next], undefined),
    previewGrid: () => log("previewGrid", [], undefined),
    getSettings: () => log("getSettings", [], structuredClone(stored)),
    saveSettings: (s) => { stored = structuredClone(s); return log("saveSettings", [s], undefined); },
    setHdr: (id, on) => log("setHdr", [id, on], { supported: true, enabled: on }),
    previewOpen: (job) => log("previewOpen", [job], undefined),
    previewClose: () => log("previewClose", [], undefined),
    identify: () => log("identify", [], undefined),
    exportTiles: (job) => log("exportTiles", [job], result),
    apply: (job) => log("apply", [job], result),
    openFolder: (path) => log("openFolder", [path], undefined),
  };
  return { api, calls, stored: () => stored, count: (name: string) => calls.filter((c) => c[0] === name).length };
}
