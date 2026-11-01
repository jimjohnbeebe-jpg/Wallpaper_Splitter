// The cutting mat on the Monitors page: a millimetre grid (10 mm and 100 mm lines) with every monitor as
// a pane you drag to its place on the desk. Panes snap to each other's edges within 3 mm (hold Alt to
// stop snapping). Keyboard: Tab to a pane, arrow keys move it 1 mm, Shift 10 mm.
import { drawTicks, h, place, svg, ticks } from "./dom.ts";
import { arrowDelta, bounds, fitView, round1, snapRect, toScreen, type View } from "./geom.ts";
import type { PlacedMonitor } from "./ipc.ts";
import type { App } from "./state.ts";

const PAD_MM = 100; // free mat around the monitors

function grid() {
  const el = svg("svg", { class: "mat-grid", "aria-hidden": "true" });
  const defs = svg("defs");
  const pat = (id: string, cls: string) => {
    const p = svg("pattern", { id, patternUnits: "userSpaceOnUse" });
    const path = svg("path", { class: cls, fill: "none" });
    p.append(path);
    defs.append(p);
    return { p, path };
  };
  const g10 = pat("mat-g10", "g10");
  const g100 = pat("mat-g100", "g100");
  const r10 = svg("rect", { width: "100%", height: "100%", fill: "url(#mat-g10)" });
  const r100 = svg("rect", { width: "100%", height: "100%", fill: "url(#mat-g100)" });
  el.append(defs, r10, r100);
  const draw = (v: View): void => {
    for (const [g, mm] of [[g10, 10], [g100, 100]] as const) {
      const step = mm * v.s;
      for (const [k, val] of Object.entries({ x: v.ox, y: v.oy, width: step, height: step })) g.p.setAttribute(k, String(val));
      g.path.setAttribute("d", `M0 0.5H${step}M0.5 0V${step}`);
    }
    r10.setAttribute("visibility", 10 * v.s >= 4 ? "visible" : "hidden"); // too dense to read below 4 px
  };
  return { el, draw };
}

export function cuttingMat(app: App) {
  const st = app.state;
  const g = grid();
  const layer = h("div", { class: "panes" });
  const el = h("section", { class: "mat", "aria-label": "Monitor arrangement" }, g.el, layer);
  let view: View = { s: 1, ox: 0, oy: 0 };
  let frozen = false; // keep the view still while dragging
  let key = "";
  let panes: { id: string; btn: HTMLButtonElement; pos: HTMLElement; t: SVGSVGElement }[] = [];

  const nameOf = (id: string, i: number): string => st.monitors.find((m) => m.id === id)?.name ?? `Monitor ${i + 1}`;
  const moveTo = (id: string, x: number, y: number): void => app.updateMonitor(id, (p) => ({ ...p, mm: { ...p.mm, x: round1(x), y: round1(y) } }));

  function build(): void {
    layer.replaceChildren();
    panes = st.placed.map((p, i) => {
      const pos = h("span", { class: "muted num" });
      const t = ticks();
      const btn = h("button", { type: "button", class: "mat-pane", "data-i": i },
        h("span", { class: "name" }, nameOf(p.id, i)), h("span", { class: "num muted" }, `${p.pxW} × ${p.pxH}`), pos, t);
      btn.addEventListener("focus", () => { if (st.selected !== i) app.setSelected(i); });
      btn.addEventListener("keydown", (e) => {
        const d = arrowDelta(e.key, e.shiftKey);
        const cur = st.placed.find((x) => x.id === p.id);
        if (!d || !cur) return;
        e.preventDefault();
        moveTo(p.id, cur.mm.x + d.x, cur.mm.y + d.y);
      });
      btn.addEventListener("pointerdown", (e) => startDrag(e, p.id, i, btn));
      layer.append(btn);
      return { id: p.id, btn, pos, t };
    });
  }

  function startDrag(e: PointerEvent, id: string, i: number, btn: HTMLButtonElement): void {
    const start = st.placed.find((x) => x.id === id);
    if (e.button !== 0 || !start) return;
    btn.focus();
    app.setSelected(i);
    btn.setPointerCapture(e.pointerId);
    frozen = true;
    const [sx, sy] = [e.clientX, e.clientY];
    const move = (ev: PointerEvent): void => {
      const r = { ...start.mm, x: start.mm.x + (ev.clientX - sx) / view.s, y: start.mm.y + (ev.clientY - sy) / view.s };
      const others = st.placed.filter((x) => x.id !== id).map((x) => x.mm);
      const at = ev.altKey ? r : snapRect(r, others, 3);
      moveTo(id, at.x, at.y);
    };
    const end = (): void => {
      btn.removeEventListener("pointermove", move);
      btn.removeEventListener("pointerup", end);
      btn.removeEventListener("pointercancel", end);
      frozen = false;
      sync();
    };
    btn.addEventListener("pointermove", move);
    btn.addEventListener("pointerup", end);
    btn.addEventListener("pointercancel", end);
  }

  function sync(): void {
    const k = st.placed.map((p) => p.id).join("|") + st.monitors.map((m) => m.name).join("|");
    if (k !== key) { key = k; build(); }
    if (!frozen) {
      const b = bounds(st.placed.map((p) => p.mm));
      view = fitView({ x: b.x - PAD_MM, y: b.y - PAD_MM, w: b.w + 2 * PAD_MM, h: b.h + 2 * PAD_MM }, el.clientWidth, el.clientHeight, 24);
      g.draw(view);
    }
    panes.forEach(({ id, btn, pos, t }, i) => {
      const p: PlacedMonitor | undefined = st.placed.find((x) => x.id === id);
      if (!p) return;
      const r = toScreen(view, p.mm);
      place(btn, r.x, r.y, r.w, r.h);
      const sel = i === st.selected;
      btn.classList.toggle("sel", sel);
      if (sel) drawTicks(t, r.w, r.h);
      pos.textContent = `${round1(p.mm.x)}, ${round1(p.mm.y)} mm`;
      btn.setAttribute("aria-label", `${nameOf(id, i)}, at ${round1(p.mm.x)} by ${round1(p.mm.y)} millimetres. Arrow keys move it.`);
    });
  }

  const ro = new ResizeObserver(() => sync());
  ro.observe(el);
  return { el, sync, dispose: () => ro.disconnect() };
}
