/**
 * A pretend controller for the mock backend (`?pad=1`), so the controller
 * mode can be looked at in a plain browser and in headless screenshots.
 *
 * - The keyboard stands in for the pad: arrows move, Enter is A, Backspace
 *   is B, X and Y are X and Y, Q/E are LB/RB, Page Up/Down are LT/RT, M is
 *   Menu, V is View. Letters type as usual in a text field.
 * - `?padseq=down,down,right,accept` plays those actions once the app has
 *   started, a few hundred milliseconds apart, for a screenshot of where
 *   they lead. Where the focus went after each is written to
 *   `<body data-padlog>`, for a `--dump-dom` to read.
 */
import type { PadAction } from "$lib/ipc/gamepad";
import { handle } from "./nav";
import { pad } from "./state.svelte";

const KEYS: Record<string, PadAction> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
  Enter: "accept",
  Backspace: "back",
  PageUp: "pageUp",
  PageDown: "pageDown",
};
const LETTERS: Record<string, PadAction> = {
  x: "primary",
  y: "secondary",
  q: "prevTab",
  e: "nextTab",
  m: "menu",
  v: "view",
};

export function installMockPad(q: URLSearchParams) {
  pad.usedPad();
  window.addEventListener(
    "keydown",
    (e) => {
      if (!e.isTrusted || e.ctrlKey || e.altKey || e.metaKey) return;
      const typing = (e.target as HTMLElement | null)?.closest(
        "input, textarea, [contenteditable]",
      );
      const a = KEYS[e.key] ?? (typing ? undefined : LETTERS[e.key.toLowerCase()]);
      if (!a || (typing && (a === "back" || a === "left" || a === "right"))) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      handle(a, e.repeat);
    },
    { capture: true },
  );

  const seq = (q.get("padseq") ?? "").split(",").filter(Boolean) as PadAction[];
  if (seq.length === 0) return;
  void (async () => {
    const { app } = await import("$lib/stores/app.svelte");
    const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
    while (!app.initialized) await wait(100);
    await wait(Number(q.get("padwait") ?? 1500));
    for (const a of seq) {
      handle(a, false);
      await wait(350);
      const el = document.activeElement;
      document.body.dataset.padlog =
        (document.body.dataset.padlog ?? "") +
        ` | ${a}: ${el?.tagName} ${(el?.textContent ?? "").trim().slice(0, 24)} @${Math.round(el?.getBoundingClientRect().top ?? 0)}`;
    }
  })();
}
