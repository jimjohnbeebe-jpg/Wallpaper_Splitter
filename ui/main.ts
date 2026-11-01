// The shell: page switching from the sidebar (click, or Up/Down while a nav item has focus).
// Pages render themselves into #page; the UI build fills in split.ts, monitors.ts and prefs.ts.

export type PageId = "split" | "monitors" | "prefs";

/** The nav item to focus after pressing `key` on item `index` of `count`, or null when the key is not ours. */
export function navTarget(index: number, count: number, key: string): number | null {
  if (key === "ArrowDown") return (index + 1) % count;
  if (key === "ArrowUp") return (index - 1 + count) % count;
  if (key === "Home") return 0;
  if (key === "End") return count - 1;
  return null;
}

function show(page: PageId): void {
  for (const b of document.querySelectorAll<HTMLButtonElement>(".nav-item")) {
    if (b.dataset["page"] === page) b.setAttribute("aria-current", "page");
    else b.removeAttribute("aria-current");
  }
  document.dispatchEvent(new CustomEvent<PageId>("page", { detail: page }));
}

function init(): void {
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
}

if (typeof document !== "undefined") init();
