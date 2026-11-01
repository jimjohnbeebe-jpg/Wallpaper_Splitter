// The light table: every placed monitor as a pane at true mm scale, bezel gaps as real gaps, the photo
// dimmed outside the panes and shown in each pane from that monitor's tile (so nudges show).
// Pan: drag or arrow keys (1 mm, Shift 10 mm). Zoom: wheel or +/-. Tab/Shift+Tab select panes.
import { arrowDelta, bounds, cropInPane, fitView, imagePlacement, round1, stepZoom, toScreen, type View } from "./geom.ts";
import { drawTicks, glyph, h, place, ticks } from "./dom.ts";
import { fileUrl } from "./ipc.ts";
import type { App } from "./state.ts";

type PaneEls = { pane: HTMLElement; clip: HTMLElement; img: HTMLImageElement; chip: HTMLElement; badge: HTMLElement; ticks: SVGSVGElement };

export function lightTable(app: App) {
  const st = app.state;
  const hint = h("span", { id: "table-hint", class: "sr" }, "Drag or use the arrow keys to move the image, plus and minus to zoom, Tab to select a monitor.");
  const backdrop = h("img", { class: "photo backdrop", alt: "", draggable: "false" });
  const mask = h("div", { class: "mask" });
  const layer = h("div", { class: "panes" });
  const el = h("section", { class: "table lt", tabindex: 0, "aria-label": "Light table", "aria-describedby": "table-hint" }, hint, backdrop, mask, layer);
  let panes = new Map<string, PaneEls>();
  let paneKey = "";
  let view: View = { s: 1, ox: 0, oy: 0 };

  function build(): void {
    layer.replaceChildren();
    panes = new Map();
    st.placed.forEach((p, i) => {
      const img = h("img", { class: "photo", alt: "", draggable: "false" });
      const badge = h("span", { class: "badge warn pane-badge" }, glyph("warn"), h("span"));
      const clip = h("div", { class: "clip" }, img);
      const pane = h("div", { class: "pane", "data-i": i }, clip, badge);
      const t = ticks();
      pane.append(t);
      const chip = h("span", { class: "chip" }, st.monitors.find((m) => m.id === p.id)?.name ?? `Monitor ${i + 1}`);
      layer.append(pane, chip);
      panes.set(p.id, { pane, clip, img, chip, badge, ticks: t });
    });
  }

  function sync(): void {
    const key = st.placed.map((p) => p.id).join("|") + st.monitors.map((m) => m.name).join("|");
    if (key !== paneKey) { paneKey = key; build(); }
    view = fitView(bounds(st.placed.map((p) => p.mm)), el.clientWidth, el.clientHeight, 24);
    const img = st.image;
    layer.hidden = !img;
    const src = img ? fileUrl(img.previewPath) : "";
    const layout = st.layout;
    const fill = st.settings.prefs.fill;
    st.placed.forEach((p, i) => {
      const e = panes.get(p.id);
      if (!e) return;
      const r = toScreen(view, p.mm);
      place(e.pane, r.x, r.y, r.w, r.h);
      place(e.chip, r.x, r.y + r.h + 8);
      e.clip.style.backgroundColor = fill;
      const sel = i === st.selected;
      e.pane.classList.toggle("sel", sel);
      if (sel) drawTicks(e.ticks, r.w, r.h);
      const tile = layout?.tiles.find((t) => t.monitorId === p.id);
      e.img.hidden = !img || !tile;
      if (img && tile) {
        if (e.img.getAttribute("src") !== src) e.img.src = src;
        const c = cropInPane(tile.crop, r.w, r.h, img.width, img.height);
        place(e.img, c.x, c.y, c.w, c.h);
      }
      const up = tile && tile.upscale > 1.05 ? tile.upscale : 0;
      e.badge.hidden = !up;
      const label = e.badge.lastElementChild;
      if (label) label.textContent = up ? `Enlarged ${up.toFixed(1)}×` : "";
    });
    const at = img && layout ? imagePlacement(layout, st.placed, img.width, img.height) : null;
    backdrop.hidden = mask.hidden = !at;
    if (img && at) {
      if (backdrop.getAttribute("src") !== src) backdrop.src = src;
      const r = toScreen(view, at);
      place(backdrop, r.x, r.y, r.w, r.h);
    }
  }

  // ---- pan by dragging; a click without movement selects the pane under the pointer ----
  let drag: { x: number; y: number; ox: number; oy: number; moved: boolean; id: number } | null = null;
  el.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || !st.image) return;
    el.focus();
    el.setPointerCapture(e.pointerId);
    drag = { x: e.clientX, y: e.clientY, ox: st.framing.offsetMm.x, oy: st.framing.offsetMm.y, moved: false, id: e.pointerId };
  });
  el.addEventListener("pointermove", (e) => {
    if (!drag || e.pointerId !== drag.id) return;
    const dx = e.clientX - drag.x;
    const dy = e.clientY - drag.y;
    if (!drag.moved && Math.hypot(dx, dy) < 3) return;
    drag.moved = true;
    el.classList.add("dragging");
    app.setFraming({ offsetMm: { x: round1(drag.ox + dx / view.s), y: round1(drag.oy + dy / view.s) } });
  });
  const endDrag = (e: PointerEvent): void => {
    if (!drag || e.pointerId !== drag.id) return;
    if (!drag.moved) {
      const i = Number((document.elementFromPoint(e.clientX, e.clientY)?.closest(".pane") as HTMLElement | null)?.dataset["i"] ?? -1);
      app.setSelected(i);
    }
    drag = null;
    el.classList.remove("dragging");
  };
  el.addEventListener("pointerup", endDrag);
  el.addEventListener("pointercancel", endDrag);

  el.addEventListener("wheel", (e) => {
    if (!st.image) return;
    e.preventDefault();
    app.setFraming({ zoom: stepZoom(st.framing.zoom, e.deltaY < 0 ? 1 : -1) });
  }, { passive: false });

  el.addEventListener("keydown", (e) => {
    if (e.target !== el || e.ctrlKey || e.altKey || e.metaKey || !st.image) return;
    const n = st.placed.length;
    if (e.key === "Tab") {
      // Cycle the panes, then let Tab leave the table (no keyboard trap).
      const next = st.selected + (e.shiftKey ? -1 : 1);
      if (e.shiftKey ? st.selected < 0 : next >= n) { app.setSelected(-1); return; }
      e.preventDefault();
      app.setSelected(next);
      return;
    }
    const d = arrowDelta(e.key, e.shiftKey);
    if (d) {
      e.preventDefault();
      const o = st.framing.offsetMm;
      app.setFraming({ offsetMm: { x: round1(o.x + d.x), y: round1(o.y + d.y) } });
    } else if (e.key === "+" || e.key === "=" || e.key === "-" || e.key === "_") {
      e.preventDefault();
      app.setFraming({ zoom: stepZoom(st.framing.zoom, e.key === "+" || e.key === "=" ? 1 : -1) });
    }
  });

  const ro = new ResizeObserver(() => sync());
  ro.observe(el);
  return { el, sync, dispose: () => ro.disconnect() };
}
