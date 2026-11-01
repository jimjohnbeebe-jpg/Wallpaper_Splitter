// The shell: page switching from the sidebar (click, or Up/Down while a nav item has focus), the footer,
// app-wide keys (Ctrl+O), file drag and drop, and the events from the preview windows.
import { footer } from "./footer.ts";
import { api, listen, onDragDrop, type PlacedMonitor } from "./ipc.ts";
import { monitorsPage } from "./monitors.ts";
import { prefsPage } from "./prefs.ts";
import { splitPage, type Page } from "./split.ts";
import { createApp, type App } from "./state.ts";

export type PageId = "split" | "monitors" | "prefs";

/** The nav item to focus after pressing `key` on item `index` of `count`, or null when the key is not ours. */
export function navTarget(index: number, count: number, key: string): number | null {
  if (key === "ArrowDown") return (index + 1) % count;
  if (key === "ArrowUp") return (index - 1 + count) % count;
  if (key === "Home") return 0;
  if (key === "End") return count - 1;
  return null;
}

const PAGES: Record<PageId, (app: App) => Page> = { split: splitPage, monitors: monitorsPage, prefs: prefsPage };

function init(): void {
  const app = createApp(api);
  const root = document.getElementById("page");
  const foot = document.querySelector<HTMLElement>(".status");
  if (!root || !foot) return;
  const status = footer(app, foot);
  let page: Page | null = null;

  const show = (id: PageId): void => {
    for (const b of document.querySelectorAll<HTMLButtonElement>(".nav-item")) {
      if (b.dataset["page"] === id) b.setAttribute("aria-current", "page");
      else b.removeAttribute("aria-current");
    }
    page?.dispose();
    page = PAGES[id](app);
    root.classList.toggle("single", page.single === true);
    root.replaceChildren(...page.els);
    page.sync();
  };

  app.subscribe(() => {
    document.documentElement.classList.toggle("hdr-preview", app.state.settings.prefs.hdrPreview);
    page?.sync();
    status.sync();
  });

  const items = [...document.querySelectorAll<HTMLButtonElement>(".nav-item")];
  items.forEach((b, i) => {
    b.addEventListener("click", () => show(b.dataset["page"] as PageId));
    b.addEventListener("keydown", (e) => {
      const t = navTarget(i, items.length, e.key);
      if (t === null) return;
      e.preventDefault();
      items[t]?.focus();
      items[t]?.click();
    });
  });

  document.addEventListener("keydown", (e) => {
    if (e.ctrlKey && !e.altKey && !e.metaKey && e.key.toLowerCase() === "o") {
      e.preventDefault();
      show("split");
      void app.pick();
    }
  });

  // Tauri's own events can fail outside the app shell (a plain browser); the page still works without them.
  const safe = (p: Promise<unknown>): void => void p.catch(() => {});
  try {
    safe(onDragDrop((e) => {
      document.body.classList.toggle("dropping", e.type === "enter" || e.type === "over");
      if (e.type === "drop" && e.paths[0]) {
        show("split");
        void app.open(e.paths[0]);
      }
    }));
    safe(listen<PlacedMonitor[]>("preview-changed", (placed) => app.setPlaced(placed)));
    safe(listen<null>("preview-closed", () => safe(app.refreshMonitors()))); // the preview may have switched HDR
  } catch { /* not inside Tauri */ }

  show("split");
  void app.start();
}

if (typeof document !== "undefined") init();
