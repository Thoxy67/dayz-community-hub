import { mount } from "svelte";

import "./app.css";

if (import.meta.env.DEV || import.meta.env.VITE_MOCK === "1") {
  // An uncaught error leaves an empty window with nothing to say why: in
  // development it is written into the page, where a screenshot shows it.
  const show = (msg: string) =>
    document.body.insertAdjacentHTML(
      "beforeend",
      `<pre data-dev-error style="position:fixed;bottom:0;left:0;right:0;max-height:40vh;overflow:auto;z-index:9999;margin:0;padding:8px;background:#300;color:#fcc;font:11px monospace;white-space:pre-wrap">${msg.replace(/</g, "&lt;")}</pre>`,
    );
  window.addEventListener("error", (e) => show(e.error?.stack ?? e.message));
  window.addEventListener("unhandledrejection", (e) =>
    show(e.reason instanceof Error ? (e.reason.stack ?? e.reason.message) : String(e.reason)),
  );
  // In a plain browser, a pretend backend stands in for Tauri (dev, or a
  // `VITE_MOCK=1` build used for screenshots).
  if (!("__TAURI_INTERNALS__" in window)) (await import("$lib/ipc/mock")).installMock();
}

const { default: App } = await import("./App.svelte");
export default mount(App, { target: document.getElementById("app")! });
