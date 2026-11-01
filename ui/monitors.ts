// The Monitors page: the cutting mat, and a side panel with each monitor's physical size (mm or
// diagonal), desk position and nudge, plus the arrangement actions (align, bezel, identify, reset).
import { h, numField, segmented, syncAll, type Field } from "./dom.ts";
import { diagonalToMm, mmToDiagonal, round1 } from "./geom.ts";
import type { Align, MonitorInfo, PlacedMonitor } from "./ipc.ts";
import { cuttingMat } from "./mat.ts";
import type { Page } from "./split.ts";
import type { App } from "./state.ts";

/** Whether the size still matches what the monitor reports (within half a millimetre). */
function sizeCaption(m: MonitorInfo | undefined, p: PlacedMonitor): string {
  const e = m?.edidMm;
  if (!e) return "Size estimated, set it from the monitor's spec sheet";
  const same = (a: number, b: number): boolean => Math.abs(a - b) < 0.5;
  return (same(e.w, p.mm.w) && same(e.h, p.mm.h)) || (same(e.w, p.mm.h) && same(e.h, p.mm.w)) ? "Size reported by the monitor" : "Size set by you";
}

function monitorGroup(app: App, id: string, i: number): { el: HTMLElement; fields: Field[] } {
  const st = app.state;
  const cur = (): PlacedMonitor | undefined => st.placed.find((p) => p.id === id);
  const info = (): MonitorInfo | undefined => st.monitors.find((m) => m.id === id);
  const upd = (f: (p: PlacedMonitor) => PlacedMonitor): void => app.updateMonitor(id, f);
  const mm = (k: "x" | "y" | "w" | "h") => (v: number | null): void => upd((p) => ({ ...p, mm: { ...p.mm, [k]: round1(v ?? p.mm[k]) } }));
  const nudge = (k: "x" | "y") => (v: number | null): void => upd((p) => ({ ...p, nudgePx: { ...p.nudgePx, [k]: Math.round(v ?? 0) } }));
  const get = (f: (p: PlacedMonitor) => number) => (): number => { const p = cur(); return p ? f(p) : 0; };

  const w = numField({ label: "Width", unit: "mm", min: 50, max: 5000, get: get((p) => p.mm.w), set: mm("w") });
  const hh = numField({ label: "Height", unit: "mm", min: 50, max: 5000, get: get((p) => p.mm.h), set: mm("h") });
  const diag = numField({
    label: "Diagonal", unit: "in", step: 0.1, min: 5, max: 200, get: get((p) => mmToDiagonal(p.mm.w, p.mm.h)),
    set: (v) => upd((p) => { const s = diagonalToMm(v ?? 27, p.pxW, p.pxH); return { ...p, mm: { ...p.mm, w: round1(s.w), h: round1(s.h) } }; }),
  });
  const x = numField({ label: "Position X", unit: "mm", get: get((p) => p.mm.x), set: mm("x") });
  const y = numField({ label: "Position Y", unit: "mm", get: get((p) => p.mm.y), set: mm("y") });
  const nx = numField({ label: "Nudge X", unit: "px", decimals: 0, get: get((p) => p.nudgePx.x), set: nudge("x") });
  const ny = numField({ label: "Nudge Y", unit: "px", decimals: 0, get: get((p) => p.nudgePx.y), set: nudge("y") });
  const caption = h("p", { class: "caption" });
  const head = h("h3", { class: "mon-name" }, info()?.name ?? `Monitor ${i + 1}`);
  const res = h("p", { class: "muted num" });
  const status: Field = {
    el: caption,
    sync: () => {
      const p = cur();
      if (!p) return;
      caption.textContent = sizeCaption(info(), p);
      res.textContent = `${p.pxW} × ${p.pxH} px`;
      head.classList.toggle("sel", st.selected === i);
    },
  };
  const fields = [w, hh, diag, x, y, nx, ny, status];
  const el = h("div", { class: "group mon" }, head, res, w.el, hh.el, diag.el, caption, x.el, y.el, nx.el, ny.el);
  return { el, fields };
}

export function monitorsPage(app: App): Page {
  const st = app.state;
  const mat = cuttingMat(app);
  const panel = h("aside", { class: "inspector", "aria-label": "Monitor details" });
  let align: Align = "center";
  let bezel = 0;
  let fields: Field[] = [];
  let key = "";

  function arrangeGroup(): HTMLElement {
    const seg = segmented<Align>("Align", [{ value: "top", label: "Top" }, { value: "center", label: "Center" }, { value: "bottom", label: "Bottom" }],
      () => align, (v) => { align = v; seg.sync(); });
    const bz = numField({ label: "Bezel gap", unit: "mm", min: 0, max: 200, get: () => bezel, set: (v) => { bezel = v ?? 0; } });
    fields.push(seg, bz);
    return h("div", { class: "group" },
      h("h2", {}, "Arrangement"),
      seg.el, bz.el,
      h("p", { class: "caption" }, "Places the monitors in Windows' order, edges touching plus the bezel gap."),
      h("div", { class: "btn-row" },
        h("button", { type: "button", class: "btn compact", onclick: () => void app.arrange(align, bezel, true) }, "Arrange"),
        h("button", { type: "button", class: "btn compact", onclick: () => app.identify() }, "Identify"),
        h("button", { type: "button", class: "btn compact", title: "Sizes from the monitors, centred, no gaps or nudges", onclick: () => void app.arrange("center", 0, false) }, "Reset to detected")));
  }

  function sync(): void {
    const k = st.placed.map((p) => p.id).join("|") + st.monitors.map((m) => m.name).join("|");
    if (k !== key) {
      key = k;
      fields = [];
      const groups = st.placed.map((p, i) => monitorGroup(app, p.id, i));
      groups.forEach((g) => fields.push(...g.fields));
      panel.replaceChildren(arrangeGroup(), ...groups.map((g) => g.el));
      if (st.placed.length === 0) panel.append(h("p", { class: "muted" }, "No monitors found."));
    }
    mat.sync();
    syncAll(fields);
  }

  return { els: [mat.el, panel], sync, dispose: mat.dispose };
}
