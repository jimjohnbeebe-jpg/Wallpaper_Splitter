// The footer: one cell per monitor with its HDR switch, then the last action's result (in words, with a
// glyph; errors show the backend's sentence as it is).
import { glyph, h } from "./dom.ts";
import type { App } from "./state.ts";

export function footer(app: App, el: HTMLElement) {
  const st = app.state;
  const result = h("span", { class: "cell result", role: "status" });
  let key = "";
  let switches: { id: string; sw: HTMLButtonElement }[] = [];
  const pending = new Set<string>();

  function build(): void {
    switches = st.monitors.map((m) => {
      const label = h("span", { class: "sr" }, `${m.name} HDR`);
      const sw = h("button", { type: "button", class: "switch", role: "switch", "aria-checked": "false" }, label);
      sw.addEventListener("click", async () => {
        const mon = st.monitors.find((x) => x.id === m.id);
        if (!mon || pending.has(m.id)) return;
        pending.add(m.id);
        sw.setAttribute("aria-busy", "true");
        await app.setHdr(m.id, !mon.hdr.enabled);
        pending.delete(m.id);
        sw.removeAttribute("aria-busy");
        sync();
      });
      el.append(h("span", { class: "cell mon-cell" }, h("span", { class: "mon-label" }, m.name), h("span", { "aria-hidden": "true" }, "HDR"), sw));
      return { id: m.id, sw };
    });
  }

  function syncResult(): void {
    const r = st.result;
    result.replaceChildren();
    result.className = "cell result";
    if (st.busy) {
      result.append(st.busy === "apply" ? "Setting wallpaper…" : st.busy === "export" ? "Exporting…" : "Opening preview…");
    } else if (!r) {
      result.append(st.image ? "Ready" : "No image open");
    } else if (r.kind === "error") {
      result.classList.add("danger");
      result.append(glyph("error"), h("span", { class: "result-text", title: r.text }, r.text));
    } else {
      const folder = r.folder;
      result.append(h("span", { class: "ok" }, glyph("check")), h("span", { class: "result-text" }, r.text));
      if (folder) result.append(h("button", { type: "button", class: "btn compact", onclick: () => app.openFolder(folder) }, "Open folder"));
    }
  }

  function sync(): void {
    const k = st.monitors.map((m) => m.id + m.name).join("|");
    if (k !== key) {
      key = k;
      el.replaceChildren();
      build();
      el.append(result);
    }
    for (const { id, sw } of switches) {
      const m = st.monitors.find((x) => x.id === id);
      if (!m) continue;
      sw.setAttribute("aria-checked", String(m.hdr.enabled));
      sw.disabled = !m.hdr.supported || pending.has(id);
      sw.title = m.hdr.supported ? (m.hdr.enabled ? "Turn HDR off" : "Turn HDR on") : "This monitor doesn't support HDR";
    }
    syncResult();
  }
  return { sync };
}
