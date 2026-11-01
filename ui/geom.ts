// Screen mapping for the light table, the cutting mat and the preview windows. This is drawing maths
// only: crops always come from the backend's compute_layout (CLAUDE.md), and this file never derives one.
import type { Layout, PlacedMonitor, Rect, Tile } from "./ipc.ts";

/** Desk millimetres to CSS pixels: screen = o + mm * s. */
export type View = { s: number; ox: number; oy: number };

export function bounds(rects: Rect[]): Rect {
  if (rects.length === 0) return { x: 0, y: 0, w: 1, h: 1 };
  const x = Math.min(...rects.map((r) => r.x));
  const y = Math.min(...rects.map((r) => r.y));
  const w = Math.max(...rects.map((r) => r.x + r.w)) - x;
  const h = Math.max(...rects.map((r) => r.y + r.h)) - y;
  return { x, y, w: w || 1, h: h || 1 };
}

/** Fits `b` (mm) centred into a `w` x `h` px box with `margin` px on every side. */
export function fitView(b: Rect, w: number, h: number, margin = 24): View {
  const s = Math.max(0.01, Math.min((w - 2 * margin) / b.w, (h - 2 * margin) / b.h));
  return { s, ox: (w - b.w * s) / 2 - b.x * s, oy: (h - b.h * s) / 2 - b.y * s };
}

export const toScreen = (v: View, r: Rect): Rect => ({ x: v.ox + r.x * v.s, y: v.oy + r.y * v.s, w: r.w * v.s, h: r.h * v.s });
export const toMm = (v: View, x: number, y: number): { x: number; y: number } => ({ x: (x - v.ox) / v.s, y: (y - v.oy) / v.s });

/** Where the whole image sits on the desk (mm), read back from one tile: source px per mm = crop.w / mm.w.
 *  Uses the monitor with the smallest nudge, so a nudged pane does not drag the dimmed backdrop along. */
export function imagePlacement(layout: Layout, placed: PlacedMonitor[], imgW: number, imgH: number): Rect | null {
  let best: { t: Tile; m: PlacedMonitor } | null = null;
  for (const t of layout.tiles) {
    const m = placed.find((p) => p.id === t.monitorId);
    if (!m || t.crop.w <= 0 || m.mm.w <= 0) continue;
    if (!best || Math.hypot(m.nudgePx.x, m.nudgePx.y) < Math.hypot(best.m.nudgePx.x, best.m.nudgePx.y)) best = { t, m };
  }
  if (!best) return null;
  const { t, m } = best;
  const k = t.crop.w / m.mm.w;
  const ky = t.crop.h > 0 && m.mm.h > 0 ? t.crop.h / m.mm.h : k;
  return { x: m.mm.x - t.crop.x / k, y: m.mm.y - t.crop.y / ky, w: imgW / k, h: imgH / ky };
}

/** The <img> rectangle, relative to a pane of `paneW` x `paneH` px, that makes the tile's crop fill the pane. */
export function cropInPane(crop: Rect, paneW: number, paneH: number, imgW: number, imgH: number): Rect {
  const sx = paneW / crop.w;
  const sy = paneH / crop.h;
  return { x: -crop.x * sx, y: -crop.y * sy, w: imgW * sx, h: imgH * sy };
}

/** Physical size from a diagonal in inches, keeping the pixel aspect: w = d * 25.4 * cos(atan(pxH / pxW)). */
export function diagonalToMm(inches: number, pxW: number, pxH: number): { w: number; h: number } {
  const a = Math.atan(pxH / pxW);
  return { w: inches * 25.4 * Math.cos(a), h: inches * 25.4 * Math.sin(a) };
}
export const mmToDiagonal = (w: number, h: number): number => Math.hypot(w, h) / 25.4;

/** Snaps a dragged pane's position to the other panes' edges (left/right, top/bottom) within `tol` mm. */
export function snapRect(r: Rect, others: Rect[], tol = 3): { x: number; y: number } {
  const axis = (pos: number, size: number, edges: number[]): number => {
    let best = pos;
    let dist = tol + 1e-9;
    for (const e of edges) {
      for (const cand of [e, e - size]) {
        const d = Math.abs(cand - pos);
        if (d < dist) [best, dist] = [cand, d];
      }
    }
    return best;
  };
  return {
    x: axis(r.x, r.w, others.flatMap((o) => [o.x, o.x + o.w])),
    y: axis(r.y, r.h, others.flatMap((o) => [o.y, o.y + o.h])),
  };
}

/** Arrow-key step: 1 unit, 10 with Shift; null for any other key. */
export function arrowDelta(key: string, shift: boolean): { x: number; y: number } | null {
  const n = shift ? 10 : 1;
  if (key === "ArrowLeft") return { x: -n, y: 0 };
  if (key === "ArrowRight") return { x: n, y: 0 };
  if (key === "ArrowUp") return { x: 0, y: -n };
  if (key === "ArrowDown") return { x: 0, y: n };
  return null;
}

export const ZOOM_MIN = 0.25;
export const ZOOM_MAX = 8;
export const clampZoom = (z: number): number => Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, z));
/** One zoom step of 5 % in (dir 1) or out (dir -1), clamped to 0.25..8. */
export const stepZoom = (z: number, dir: 1 | -1): number => clampZoom(Math.round(z * 1.05 ** dir * 1000) / 1000);

export const round1 = (n: number): number => Math.round(n * 10) / 10;
