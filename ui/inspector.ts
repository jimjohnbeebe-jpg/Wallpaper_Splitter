// The Split page's inspector: Source, Framing, Monitors, Output, then the actions. Rebuilt only when its
// structure changes (image, monitors, output format); otherwise fields sync in place and keep focus.
import { glyph, h, keys, numField, segmented, syncAll, type Field } from "./dom.ts";
import { clampZoom, round1 } from "./geom.ts";
import type { ExportFormat, Fit, PlacedMonitor } from "./ipc.ts";
import { basename, defaultFraming, type App } from "./state.ts";

const FORMAT_NAMES: Record<string, string> = { jpeg: "JPEG", ultrahdr: "Ultra HDR JPEG", png: "PNG", tiff: "TIFF", webp: "WebP", avif: "AVIF", jxl: "JPEG XL" };

export function inspector(app: App) {
  const st = app.state;
  const el = h("aside", { class: "inspector", "aria-label": "Inspector" });
  let fields: Field[] = [];
  let structure = "";
  let buttons: { apply: HTMLButtonElement; exp: HTMLButtonElement; prev: HTMLButtonElement } | null = null;

  const monitorName = (p: PlacedMonitor, i: number): string => st.monitors.find((m) => m.id === p.id)?.name ?? `Monitor ${i + 1}`;

  function source(): HTMLElement {
    const img = st.image;
    // Starting over with a new image is the Source group's one job, so its button is full size and in the accent:
    // filled while nothing is open (Set wallpaper is disabled then), outlined once Set wallpaper takes the fill.
    const open = h("button", { type: "button", class: `btn action ${img ? "open" : "primary"}`, onclick: () => void app.pick() },
      h("span", { class: "label" }, img ? "Open another image…" : "Open image…"), keys("Ctrl", "O"));
    if (!img) return h("div", { class: "group" }, h("h2", {}, "Source"), h("p", { class: "muted" }, "No image open."), open);
    return h("div", { class: "group" },
      h("h2", {}, "Source"),
      h("p", { class: "name", title: img.path }, basename(img.path)),
      h("div", { class: "meta" },
        h("span", { class: "num muted" }, `${img.width} × ${img.height} px`),
        h("span", { class: "badge" }, FORMAT_NAMES[img.format] ?? img.format),
        img.hdr ? h("span", { class: "badge accent" }, "HDR") : null),
      open);
  }

  function framing(): HTMLElement {
    const f = (): typeof st.framing => st.framing;
    const fit = segmented<Fit>("Fit", [{ value: "cover", label: "Cover" }, { value: "contain", label: "Contain" }], () => f().fit, (v) => app.setFraming({ fit: v }));
    const zoom = numField({ label: "Zoom", unit: "%", step: 5, min: 25, max: 800, decimals: 0, get: () => f().zoom * 100, set: (v) => app.setFraming({ zoom: clampZoom((v ?? 100) / 100) }) });
    const ox = numField({ label: "Offset X", unit: "mm", step: 1, get: () => f().offsetMm.x, set: (v) => app.setFraming({ offsetMm: { ...f().offsetMm, x: round1(v ?? 0) } }) });
    const oy = numField({ label: "Offset Y", unit: "mm", step: 1, get: () => f().offsetMm.y, set: (v) => app.setFraming({ offsetMm: { ...f().offsetMm, y: round1(v ?? 0) } }) });
    fields.push(fit, zoom, ox, oy);
    const reset = h("button", { type: "button", class: "btn compact", onclick: () => app.setFraming(defaultFraming()) }, "Reset");
    return h("div", { class: "group" }, h("div", { class: "group-head" }, h("h2", {}, "Framing"), reset), fit.el, zoom.el, ox.el, oy.el);
  }

  function monitors(): HTMLElement {
    const g = h("div", { class: "group" }, h("h2", {}, "Monitors"));
    st.placed.forEach((p, i) => {
      const upd = (axis: "x" | "y") => (v: number | null): void =>
        app.updateMonitor(p.id, (m) => ({ ...m, nudgePx: { ...m.nudgePx, [axis]: Math.round(v ?? 0) } }));
      const cur = (): PlacedMonitor | undefined => st.placed.find((m) => m.id === p.id);
      const nx = numField({ label: "Nudge X", unit: "px", decimals: 0, get: () => cur()?.nudgePx.x ?? 0, set: upd("x") });
      const ny = numField({ label: "Nudge Y", unit: "px", decimals: 0, get: () => cur()?.nudgePx.y ?? 0, set: upd("y") });
      const warnText = h("span");
      const warn = h("p", { class: "warn line" }, glyph("warn"), warnText);
      const head = h("h3", { class: "mon-name" }, monitorName(p, i));
      fields.push(nx, ny, {
        el: warn,
        sync: () => {
          const t = st.layout?.tiles.find((x) => x.monitorId === p.id);
          warn.hidden = !t || t.upscale <= 1.05;
          warnText.textContent = t ? `Enlarged ${t.upscale.toFixed(1)}×, the image is smaller than this monitor` : "";
          head.classList.toggle("sel", st.selected === i);
        },
      });
      g.append(h("div", { class: "mon" }, head, nx.el, ny.el, warn));
    });
    return g;
  }

  function output(): HTMLElement {
    const fmt = (): ExportFormat => st.settings.prefs.format;
    const hdr = (): boolean => st.image?.hdr ?? false;
    const shown = (): ExportFormat["kind"] => (fmt().kind === "jxr" && !hdr() ? "png" : fmt().kind);
    const seg = segmented<ExportFormat["kind"]>("Format", [
      { value: "png", label: "PNG" },
      { value: "jpg", label: "JPG" },
      { value: "jxr", label: "HDR JXR", disabled: () => (hdr() ? null : "HDR JPEG XR needs an HDR image") },
    ], shown, (k) => app.setPrefs({ format: k === "jpg" ? { kind: "jpg", quality: 92 } : { kind: k } }));
    fields.push(seg);
    const g = h("div", { class: "group" }, h("h2", {}, "Output"), seg.el);
    const f = fmt();
    if (f.kind === "jpg") {
      const q = numField({ label: "JPG quality", decimals: 0, min: 1, max: 100, get: () => { const c = fmt(); return c.kind === "jpg" ? c.quality : 92; }, set: (v) => app.setPrefs({ format: { kind: "jpg", quality: Math.round(v ?? 92) } }) });
      fields.push(q);
      g.append(q.el);
    }
    return g;
  }

  function actions(): HTMLElement {
    const btn = (cls: string, label: string, hint: HTMLElement, run: () => void): HTMLButtonElement =>
      h("button", { type: "button", class: `btn ${cls} action`, onclick: run }, h("span", { class: "label" }, label), hint);
    buttons = {
      apply: btn("primary", "Set wallpaper", keys("Ctrl", "Enter"), () => void app.run("apply")),
      exp: btn("secondary", "Export only", keys("Ctrl", "E"), () => void app.run("export")),
      prev: btn("secondary", "Preview full screen", keys("F11"), () => void app.run("preview")),
    };
    return h("div", { class: "actions" }, buttons.apply, buttons.exp, buttons.prev);
  }

  function syncButtons(): void {
    if (!buttons) return;
    const labels = { apply: ["Set wallpaper", "Setting wallpaper…"], exp: ["Export only", "Exporting…"], prev: ["Preview full screen", "Opening preview…"] } as const;
    const busyKey = { apply: "apply", exp: "export", prev: "preview" } as const;
    for (const k of ["apply", "exp", "prev"] as const) {
      const b = buttons[k];
      const busy = st.busy === busyKey[k];
      b.disabled = !st.image || st.busy !== null;
      b.title = st.image ? "" : "Open an image first";
      b.setAttribute("aria-busy", String(busy));
      const l = b.querySelector(".label");
      if (l) l.textContent = labels[k][busy ? 1 : 0];
    }
  }

  function sync(): void {
    const s = [st.image?.path, st.image?.hdr, st.placed.map((p) => p.id).join("|"), st.monitors.map((m) => m.name).join("|"), st.settings.prefs.format.kind].join("\n");
    if (s !== structure) {
      structure = s;
      fields = [];
      el.replaceChildren(source(), ...(st.image ? [framing()] : []), monitors(), output(), actions());
    }
    syncAll(fields);
    syncButtons();
  }
  return { el, sync };
}
