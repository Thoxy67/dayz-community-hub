/**
 * The pad's actions, acted on in the page.
 *
 * - Directions move the focus to the nearest focusable element that way
 *   (`spatial.ts`), within the topmost open dialog or popup, else the page.
 *   Inside a virtual list they go row by row through every row (`list.ts`).
 *   A select's or a menu's popup is driven with its own arrow keys instead.
 * - Accept clicks what is focused (opens a select, toggles a switch); on a
 *   text field in Steam's big-screen interface it also opens Steam's keyboard.
 * - Back closes the topmost popup or dialog, else is Escape for the view
 *   (which clears its selection).
 * - LB/RB go to the previous/next view, LT/RT page through a list or scroll.
 * - X, Y, Menu and View are the view's own (`actions.svelte.ts`).
 *
 * Elements opt out with `data-pad-skip` (and everything inside), in with
 * `data-pad-focus`. Only the active scope is searched, once per press.
 */
import { tick } from "svelte";
import type { PadAction } from "$lib/ipc/gamepad";
import { openUrl } from "$lib/ipc/native";
import { app, type ViewId } from "$lib/stores/app.svelte";
import { registry, type PadButtonCommand } from "./actions.svelte";
import { indexOf, listOf, rememberRow, rememberedRow, rowEl } from "./list";
import { cost, type Box, type Dir } from "./spatial";
import { pad } from "./state.svelte";

const FOCUSABLE = [
  "a[href]",
  "button",
  "input:not([type=hidden])",
  "select",
  "textarea",
  "summary",
  "[contenteditable='']",
  "[contenteditable=true]",
  "[tabindex]:not([tabindex='-1'])",
  "[role=button]",
  "[role=tab]",
  "[role=radio]",
  "[role=switch]",
  "[role=checkbox]",
  "[role=slider]",
  "[role=menuitem]",
  "[data-pad-index]",
  "[data-pad-focus]",
].join(",");

/** Popups that bits-ui drives with the keyboard: the pad speaks to them in keys. */
const KEYED =
  "[data-select-content],[data-menu-content],[data-dropdown-menu-content],[data-context-menu-content]";
/** Anything that takes the whole interaction while it is open. */
const SCOPES = `[data-dialog-content],[data-popover-content],[role=dialog][aria-modal=true],[role=alertdialog],[data-pad-scope],${KEYED}`;
/** Triggers bits-ui opens from the keyboard (they open on pointerdown, not click). */
const KEY_TRIGGERS = "[data-select-trigger],[data-dropdown-menu-trigger],[data-menu-trigger]";
const TEXT =
  "textarea, [contenteditable=''], [contenteditable=true], input:not([type=checkbox],[type=radio],[type=range],[type=button],[type=submit],[type=reset],[type=file],[type=color],[type=hidden])";

/** Parts of the page a move prefers to stay in. */
const REGIONS = "[data-pad-region], nav, aside";
/** What leaving a region sideways costs, in pixels of distance. */
const ELSEWHERE = 400;

let views: readonly ViewId[] = [];

/** Events this module made up: they reach the component, not the window's own shortcuts. */
const synthetic = new WeakSet<Event>();

function key(target: Element, k: string, contained = true) {
  const opts = { key: k, code: k, bubbles: true, cancelable: true, composed: true };
  const down = new KeyboardEvent("keydown", opts);
  if (contained) synthetic.add(down);
  target.dispatchEvent(down);
  const up = new KeyboardEvent("keyup", opts);
  if (contained) synthetic.add(up);
  target.dispatchEvent(up);
}

const visible = (el: Element) =>
  el.getClientRects().length > 0 &&
  (typeof el.checkVisibility !== "function" ||
    el.checkVisibility({ visibilityProperty: true, opacityProperty: false }));

/** The topmost open dialog or popup, else the app. */
function scope(): Element {
  const open = [...document.querySelectorAll(SCOPES)].filter(visible);
  return open.at(-1) ?? document.getElementById("app") ?? document.body;
}

/** The section of the view on screen. */
const viewRoot = () => document.querySelector(`[data-view="${app.view}"]`);

