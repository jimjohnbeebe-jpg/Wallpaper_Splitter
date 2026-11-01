// The Split page: the light table, its empty state with recent images, the inspector, and the page's
// keyboard shortcuts (Ctrl+Enter set wallpaper, Ctrl+E export only, F11 preview).
import { h, keys } from "./dom.ts";
import { inspector } from "./inspector.ts";
import { fileUrl, type Session } from "./ipc.ts";
import { basename, type App } from "./state.ts";
import { lightTable } from "./table.ts";

export type Page = { els: HTMLElement[]; sync: () => void; dispose: () => void; single?: boolean };

function recentItem(app: App, s: Session): HTMLElement {
  const name = basename(s.image);
  const folder = s.image.slice(0, Math.max(0, s.image.length - name.length - 1));
  const thumb = h("img", { class: "thumb", alt: "", loading: "lazy", decoding: "async", draggable: "false" });
  const well = h("span", { class: "thumb-well" }, thumb);
  // WebView2 cannot show every format (JXL, TIFF), and a moved file has no thumbnail: show an empty well.
  thumb.addEventListener("error", () => { thumb.hidden = true; });
  try { thumb.src = fileUrl(s.image); } catch { thumb.hidden = true; }
  return h("li", {}, h("button", { type: "button", class: "recent-item", title: s.image, onclick: () => void app.open(s.image, s.framing) },
    well, h("span", { class: "recent-text" }, h("span", { class: "name" }, name), h("span", { class: "caption" }, folder))));
}

function emptyState(app: App) {
  const list = h("ul", { class: "recent-list" });
  const recent = h("div", { class: "recent" }, h("h2", {}, "Recent"), list);
  const el = h("div", { class: "empty-state" },
    h("p", { class: "empty" }, "Open an image to start"),
    h("div", { class: "empty-actions" },
      h("button", { type: "button", class: "btn primary", onclick: () => void app.pick() }, "Open image"),
      keys("Ctrl", "O")),
    h("p", { class: "caption drop-hint" }, "Or drop an image file anywhere in this window."),
    recent);
  let key = "";
  const sync = (): void => {
    el.hidden = app.state.image !== null;
    const r = app.state.settings.recent;
    const k = r.map((s) => s.image).join("\n");
    if (k === key) return;
    key = k;
    recent.hidden = r.length === 0;
    list.replaceChildren(...r.map((s) => recentItem(app, s)));
  };
  return { el, sync };
}

export function splitPage(app: App): Page {
  const table = lightTable(app);
  const empty = emptyState(app);
  table.el.append(empty.el);
  const insp = inspector(app);

  const onKey = (e: KeyboardEvent): void => {
    const ctrl = e.ctrlKey && !e.altKey && !e.metaKey;
    const k = e.key.toLowerCase();
    let act: (() => void) | null = null;
    if (ctrl && k === "enter") act = () => void app.run("apply");
    else if (ctrl && k === "e") act = () => void app.run("export");
    else if (e.key === "F11" && !e.ctrlKey && !e.altKey) act = () => void app.run("preview");
    if (!act) return;
    e.preventDefault();
    act();
  };
  document.addEventListener("keydown", onKey);

  return {
    els: [table.el, insp.el],
    sync: () => { table.sync(); empty.sync(); insp.sync(); },
    dispose: () => { table.dispose(); document.removeEventListener("keydown", onKey); },
  };
}
