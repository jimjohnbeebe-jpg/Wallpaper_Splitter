import { expect, test } from "vitest";
import { arrowDelta, bounds, cropInPane, diagonalToMm, fitView, imagePlacement, mmToDiagonal, snapRect, stepZoom, toMm, toScreen } from "./geom.ts";
import type { PlacedMonitor } from "./ipc.ts";

const pm = (id: string, x: number, w: number, h: number, nudge = 0): PlacedMonitor =>
  ({ id, pxW: 3840, pxH: 2160, mm: { x, y: 0, w, h }, nudgePx: { x: nudge, y: 0 } });

test("the light table fits the monitors with a 24 px margin and round-trips mm", () => {
  const b = bounds([{ x: 0, y: 0, w: 698, h: 393 }, { x: 708, y: 18, w: 632, h: 356 }]);
  expect(b).toEqual({ x: 0, y: 0, w: 1340, h: 393 });
  const v = fitView(b, 1388, 600);
  expect(v.s).toBeCloseTo(1); // (1388 - 48) / 1340
  expect(v.ox).toBeCloseTo(24);
  expect(v.oy).toBeCloseTo((600 - 393) / 2);
  const r = toScreen(v, { x: 708, y: 18, w: 632, h: 356 });
  expect(r.x).toBeCloseTo(732);
  const back = toMm(v, r.x, r.y);
  expect(back.x).toBeCloseTo(708);
  expect(back.y).toBeCloseTo(18);
});

test("a tall well is limited by height", () => {
  const v = fitView({ x: -100, y: 0, w: 200, h: 100 }, 400, 148);
  expect(v.s).toBeCloseTo(1);
  expect(toScreen(v, { x: -100, y: 0, w: 1, h: 1 }).x).toBeCloseTo(100);
});

test("the image's desk placement comes back from a tile", () => {
  // 10 source px per mm; the monitor at x=700 mm shows source x 2000..8320 px.
  const m = pm("b", 700, 632, 356);
  const layout = { tiles: [{ monitorId: "b", crop: { x: 2000, y: 500, w: 6320, h: 3560 }, outW: 3840, outH: 2160, upscale: 0.6 }] };
  expect(imagePlacement(layout, [m], 9000, 4000)).toEqual({ x: 500, y: -50, w: 900, h: 400 });
  expect(imagePlacement({ tiles: [] }, [m], 1, 1)).toBeNull();
});

test("the least-nudged monitor sets the backdrop", () => {
  const a = pm("a", 0, 600, 300, 5);
  const b = pm("b", 600, 600, 300, 0);
  const layout = { tiles: [
    { monitorId: "a", crop: { x: 3, y: 0, w: 600, h: 300 }, outW: 600, outH: 300, upscale: 1 },
    { monitorId: "b", crop: { x: 600, y: 0, w: 600, h: 300 }, outW: 600, outH: 300, upscale: 1 },
  ] };
  expect(imagePlacement(layout, [a, b], 1200, 300)?.x).toBe(0);
});

test("a crop fills its pane", () => {
  const r = cropInPane({ x: 1000, y: 200, w: 2000, h: 1000 }, 400, 200, 6000, 2000);
  expect(r).toEqual({ x: -200, y: -40, w: 1200, h: 400 });
});

test("diagonal to millimetres keeps the pixel aspect", () => {
  const { w, h } = diagonalToMm(32, 3840, 2160);
  expect(w).toBeCloseTo(708.4, 1);
  expect(h).toBeCloseTo(398.5, 1);
  expect(mmToDiagonal(w, h)).toBeCloseTo(32);
  const p = diagonalToMm(27, 1440, 2560); // portrait
  expect(p.h).toBeGreaterThan(p.w);
});

test("panes snap to other panes' edges within 3 mm", () => {
  const other = { x: 0, y: 0, w: 700, h: 400 };
  expect(snapRect({ x: 702, y: 2.5, w: 600, h: 350 }, [other])).toEqual({ x: 700, y: 0 });
  expect(snapRect({ x: 705, y: 20, w: 600, h: 350 }, [other])).toEqual({ x: 705, y: 20 });
  expect(snapRect({ x: 50, y: 52, w: 600, h: 350 }, [other])).toEqual({ x: 50, y: 50 }); // bottoms align
  expect(snapRect({ x: 10, y: 10, w: 10, h: 10 }, [])).toEqual({ x: 10, y: 10 });
});

test("arrow keys move 1, Shift 10", () => {
  expect(arrowDelta("ArrowLeft", false)).toEqual({ x: -1, y: 0 });
  expect(arrowDelta("ArrowDown", true)).toEqual({ x: 0, y: 10 });
  expect(arrowDelta("a", false)).toBeNull();
});

test("zoom steps 5 % and stays within 0.25..8", () => {
  expect(stepZoom(1, 1)).toBe(1.05);
  expect(stepZoom(1.05, -1)).toBe(1);
  expect(stepZoom(8, 1)).toBe(8);
  expect(stepZoom(0.25, -1)).toBe(0.25);
});