function candidates(root: Element): HTMLElement[] {
  const out: HTMLElement[] = [];
  for (const el of root.querySelectorAll<HTMLElement>(FOCUSABLE)) {
    if (el.matches(":disabled, [aria-disabled=true]")) continue;
    if (el.closest("[data-pad-skip], [inert], [hidden], [aria-hidden=true]")) continue;
    // Inside a list, only its rows: their buttons are reached through X/Y or the details.
    const list = el.closest("[data-pad-list]");
    if (list && !el.hasAttribute("data-pad-index")) continue;
    if (!visible(el)) continue;
    out.push(el);
  }
  return out;
}

const box = (el: Element): Box => el.getBoundingClientRect();

function focus(el: HTMLElement) {
  el.focus({ preventScroll: true });
  reveal(el);
  const list = listOf(el);
  const i = indexOf(el);
  if (list && i !== null) remember(list.node, i);
}

function remember(list: Element, i: number) {
  rememberRow(list, i);
  if (document.getElementById("app")?.contains(list)) lastRow.set(app.view, i);
}

/** The element the pad is on: the focused one if it is in the scope. */
function current(root: Element): HTMLElement | null {
  const a = document.activeElement as HTMLElement | null;
  if (!a || a === document.body || !root.contains(a) || !visible(a)) return null;
  return a;
}

/**
 * Put the focus somewhere when nothing in the scope has it (the start, a
 * view just opened, a row that was redrawn away): what had it last in this
 * view, else the view's list at the row it was on (scrolling to it), else
 * the first thing in reading order.
 */
function land(root: Element): boolean {
  const inApp = root.id === "app";
  if (inApp) {
    const mem = remembered.get(app.view);
    if (mem?.isConnected && visible(mem) && !mem.hasAttribute("data-pad-index")) {
      focus(mem);
      return true;
    }
  }
  const within = (inApp && viewRoot()) || root;
  const listNode = [...within.querySelectorAll("[data-pad-list]")].find(visible);
  const list = listNode ? listOf(listNode) : null;
  const n = list?.api.count() ?? 0;
  if (list && n > 0) {
    const sel = list.api.current?.() ?? -1;
    const i = rememberedRow(list.node) ?? (inApp ? lastRow.get(app.view) : undefined) ?? sel;
    void goRow(list.node, list.api, Math.min(n - 1, Math.max(0, i)));
    return true;
  }
  let all = candidates(within);
  if (all.length === 0 && within !== root) all = candidates(root);
  // Top-left first: what reading order would reach first.
  const first = all.sort((a, b) => {
    const x = box(a);
    const y = box(b);
    return Math.abs(x.top - y.top) > 8 ? x.top - y.top : x.left - y.left;
  })[0];
  if (!first) return false;
  focus(first);
  return true;
}

/** The row each view's list was last on, for a list drawn anew (a layout change remounts it). */
const lastRow = new Map<ViewId, number>();

/** The focused element of each view, to come back to it. */
const remembered = new Map<ViewId, HTMLElement>();

let pending = 0;
/** Bring row `i` of a list into view and focus it once it is drawn. */
async function goRow(node: Element, api: { reveal: (i: number) => void }, i: number) {
  const token = ++pending;
  remember(node, i);
  api.reveal(i);
  for (let f = 0; f < 30; f++) {
    const row = rowEl(node, i);
    if (row) {
      if (token === pending) {
        row.focus({ preventScroll: true });
        reveal(row);
      }
      return;
    }
    await frame();
    if (token !== pending) return;
  }
}

