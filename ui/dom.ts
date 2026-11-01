// DOM helpers shared by the pages: element builder, inline SVG glyphs (the Inter subset has no arrows,
// check or warning signs), and form fields that sync from state without stealing focus.
// The CSP has no 'unsafe-inline' styles, so nothing here writes a style attribute; el.style is CSSOM.

type Kid = Node | string | null | undefined | false;
type Attrs = Record<string, string | number | boolean | null | undefined | ((e: Event) => void)>;

export function h<K extends keyof HTMLElementTagNameMap>(tag: K, attrs: Attrs = {}, ...kids: Kid[]): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v === null || v === undefined || v === false) continue;
    if (typeof v === "function") el.addEventListener(k.replace(/^on/, ""), v);
    else if (k === "class") el.className = String(v);
    else el.setAttribute(k, v === true ? "" : String(v));
  }
  el.append(...kids.filter((k): k is Node | string => k !== null && k !== undefined && k !== false));
  return el;
}

const NS = "http://www.w3.org/2000/svg";
export function svg<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}): SVGElementTagNameMap[K] {
  const el = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, String(v));
  return el;
}

const GLYPHS = {
  warn: "M8 2.5 14.5 13.5H1.5Z M8 6.5V9.5 M8 11.25V11.5",
  check: "M3 8.5 6.5 12 13 4.5",
  error: "M8 1.75A6.25 6.25 0 1 1 8 14.25 6.25 6.25 0 0 1 8 1.75Z M8 4.75V8.75 M8 10.75V11",
  folder: "M1.75 3.5H6L7.5 5H14.25V12.75H1.75Z",
};
export type GlyphName = keyof typeof GLYPHS;

/** A 16 px stroke glyph in currentColor, hidden from screen readers (the words next to it carry the meaning). */
export function glyph(name: GlyphName): SVGSVGElement {
  const s = svg("svg", { viewBox: "0 0 16 16", width: 16, height: 16, class: "glyph", "aria-hidden": "true", focusable: "false" });
  s.append(svg("path", { d: GLYPHS[name], fill: "none", stroke: "currentColor", "stroke-width": 1.5, "stroke-linecap": "round", "stroke-linejoin": "round" }));
  return s;
}

/** Positions an absolutely placed element (CSSOM only). */
export function place(el: HTMLElement | SVGElement, x: number, y: number, w?: number, hgt?: number): void {
  el.style.left = `${x}px`;
  el.style.top = `${y}px`;
  if (w !== undefined) el.style.width = `${w}px`;
  if (hgt !== undefined) el.style.height = `${hgt}px`;
}

// Registration ticks: crop marks outside the corners of the selected pane (light table and cutting mat).
const TICK = 10; // registration tick length, px
const GAP = 4; // gap between a pane corner and its tick
export function ticks(): SVGSVGElement {
  const s = svg("svg", { class: "ticks", "aria-hidden": "true" });
  for (let i = 0; i < 8; i++) s.append(svg("line"));
  return s;
}
/** Crop marks outside the four corners of a w x h pane; the svg overhangs the pane by TICK + GAP. */
export function drawTicks(s: SVGSVGElement, w: number, hgt: number): void {
  const o = TICK + GAP;
  place(s, -o, -o, w + 2 * o, hgt + 2 * o);
  const [x0, y0, x1, y1] = [o, o, o + w, o + hgt];
  const lines: [number, number, number, number][] = [
    [x0 - GAP, y0, 0, y0], [x0, y0 - GAP, x0, 0], [x1 + GAP, y0, x1 + o, y0], [x1, y0 - GAP, x1, 0],
    [x0 - GAP, y1, 0, y1], [x0, y1 + GAP, x0, y1 + o], [x1 + GAP, y1, x1 + o, y1], [x1, y1 + GAP, x1, y1 + o],
  ];
  [...s.children].forEach((l, i) => {
    const c = lines[i];
    if (c) ["x1", "y1", "x2", "y2"].forEach((a, j) => l.setAttribute(a, String(c[j])));
  });
}

let uid = 0;
export const nextId = (p: string): string => `${p}-${++uid}`;

export type Field = { el: HTMLElement; sync: () => void };
export const syncAll = (fields: Field[]): void => fields.forEach((f) => f.sync());

type NumOpts = { label: string; unit?: string; get: () => number | null; set: (v: number | null) => void; step?: number; min?: number; max?: number; decimals?: number; placeholder?: string; disabled?: () => boolean; title?: string };

/** A labelled number field; commits on change (Enter, blur, arrow steps) and never overwrites itself while focused. */
export function numField(o: NumOpts): Field {
  const id = nextId("f");
  const input = h("input", { id, class: "input num", type: "number", inputmode: "decimal", step: o.step ?? 1, min: o.min, max: o.max, placeholder: o.placeholder, title: o.title });
  const fmt = (v: number | null): string => (v === null ? "" : String(Number(v.toFixed(o.decimals ?? 1))));
  const sync = (): void => {
    input.disabled = o.disabled?.() ?? false;
    if (document.activeElement !== input) input.value = fmt(o.get());
  };
  input.addEventListener("change", () => {
    const raw = input.value.trim();
    const v = raw === "" ? null : Number(raw);
    if (v === null ? o.placeholder === undefined : !Number.isFinite(v)) return sync();
    const clamped = v === null ? null : Math.min(o.max ?? Infinity, Math.max(o.min ?? -Infinity, v));
    o.set(clamped);
    input.value = fmt(clamped);
  });
  const el = h("div", { class: "field" }, h("label", { for: id }, o.label), h("span", { class: "unit-wrap" }, input, o.unit ? h("span", { class: "unit" }, o.unit) : null));
  sync();
  return { el, sync };
}

type SegOpt<T extends string> = { value: T; label: string; disabled?: () => string | null };
/** A segmented control (aria-pressed buttons). A disabled option says why in its tooltip. */
export function segmented<T extends string>(label: string, opts: SegOpt<T>[], get: () => T, set: (v: T) => void): Field {
  const id = nextId("seg");
  const buttons = opts.map((o) => h("button", { type: "button", onclick: () => set(o.value) }, o.label));
  const sync = (): void => buttons.forEach((b, i) => {
    const o = opts[i];
    if (!o) return;
    const why = o.disabled?.() ?? null;
    b.disabled = why !== null;
    b.title = why ?? "";
    b.setAttribute("aria-pressed", String(get() === o.value));
  });
  const el = h("div", { class: "field" }, h("span", { id, class: "label" }, label), h("div", { class: "seg", role: "group", "aria-labelledby": id }, ...buttons));
  sync();
  return { el, sync };
}

/** A switch row with a plain label and a one-line caption. */
export function switchField(label: string, caption: string, get: () => boolean, set: (v: boolean) => void): Field {
  const id = nextId("sw");
  const sw = h("button", { type: "button", class: "switch", role: "switch", "aria-labelledby": id, onclick: () => set(!get()) });
  const sync = (): void => sw.setAttribute("aria-checked", String(get()));
  const el = h("div", { class: "row" }, h("div", { class: "row-text" }, h("span", { id }, label), h("span", { class: "caption" }, caption)), sw);
  sync();
  return { el, sync };
}

/** A key hint, e.g. key("Ctrl", "O"). */
export const keys = (...k: string[]): HTMLElement => h("span", { class: "keys", "aria-hidden": "true" }, ...k.map((x) => h("span", { class: "key" }, x)));
