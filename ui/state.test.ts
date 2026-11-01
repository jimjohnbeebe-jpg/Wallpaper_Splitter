import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { benq, mockApi, samsung } from "./mock.ts";
import { calibrationKey, createApp, renderTemplate, updateRecent, withPx } from "./state.ts";

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

const f = (x: number) => ({ fit: "cover" as const, zoom: 1, offsetMm: { x, y: 0 } });

test("the calibration key is the sorted ids joined by |", () => {
  expect(calibrationKey(["b", "a", "c"])).toBe("a|b|c");
  expect(calibrationKey([samsung.id, benq.id])).toBe(calibrationKey([benq.id, samsung.id]));
});

test("recent images: newest first, no duplicates, at most 8", () => {
  let r = [] as ReturnType<typeof updateRecent>;
  for (let i = 0; i < 10; i++) r = updateRecent(r, { image: `C:\\p\\${i}.jpg`, framing: f(i) });
  expect(r).toHaveLength(8);
  expect(r[0]?.image).toBe("C:\\p\\9.jpg");
  r = updateRecent(r, { image: "c:\\P\\5.JPG", framing: f(55) });
  expect(r).toHaveLength(8);
  expect(r[0]?.framing.offsetMm.x).toBe(55);
  expect(r.filter((s) => s.image.toLowerCase() === "c:\\p\\5.jpg")).toHaveLength(1);
});

test("the template example renders every token", () => {
  const v = { image: "Zephyr", monitor: "BenQ EW3270U", index: 1, w: 3840, h: 2160 };
  expect(renderTemplate("{image}_{monitor}_{w}x{h}", v, "png")).toBe("Zephyr_BenQ-EW3270U_3840x2160.png");
  expect(renderTemplate("{index}-{image}", v, "jpg")).toBe("1-Zephyr.jpg");
});

test("start uses the default placement when nothing is saved, and reopens the last image with its framing", async () => {
  const m = mockApi({ last: { image: "C:\\p\\a.jpg", framing: f(12) } });
  const app = createApp(m.api);
  await app.start();
  expect(m.calls.find((c) => c[0] === "defaultPlacement")?.[1].slice(1)).toEqual(["center", 0]);
  expect(app.state.placed).toHaveLength(2);
  expect(app.state.image?.path).toBe("C:\\p\\a.jpg");
  expect(app.state.framing.offsetMm.x).toBe(12);
  await vi.advanceTimersByTimeAsync(20);
  expect(m.count("computeLayout")).toBe(1);
  expect(app.state.layout?.tiles).toHaveLength(2);
});

test("a saved calibration wins over the default placement", async () => {
  const saved = [{ id: benq.id, pxW: 1, pxH: 1, mm: { x: 5, y: 6, w: 7, h: 8 }, nudgePx: { x: 1, y: 2 } }, { id: samsung.id, pxW: 1, pxH: 1, mm: { x: 9, y: 0, w: 7, h: 8 }, nudgePx: { x: 0, y: 0 } }];
  const m = mockApi({ calibrations: { [calibrationKey([benq.id, samsung.id])]: saved } });
  const app = createApp(m.api);
  await app.start();
  expect(m.count("defaultPlacement")).toBe(0);
  expect(app.state.placed[0]?.mm.x).toBe(5);
  expect(app.state.placed[0]?.pxW).toBe(3840); // pixel size follows the monitor
});

test("changes are saved once, 500 ms after the last one, and layout requests coalesce", async () => {
  const m = mockApi({ prefs: { ...mockApi().stored().prefs, reopenLast: false } });
  const app = createApp(m.api);
  await app.start();
  await app.open("C:\\p\\b.jpg");
  for (let i = 1; i <= 5; i++) app.setFraming({ offsetMm: { x: i, y: 0 } });
  await vi.advanceTimersByTimeAsync(20);
  expect(m.count("computeLayout")).toBe(1);
  expect(m.count("saveSettings")).toBe(0);
  await vi.advanceTimersByTimeAsync(500);
  expect(m.count("saveSettings")).toBe(1);
  expect(m.stored().recent[0]?.framing.offsetMm.x).toBe(5);
  expect(m.stored().last?.image).toBe("C:\\p\\b.jpg");
});

test("closing the window saves a waiting change at once, and only once", async () => {
  const m = mockApi();
  const app = createApp(m.api);
  await app.start();
  app.setPrefs({ span: true });
  await app.flush();
  expect(m.count("saveSettings")).toBe(1);
  expect(m.stored().prefs.span).toBe(true);
  await vi.advanceTimersByTimeAsync(1000);
  expect(m.count("saveSettings")).toBe(1);
  await app.flush(); // nothing waiting: no save
  expect(m.count("saveSettings")).toBe(1);
});

test("reopening a recent image restores its framing", async () => {
  const m = mockApi({ recent: [{ image: "C:\\p\\c.jpg", framing: f(-30) }] });
  const app = createApp(m.api);
  await app.start();
  await app.open("C:\\p\\c.jpg");
  expect(app.state.framing.offsetMm.x).toBe(-30);
});

test("results are said in words; JXR falls back to PNG for an SDR image", async () => {
  const m = mockApi();
  const app = createApp(m.api);
  await app.start();
  await app.open("C:\\p\\d.jpg");
  app.setPrefs({ format: { kind: "jxr" } });
  await app.run("apply");
  expect(app.state.result).toEqual({ kind: "ok", text: "Wallpaper set on 2 monitors", folder: "C:\\out" });
  const job = m.calls.find((c) => c[0] === "apply")?.[1][0] as { prefs: { format: { kind: string } } };
  expect(job.prefs.format.kind).toBe("png");
  await app.run("export");
  expect(app.state.result?.text).toBe("Exported 2 files");
  m.api.exportTiles = () => Promise.reject("The folder is read-only.");
  await app.run("export");
  expect(app.state.result).toEqual({ kind: "error", text: "The folder is read-only." });
});

test("HDR images export as JPEG XR while hdrOutput is on, in the chosen format when it is off", async () => {
  const m = mockApi();
  m.api.openImage = (path) => Promise.resolve({ path, previewPath: path, width: 6000, height: 2000, format: "jpeg", hdr: true });
  const app = createApp(m.api);
  await app.start();
  await app.open("C:\\p\\uhdr.jpg");
  expect(app.job()?.prefs.format.kind).toBe("jxr");
  app.setPrefs({ hdrOutput: false });
  expect(app.job()?.prefs.format.kind).toBe("png");
});

test("pixel sizes follow the monitors, and the mm size turns with a rotated monitor", async () => {
  const p = { id: "a", pxW: 3840, pxH: 2160, mm: { x: 1, y: 2, w: 632, h: 360 }, nudgePx: { x: 0, y: 0 } };
  expect(withPx(p, 2160, 3840).mm).toEqual({ x: 1, y: 2, w: 360, h: 632 });
  expect(withPx(p, 2560, 1440)).toEqual({ ...p, pxW: 2560, pxH: 1440 });
  const m = mockApi();
  const app = createApp(m.api);
  await app.start();
  const listed = m.api.listMonitors;
  m.api.listMonitors = () => listed().then((ms) => ms.map((x) => (x.id === benq.id ? { ...x, px: { ...x.px, w: 2560, h: 1440 } } : x)));
  await app.refreshMonitors(); // same monitors, new resolution
  expect(app.state.placed.find((x) => x.id === benq.id)).toMatchObject({ pxW: 2560, pxH: 1440 });
});