function move(dir: Dir) {
  const root = scope();
  if (root.matches(KEYED)) {
    const map = { up: "ArrowUp", down: "ArrowDown", left: "ArrowLeft", right: "ArrowRight" };
    const a = document.activeElement;
    key(a && a !== document.body ? a : root, map[dir]);
    return;
  }
  const from = current(root);
  if (!from) {
    land(root);
    return;
  }
  // A slider takes Left and Right itself.
  if ((dir === "left" || dir === "right") && from.matches("[role=slider]")) {
    key(from, dir === "left" ? "ArrowLeft" : "ArrowRight");
    return;
  }
  // Up and Down inside a list go through its rows, mounted or not.
  const list = listOf(from);
  if (list && (dir === "up" || dir === "down")) {
    const at = indexOf(from) ?? rememberedRow(list.node) ?? 0;
    const n = list.api.count();
    const next = at + (dir === "down" ? 1 : -1);
    if (next >= 0 && next < n) {
      void goRow(list.node, list.api, next);
      return;
    }
    if (dir === "down") return; // the last row: nothing below it belongs to the list
  }
  const all = candidates(root).filter((c) => c !== from && !from.contains(c));
  // Regions (the shell's parts, and any nav or aside: the settings' index,
  // a details pane) keep a move among their own. Up and Down leave one only
  // when it has nothing more that way (Down in a page goes to the next field
  // of that page, not to a link of the index beside it); Left and Right
  // leave more readily, since that is how one gets from a list to its details.
  const home = scrollerOf(from);
  const region = from.closest(REGIONS);
  const at = shownBox(from);
  const scored: [number, HTMLElement][] = [];
  for (const c of all) {
    const k = cost(at, box(c), dir);
    const away =
      c.closest(REGIONS) === region ? 0 : dir === "up" || dir === "down" ? 1e6 : ELSEWHERE;
    if (k !== null) scored.push([k + away, c]);
  }
  scored.sort((a, b) => a[0] - b[0]);
  // Nearest first; the first one the player can actually see wins. One that
  // is scrolled out of sight counts only in the scroller the focus is in
  // (the next field below the fold); one under a drawer or a popup never.
  let to = scored.find(([, c]) => seen(c) || (!!home?.contains(c) && !covered(c)))?.[1] ?? null;
  if (!to) return;
  const target = listOf(to);
  // Entering a list lands on the row it remembers (or its selection) when
  // that row is on screen, rather than on whichever row happens to be nearest.
  if (target && target.node !== list?.node) {
    const r = rememberedRow(target.node) ?? target.api.current?.();
    const row = r !== undefined && r >= 0 ? rowEl(target.node, r) : null;
    if (row && visible(row) && isOnScreen(row, target.node)) to = row;
  }
  focus(to);
}

/** The point of `el` the player would look at: the middle of its part inside the window. */
function middle(el: Element): [number, number] | null {
  const r = box(el);
  const left = Math.max(r.left, 0);
  const right = Math.min(r.right, window.innerWidth);
  const top = Math.max(r.top, 0);
  const bottom = Math.min(r.bottom, window.innerHeight);
  if (right <= left || bottom <= top) return null;
  return [(left + right) / 2, (top + bottom) / 2];
}

/**
 * The part of a wide element (a list row) that is not under something else:
 * a details drawer over the right of the list must not make the row count as
 * reaching under it, or Right would have nowhere to go.
 */
function shownBox(el: Element): Box {
  const r = box(el);
  if (r.right - r.left < 240) return r;
  const y = Math.min(Math.max((r.top + r.bottom) / 2, 0), window.innerHeight - 1);
  const STEPS = 16;
  const step = (r.right - r.left) / STEPS;
  let left = Infinity;
  let right = -Infinity;
  for (let k = 0; k <= STEPS; k++) {
    const x = Math.min(r.left + k * step, r.right - 1);
    const hit = document.elementFromPoint(x, y);
    if (hit && el.contains(hit)) {
      left = Math.min(left, x);
      right = Math.max(right, x);
    }
  }
  return right > left ? { left, right, top: r.top, bottom: r.bottom } : r;
}

/** On screen and not hidden behind anything (a drawer, a popup, its scroller's edge). */
function seen(el: Element): boolean {
  const m = middle(el);
  if (!m) return false;
  const hit = document.elementFromPoint(m[0], m[1]);
  return !!hit && (hit === el || el.contains(hit));
}

/**
 * Under something else (a drawer, a popup), rather than only scrolled out of
 * its scroller's sight: what lies past the scroller's edge is not covered,
 * whatever the window draws there (the status bar under a long page).
 */
function covered(el: Element): boolean {
  const m = middle(el);
  if (!m) return false;
  const sc = scrollerOf(el);
  if (sc) {
    const v = box(sc);
    if (m[0] < v.left || m[0] > v.right || m[1] < v.top || m[1] > v.bottom) return false;
  }
  const hit = document.elementFromPoint(m[0], m[1]);
  return !!hit && hit !== el && !el.contains(hit) && !hit.contains(el) && !sc?.contains(hit);
}

/**
 * Scroll the scrollers holding `el` just enough to show it. Not
 * `scrollIntoView`: that also scrolls boxes that clip their content
 * (`overflow: hidden`), shifting a pane sideways out of its frame.
 */
function reveal(el: Element) {
  const MARGIN = 8;
  for (let s = scrollerOf(el); s; s = scrollerOf(s)) {
    const r = box(el);
    const v = box(s);
    if (r.top < v.top) s.scrollTop -= v.top - r.top + MARGIN;
    else if (r.bottom > v.bottom)
      s.scrollTop += Math.min(r.bottom - v.bottom + MARGIN, r.top - v.top);
  }
}

