/**
 * The window's own keys: what the `global` and `views` shortcuts of
 * `lib/shortcuts/registry.ts` do, and the one keydown handler that runs
 * them. The views handle their own list keys (`list`, `mods`, `news`) with
 * `idOf(e, scope)` from the same registry.
 *
 * Rules, after the Web Interface Guidelines on keyboard access
 * (https://github.com/vercel-labs/web-interface-guidelines, "Keyboard"):
 * - A key without Ctrl never fires while a text field has the focus: typing
 *   "f" in a search must type an f. Esc there empties the field, then
 *   leaves it.
 * - Esc closes the top-most thing only: an open dialog or popup takes it
 *   (bits-ui closes on Escape); the window acts only when none is open.
 * - Behind an open dialog nothing but Ctrl+K answers.
 * - What a browser would do with a key, an app must not (reload, print,
 *   find, history): those are swallowed in a release build.
 */
import { words } from "$lib/i18n";
import { app } from "$lib/stores/app.svelte";
import { connect } from "$lib/stores/connect.svelte";
import { dialogs } from "$lib/stores/dialogs.svelte";
import { mods } from "$lib/stores/mods.svelte";
import { say } from "$lib/stores/say";
import { servers } from "$lib/stores/servers.svelte";
import { provide, runShortcut } from "$lib/shortcuts/actions";
import { find, matches, typing, VIEW_ORDER } from "$lib/shortcuts/registry";

/**
 * What a browser would do with a key, which an app must not: reload (F5,
 * Ctrl+R, which WebView2 honours and which threw away a running download's
 * state), print, find, view source, caret browsing, history back and
 * forward. Kept in development, where reloading helps.
 */
function browserKey(e: KeyboardEvent): boolean {
  if (import.meta.env.DEV) return false;
  const k = e.key.toLowerCase();
  if (k === "f5" || k === "f3" || k === "f7" || k === "browserback" || k === "browserforward")
    return true;
  if (e.altKey && (k === "arrowleft" || k === "arrowright")) return true;
  return (e.ctrlKey || e.metaKey) && ["r", "p", "f", "g", "j", "s", "o", "0"].includes(k);
}

/** A dialog or popup of bits-ui's is open over the window (it handles its own keys). */
const overlay = () =>
  dialogs.open ||
  !!document.querySelector(
    "[data-dialog-content][data-state=open],[role=alertdialog],[data-popover-content],[data-select-content],[data-menu-content]",
  );

const viewRoot = () => document.querySelector<HTMLElement>(`[data-view="${app.view}"]`);

/** Put the focus in the view's search field, its text selected. */
function focusSearch() {
  const field = [
    ...(viewRoot()?.querySelectorAll<HTMLInputElement>("[data-pad-search]") ?? []),
  ].find((f) => f.getClientRects().length > 0);
  if (!field) return false;
  field.focus();
  field.select();
  return true;
}

/** Esc in a text field of the view: empty it if it is a search with something in it, else leave it. */
function escapeField(el: HTMLElement) {
  if (el instanceof HTMLInputElement && el.matches("[data-pad-search]") && el.value) {
    el.value = "";
    // `bind:value` listens for input.
    el.dispatchEvent(new Event("input", { bubbles: true }));
    return;
  }
  el.blur();
}

function step(delta: number) {
  const i = VIEW_ORDER.indexOf(app.view);
  app.go(VIEW_ORDER[(i + delta + VIEW_ORDER.length) % VIEW_ORDER.length]!);
}

/** Say what the app-wide shortcuts do. Returns the undo. */
export function provideGlobal(): () => void {
  const off = [
    provide("palette", () => {
      dialogs.shortcuts = false;
      dialogs.palette = !dialogs.palette;
    }),
    provide("sheet", () => {
      dialogs.palette = false;
      dialogs.shortcuts = !dialogs.shortcuts;
    }),
    provide("search", () => void focusSearch()),
    provide("refresh", () => void servers.refresh()),
    provide("updateMods", () => {
      if (mods.stale.length > 0) {
        app.go("mods");
        mods.updateStale();
      } else say.ok(words("mods").upToDate);
    }),
    provide("rejoin", () => void connect.rejoin()),
    provide("view.prev", () => step(-1)),
    provide("view.next", () => step(1)),
    ...VIEW_ORDER.map((v) => provide(`view.${v}`, () => app.go(v))),
  ];
  return () => off.forEach((u) => u());
}

/** The window's keydown. */
export function onWindowKey(e: KeyboardEvent) {
  if (browserKey(e)) e.preventDefault();
  if (e.defaultPrevented && !browserKey(e)) return;
  const target = e.target as HTMLElement | null;
  const inField = typing(target);
  const s = find(e, "global") ?? find(e, "views");

  // Ctrl+K toggles the palette from anywhere, over anything.
  if (s?.id === "palette") {
    e.preventDefault();
    runShortcut("palette");
    return;
  }
  // Esc: whatever is open takes it first (bits-ui listens on the document).
  if (e.key === "Escape") {
    if (!overlay() && inField && target && !target.closest("[role=dialog]")) {
      e.preventDefault();
      escapeField(target);
    }
    return;
  }
  if (!s || s.id === "back" || overlay()) return;
  // A key with no Ctrl is the field's while one is focused.
  const combo = s.keys.find((k) => matches(e, k)) ?? "";
  if (inField && !combo.startsWith("Ctrl+") && !/^F\d+$/.test(combo)) return;
  e.preventDefault();
  runShortcut(s.id);
}
