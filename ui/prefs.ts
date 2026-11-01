// The Preferences page: HDR, Export and Startup, every field of Prefs with a plain label and a one-line
// caption. Changes save on their own (state.ts debounces).
import { h, nextId, numField, segmented, switchField, syncAll, type Field } from "./dom.ts";
import { hasFolderPicker, pickFolder, type ExportFormat, type Prefs } from "./ipc.ts";
import type { Page } from "./split.ts";
import { extFor, renderTemplate, stem, type App } from "./state.ts";

type TextOpts = { label: string; caption: string; get: () => string; set: (v: string) => void; placeholder?: string; type?: string; extra?: HTMLElement };
function textField(o: TextOpts): Field {
  const id = nextId("t");
  const input = h("input", { id, class: "input wide", type: o.type ?? "text", placeholder: o.placeholder, spellcheck: "false" });
  input.addEventListener("change", () => o.set(input.value.trim()));
  const sync = (): void => { if (document.activeElement !== input) input.value = o.get(); };
  const el = h("div", { class: "stack" }, h("label", { for: id }, o.label), h("div", { class: "input-row" }, input, o.extra ?? null), h("span", { class: "caption" }, o.caption));
  sync();
  return { el, sync };
}

export function prefsPage(app: App): Page {
  const st = app.state;
  const p = (): Prefs => st.settings.prefs;
  const set = (patch: Partial<Prefs>): void => app.setPrefs(patch);
  const fields: Field[] = [];
  const add = <T extends Field>(f: T): HTMLElement => { fields.push(f); return f.el; };

  const hdr = h("section", { class: "group" }, h("h2", {}, "HDR"),
    add(switchField("HDR files for HDR images", "Export JPEG XR when the image is HDR, so the desktop shows it in HDR", () => p().hdrOutput, (v) => set({ hdrOutput: v }))),
    add(switchField("Turn on HDR for the full-screen preview", "Switches HDR on while previewing and puts it back afterwards", () => p().autoHdrPreview, (v) => set({ autoHdrPreview: v }))),
    add(switchField("HDR in this window", "Shows HDR images at full brightness on the light table", () => p().hdrPreview, (v) => set({ hdrPreview: v }))),
    add(numField({ label: "SDR white level", unit: "nits", min: 80, max: 480, step: 10, decimals: 0, placeholder: "Windows", title: "Leave empty to use the Windows setting",
      get: () => p().sdrWhiteNits, set: (v) => set({ sdrWhiteNits: v }) })),
    h("p", { class: "caption" }, "How bright white is in HDR files. Leave it empty to use the Windows setting."));

  const fmt = (): ExportFormat => p().format;
  const quality = numField({ label: "JPG quality", min: 1, max: 100, decimals: 0, disabled: () => fmt().kind !== "jpg",
    get: () => { const f = fmt(); return f.kind === "jpg" ? f.quality : 92; }, set: (v) => set({ format: { kind: "jpg", quality: Math.round(v ?? 92) } }) });

  const browse = hasFolderPicker()
    ? h("button", { type: "button", class: "btn compact", onclick: async () => { const f = await pickFolder().catch(() => null); if (f) set({ folder: f }); } }, "Browse…")
    : undefined;
  const folder = textField({ label: "Folder", caption: "Keep the files here: Windows reads the wallpaper from them.", placeholder: "Pictures\\Wallpaper Splitter\\<image>",
    get: () => p().folder, set: (v) => set({ folder: v }), ...(browse ? { extra: browse } : {}) });

  const example = h("span", { class: "num" });
  const template = textField({ label: "File name", caption: "Tokens: {image} {monitor} {index} {w} {h}", get: () => p().template, set: (v) => set({ template: v || "{image}_{monitor}_{w}x{h}" }) });
  const exampleField: Field = {
    el: h("p", { class: "caption example" }, "Example: ", example),
    sync: () => {
      const m = st.monitors[0];
      const pl = st.placed.find((x) => x.id === m?.id);
      const v = { image: st.image ? stem(st.image.path) : "Zephyr", monitor: m?.name ?? "BenQ EW3270U", index: 1, w: pl?.pxW ?? 3840, h: pl?.pxH ?? 2160 };
      example.textContent = renderTemplate(p().template, v, extFor(p()));
    },
  };

  const fill = textField({ label: "Fill colour", caption: "For any part of a monitor the image does not cover", type: "color", get: () => p().fill, set: (v) => set({ fill: v }) });

  const exp = h("section", { class: "group" }, h("h2", {}, "Export"),
    add(segmented<ExportFormat["kind"]>("Format", [{ value: "png", label: "PNG" }, { value: "jpg", label: "JPG" }, { value: "jxr", label: "HDR JXR" }],
      () => fmt().kind, (k) => set({ format: k === "jpg" ? { kind: "jpg", quality: 92 } : { kind: k } }))),
    h("p", { class: "caption" }, "HDR JXR is used only for HDR images; others are saved as PNG."),
    add(quality),
    add(folder), add(template), add(exampleField),
    add(switchField("Also export one spanning image", "One file across all monitors, beside the per-monitor files", () => p().span, (v) => set({ span: v }))),
    add(switchField("Set wallpaper after export", "Use the exported files as the wallpaper straight away", () => p().applyAfterExport, (v) => set({ applyAfterExport: v }))),
    add(fill),
    add(switchField("JPEG import fix", "Stop Windows recompressing JPG wallpapers (sets JPEGImportQuality to 100)", () => p().jpegImportFix, (v) => set({ jpegImportFix: v }))));

  const startup = h("section", { class: "group" }, h("h2", {}, "Startup"),
    add(switchField("Reopen the last image", "Starts where you left off, with the same framing", () => p().reopenLast, (v) => set({ reopenLast: v }))));

  const page = h("div", { class: "page" }, hdr, exp, startup);
  return { els: [page], sync: () => syncAll(fields), dispose: () => {}, single: true };
}