const isOnScreen = (el: Element, scroller: Element) => {
  const a = box(el);
  const b = box(scroller);
  return a.bottom > b.top && a.top < b.bottom;
};

function accept() {
  const root = scope();
  if (root.matches(KEYED)) {
    const a = document.activeElement;
    key(a && a !== document.body ? a : root, "Enter");
    return;
  }
  const el = current(root);
  if (!el) {
    land(root);
    return;
  }
  if (el.matches(TEXT)) {
    el.focus();
    // Game Mode has no keyboard: Steam's own comes up over the window.
    if (pad.steamUi) void openUrl("steam://open/keyboard").catch(() => {});
    return;
  }
  if (el.matches(KEY_TRIGGERS)) return key(el, "Enter");
  if (el.hasAttribute("data-pad-index")) {
    const t =
      el.querySelector<HTMLElement>("[data-pad-accept], [role=row]") ?? el.firstElementChild;
    (t instanceof HTMLElement ? t : el).click();
  } else el.click();
  // Selecting a row can redraw its list elsewhere (the details pane opening
  // beside it): the focus then goes back to the same row.
  void settle();
}

/** After an action, a focus that was lost is put back. */
async function settle() {
  await tick();
  await frame();
  const root = scope();
  if (!current(root)) land(root);
}

function back() {
  const root = scope();
  const a = document.activeElement;
  const target = a && a !== document.body ? a : root;
  if (root.id !== "app") {
    // bits-ui closes on Escape at the document; an in-app overlay listens on the window.
    key(target, "Escape", root.matches("[data-dialog-content],[data-popover-content]," + KEYED));
    return;
  }
  // The view's own Escape: the lists clear their selection, the details close.
  key(target, "Escape", false);
  void settle();
}

async function switchView(delta: number) {
  if (scope().id !== "app" || views.length === 0) return;
  const i = views.indexOf(app.view);
  const next = views[(i + delta + views.length) % views.length]!;
  app.go(next);
  // The view may be loading for the first time: wait until it has something to focus.
  for (let f = 0; f < 30; f++) {
    await frame();
    if (app.view !== next) return;
    if (viewRoot()?.querySelector(FOCUSABLE)) {
      land(scope());
      return;
    }
  }
}

/** Scroll what holds the focus (or the view's main scroller) by a page. */
function page(delta: 1 | -1) {
  const root = scope();
  if (root.matches(KEYED))
    return key(document.activeElement ?? root, delta > 0 ? "PageDown" : "PageUp");
  const from = current(root);
  // The list the focus is in; with nothing focused, the view's own list.
  const within = root.id === "app" ? (viewRoot() ?? root) : root;
  const list = listOf(from ?? within.querySelector("[data-pad-list]"));
  if (list) {
    const at = indexOf(from) ?? rememberedRow(list.node) ?? 0;
    const step = list.api.page?.() ?? 10;
    const next = Math.max(0, Math.min(list.api.count() - 1, at + delta * step));
    void goRow(list.node, list.api, next);
    return;
  }
  const scroller = scrollerOf(from) ?? mainScroller(root);
  if (!scroller) return;
  const before = from ? box(from) : null;
  scroller.scrollBy({ top: delta * scroller.clientHeight * 0.85 });
  // The focus follows what is on screen, so the next move starts from there.
  // A plain scroll is laid out at once: no need to wait for a frame.
  if (from && isOnScreen(from, scroller)) return;
  const inside = candidates(scroller).filter((c) => isOnScreen(c, scroller) && seen(c));
  if (inside.length === 0) return;
  const y = before ? (before.top + before.bottom) / 2 : 0;
  inside.sort((p, q) => Math.abs(box(p).top - y) - Math.abs(box(q).top - y));
  focus(inside[0]!);
}

/**
 * The next frame, or 20 ms when frames are not being drawn (a hidden or
 * headless window), so a wait for the list to redraw never stalls.
 */
const frame = () =>
  new Promise<void>((done) => {
    const t = setTimeout(done, 20);
    requestAnimationFrame(() => (clearTimeout(t), done()));
  });

function scrollable(el: Element) {
  const s = getComputedStyle(el);
  return /(auto|scroll)/.test(s.overflowY) && el.scrollHeight > el.clientHeight + 1;
}

