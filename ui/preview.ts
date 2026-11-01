// The secondary windows (preview.html): one full-screen preview per monitor, or the Identify label.
// Preview keys: Esc closes, G toggles the grid, Tab selects the next monitor, arrows nudge it (Shift 10 px).
import { h, svg } from "./dom.ts";
import { arrowDelta } from "./geom.ts";
import { api, fileUrl, listen, windowLabel, type PreviewState } from "./ipc.ts";

type Preview = Extract<PreviewState, { kind: "preview" }>;

/** Grid line positions (CSS px from the window's left or top) every `stepMm` across a monitor that spans
 *  `startMm`..`startMm + sizeMm` on the desk, so lines continue from one monitor to the next. */
export function gridLines(startMm: number, sizeMm: number, pxPerMm: number, stepMm: number, shiftPx = 0): number[] {
  const out: number[] = [];
  for (let mm = Math.ceil((startMm - shiftPx / pxPerMm) / stepMm) * stepMm; ; mm += stepMm) {
    const x = (mm - startMm) * pxPerMm + shiftPx;
    if (x > sizeMm * pxPerMm) break;
    if (x >= 0) out.push(x);
  }
  return out;
}

/** How far (CSS px) a nudge moves the picture on screen. geometry.rs adds the nudge to the crop origin,
 *  so a positive nudge moves the picture left/up; the grid moves with the picture. */
export const nudgeShift = (n: { x: number; y: number }, dpr: number): [number, number] => [-n.x / dpr, -n.y / dpr];

function drawGrid(s: Preview, w: number, hgt: number, dpr: number): SVGSVGElement {
  const m = s.monitor;
  const k = w / m.mm.w; // CSS px per mm
  const [nx, ny] = nudgeShift(m.nudgePx, dpr);
  const g = svg("svg", { class: "grid", width: w, height: hgt, "aria-hidden": "true" });
  for (const step of [10, 100]) {
    const cls = step === 100 ? "g100" : "g10";
    for (const x of gridLines(m.mm.x, m.mm.w, k, step, nx)) g.append(svg("line", { class: cls, x1: x, y1: 0, x2: x, y2: hgt }));
    for (const y of gridLines(m.mm.y, m.mm.h, k, step, ny)) g.append(svg("line", { class: cls, x1: 0, y1: y, x2: w, y2: y }));
  }
  return g;
}

const names = new Map<string, string>(); // monitor id -> friendly name (the preview state carries only the id)

function renderPreview(root: HTMLElement, s: Preview): void {
  const dpr = window.devicePixelRatio || 1;
  const w = s.tile.outW / dpr;
  const hgt = s.tile.outH / dpr;
  const c = s.tile.crop;
  const [sx, sy] = [w / c.w, hgt / c.h];
  root.style.backgroundColor = s.fill;
  let img = root.querySelector<HTMLImageElement>("img.photo");
  if (!img) {
    img = h("img", { class: "photo", alt: "" });
    img.addEventListener("load", () => document.body.classList.add("shown"), { once: true });
    img.addEventListener("error", () => document.body.classList.add("shown"), { once: true });
    root.replaceChildren(img);
  }
  const src = fileUrl(s.previewPath);
  if (img.getAttribute("src") !== src) img.src = src;
  img.style.left = `${-c.x * sx}px`;
  img.style.top = `${-c.y * sy}px`;
  img.style.width = `${s.imageW * sx}px`;
  img.style.height = `${s.imageH * sy}px`;
  root.querySelector(".grid")?.remove();
  root.querySelector(".label-chip")?.remove();
  if (s.grid) root.append(drawGrid(s, w, hgt, dpr));
  if (s.selected) {
    const n = s.monitor.nudgePx;
    root.append(h("div", { class: "label-chip" },
      h("span", { class: "name" }, names.get(s.monitor.id) ?? `${s.monitor.pxW} × ${s.monitor.pxH} monitor`),
      h("span", { class: "num" }, `Nudge ${n.x}, ${n.y} px`),
      h("span", { class: "caption" }, "Arrow keys nudge, Tab next monitor, G grid, Esc close")));
  }
}

function renderIdentify(root: HTMLElement, s: Extract<PreviewState, { kind: "identify" }>): void {
  document.body.classList.add("identify", "shown");
  root.replaceChildren(h("div", { class: "identify-card" },
    h("span", { class: "big num" }, String(s.number)),
    h("span", { class: "name" }, s.name),
    h("span", { class: "muted num" }, `${s.pxW} × ${s.pxH}`)));
}

async function refresh(root: HTMLElement, label: string): Promise<void> {
  try {
    const s = await api.previewState(label);
    if (s.kind === "preview") renderPreview(root, s);
    else renderIdentify(root, s);
  } catch (e) {
    document.body.classList.add("shown");
    root.replaceChildren(h("p", { class: "danger preview-error" }, String(e)));
  }
}

function init(): void {
  const root = document.getElementById("preview");
  if (!root) return;
  const label = windowLabel();
  void refresh(root, label);
  if (label.startsWith("preview")) {
    api.listMonitors().then((ms) => { ms.forEach((m) => names.set(m.id, m.name)); void refresh(root, label); }, () => {});
  }
  void listen<unknown>("preview-changed", () => void refresh(root, label));
  window.addEventListener("resize", () => void refresh(root, label)); // a scale change resizes the window
  if (label.startsWith("identify")) return;
  document.addEventListener("keydown", (e) => {
    const d = arrowDelta(e.key, e.shiftKey);
    let p: Promise<void> | null = null;
    if (e.key === "Escape") p = api.previewClose();
    else if (e.key === "g" || e.key === "G") p = api.previewGrid();
    else if (e.key === "Tab") p = api.previewNudge(0, 0, true);
    else if (d) p = api.previewNudge(d.x, d.y, false);
    if (!p) return;
    e.preventDefault();
    void p.catch(() => {});
  });
}

if (typeof document !== "undefined") init();