function scrollerOf(el: Element | null): HTMLElement | null {
  for (let e = el?.parentElement; e; e = e.parentElement) if (scrollable(e)) return e;
  return null;
}

function mainScroller(root: Element): HTMLElement | null {
  const within = root.id === "app" ? (viewRoot() ?? root) : root;
  let bestEl: HTMLElement | null = null;
  let area = 0;
  for (const el of within.querySelectorAll<HTMLElement>(
    "[class*='overflow-y-auto'],[class*='overflow-auto']",
  )) {
    if (!scrollable(el)) continue;
    const a = el.clientWidth * el.clientHeight;
    if (a > area) [bestEl, area] = [el, a];
  }
  return bestEl;
}

function command(button: PadButtonCommand) {
  if (scope().id !== "app") return;
  const c = registry.get(app.view, button);
  if (c) return c.run();
  if (button === "menu") return void switchTo("settings");
  if (button === "leftStick") {
    // Rejoin the last server, from anywhere: the Rejoin card's button.
    void import("$lib/stores/connect.svelte").then(({ connect }) => connect.rejoin());
    return;
  }
  if (button === "view") {
    const field = viewRoot()?.querySelector<HTMLElement>("[data-pad-search]");
    if (field && visible(field)) focus(field);
  }
}

async function switchTo(view: ViewId) {
  const i = views.indexOf(view);
  const at = views.indexOf(app.view);
  if (i >= 0 && at >= 0) await switchView(i - at);
  else app.go(view);
}

/**
 * What the right stick scrolls: what is being read (a pane marked
 * `data-pad-scroll`: a server's details, an article), else the scroller
 * around the focus, else the view's main one.
 */
function scrollTarget(): HTMLElement | null {
  const root = viewRoot();
  if (!root) return null;
  const marked = [...root.querySelectorAll<HTMLElement>("[data-pad-scroll]")].find(
    (el) => visible(el) && scrollable(el),
  );
  return marked ?? scrollerOf(pad.focused) ?? mainScroller(root);
}

function scroll(dir: 1 | -1) {
  if (scope().id !== "app") return;
  scrollTarget()?.scrollBy({ top: dir * 140 });
}

/** One action from a pad. */
export function handle(action: PadAction, _repeat: boolean) {
  if (!pad.enabled) return;
  pad.usedPad();
  switch (action) {
    case "up":
    case "down":
    case "left":
    case "right":
      return move(action);
    case "accept":
      return accept();
    case "back":
      return back();
    case "prevTab":
      return void switchView(-1);
    case "nextTab":
      return void switchView(1);
    case "pageUp":
      return page(-1);
    case "pageDown":
      return page(1);
    case "scrollUp":
      return scroll(-1);
    case "scrollDown":
      return scroll(1);
    case "primary":
    case "secondary":
    case "menu":
    case "view":
    case "leftStick":
    case "rightStick":
      return command(action);
  }
}

/**
 * Wire the page: which views LB/RB cycle through, the mouse taking over again,
 * and the focus being tracked for the hints. Returns the teardown.
 */
export function install(order: readonly ViewId[]) {
  views = order;
  // Made-up keys reach the component they were sent to and the document
  // (where bits-ui listens for Escape), not the views' shortcuts on the
  // window: this listener is on the window before any view has mounted, and
  // stops the rest there.
  const contain = (e: Event) => synthetic.has(e) && e.stopImmediatePropagation();
  const moved = (e: PointerEvent) => {
    if (e.isTrusted && (e.movementX !== 0 || e.movementY !== 0)) pad.usedPointer();
  };
  const pressed = (e: PointerEvent) => e.isTrusted && pad.usedPointer();
  const focusin = (e: FocusEvent) => {
    if (pad.mode !== "gamepad") return;
    const el = e.target as HTMLElement;
    pad.focused = el;
    if (document.getElementById("app")?.contains(el)) remembered.set(app.view, el);
  };
  window.addEventListener("keydown", contain);
  window.addEventListener("keyup", contain);
  window.addEventListener("pointermove", moved, { passive: true });
  window.addEventListener("pointerdown", pressed, { passive: true });
  document.addEventListener("focusin", focusin);
  return () => {
    window.removeEventListener("keydown", contain);
    window.removeEventListener("keyup", contain);
    window.removeEventListener("pointermove", moved);
    window.removeEventListener("pointerdown", pressed);
    document.removeEventListener("focusin", focusin);
  };
}
