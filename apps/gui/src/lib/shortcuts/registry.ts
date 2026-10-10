/**
 * Every shortcut the window answers to, keyboard and controller, in one
 * list. The window's key handler (`shell/keys.ts`), the views' list keys,
 * the controller's X/Y/Menu/View/stick clicks (`lib/gamepad`), the `?`
 * sheet, the About page, the hint bar and the command palette all read it,
 * so a list of shortcuts can no longer tell a different story from the keys.
 *
 * Pure data and matching, no stores: it is tested without a page.
 *
 * Controller conventions (Xbox's, which Steam Input and the Steam Deck
 * follow, the Deck printing the same A/B/X/Y and L1/R1 for LB/RB):
 * A selects, B goes back, the triggers page up and down, the bumpers move
 * between the app's top-level tabs (here its views), Menu opens the app's
 * commands and View its secondary panel.
 * https://learn.microsoft.com/en-us/windows/apps/design/input/gamepad-and-remote-interactions
 * On top of that, every list gives X and Y the same meaning: X marks the row
 * (star a server, tick a mod), Y refreshes it (ping a server, update a mod).
 */
import type { ViewId } from "$lib/stores/app.svelte";

/** A controller button, by what Xbox prints on it. */
export type PadButton =
  "a" | "b" | "x" | "y" | "lb" | "rb" | "lt" | "rt" | "menu" | "view" | "l3" | "r3" | "dpad" | "rs";

/**
 * Where a shortcut works. `global` and `views` everywhere (outside a text
 * field for those without Ctrl); `list` in the three server lists, `saved`
 * in favourites and history only; a view id for that view's own controller
 * buttons; `palette` and `dialog` while one is open; `pad` for the
 * controller's moves that have no key.
 */
export type Scope =
  | "global"
  | "views"
  | "list"
  | "saved"
  | "mods"
  | "news"
  | "palette"
  | "dialog"
  | "pad"
  | "servers"
  | "connect"
  | "offline"
  | "options";

export type Shortcut = {
  id: string;
  scope: Scope;
  /** Each a combination: "Ctrl+K", "?", "↑". Several are alternatives. */
  keys: readonly string[];
  /** Shown as one row "↑ / ↓" rather than two: the keys of one gesture. */
  pair?: boolean;
  /** The controller buttons doing the same, if any. */
  pad?: readonly PadButton[];
  /** The view a `views` entry goes to; its label is the view's own name. */
  view?: ViewId;
  /** The words, in the `keys` dictionary (not for a `views` entry). */
  label?: string;
  /** Shorter words for the controller's hint bar, in the `pad` dictionary. */
  hint?: string;
};

const VIEW_KEYS: [ViewId, string][] = [
  ["servers", "Ctrl+1"],
  ["favorites", "Ctrl+2"],
  ["history", "Ctrl+3"],
  ["friends", "Ctrl+4"],
  ["connect", "Ctrl+5"],
  ["offline", "Ctrl+6"],
  ["mods", "Ctrl+7"],
  ["options", "Ctrl+8"],
  ["news", "Ctrl+9"],
  ["stats", "Ctrl+0"],
  // Ctrl+, is where desktop apps keep their settings (VS Code, Firefox,
  // GNOME apps); F1 is help, and the guide lives on the About page.
  ["settings", "Ctrl+,"],
  ["about", "F1"],
];

export const SHORTCUTS: readonly Shortcut[] = [
  // ── everywhere ──────────────────────────────────────────────────────────
  {
    id: "palette",
    scope: "global",
    keys: ["Ctrl+K"],
    pad: ["menu"],
    label: "palette",
    hint: "search",
  },
  { id: "search", scope: "global", keys: ["Ctrl+F", "/"], label: "search" },
  { id: "sheet", scope: "global", keys: ["?"], pad: ["view"], label: "sheet", hint: "shortcuts" },
  { id: "refresh", scope: "global", keys: ["Ctrl+R"], label: "refresh" },
  { id: "updateMods", scope: "global", keys: ["Ctrl+U"], label: "updateMods" },
  { id: "rejoin", scope: "global", keys: ["Ctrl+L"], pad: ["l3"], label: "rejoin", hint: "rejoin" },
  { id: "back", scope: "global", keys: ["Esc"], pad: ["b"], label: "back", hint: "back" },

  // ── views ───────────────────────────────────────────────────────────────
  ...VIEW_KEYS.map(([view, k]): Shortcut => ({
    id: `view.${view}`,
    scope: "views",
    keys: [k],
    view,
  })),
  {
    id: "view.prev",
    scope: "views",
    keys: ["Ctrl+PgUp"],
    pad: ["lb"],
    label: "viewPrev",
    hint: "views",
  },
  { id: "view.next", scope: "views", keys: ["Ctrl+PgDn"], pad: ["rb"], label: "viewNext" },

  // ── the server lists: servers, favourites, history ─────────────────────
  { id: "list.move", scope: "list", keys: ["↑", "↓"], pair: true, pad: ["dpad"], label: "move" },
  { id: "list.edges", scope: "list", keys: ["Home", "End"], pair: true, label: "edges" },
  {
    id: "list.page",
    scope: "list",
    keys: ["PgUp", "PgDn"],
    pair: true,
    pad: ["lt", "rt"],
    label: "page",
  },
  { id: "list.join", scope: "list", keys: ["Enter"], pad: ["a"], label: "join", hint: "join" },
  { id: "list.fav", scope: "list", keys: ["F"], pad: ["x"], label: "favorite", hint: "favorite" },
  { id: "list.ping", scope: "list", keys: ["P"], pad: ["y"], label: "ping", hint: "ping" },
  { id: "list.detail", scope: "list", keys: ["I"], pad: ["r3"], label: "detail", hint: "details" },
  { id: "list.copy", scope: "list", keys: ["C"], label: "copyAddress" },
  { id: "list.link", scope: "list", keys: ["L"], label: "copyLink" },
  { id: "list.direct", scope: "list", keys: ["D"], label: "direct" },
  { id: "list.close", scope: "list", keys: ["Esc"], pad: ["b"], label: "deselect", hint: "back" },
  { id: "saved.remove", scope: "saved", keys: ["Del"], label: "remove" },

  // ── mods ────────────────────────────────────────────────────────────────
  { id: "mods.move", scope: "mods", keys: ["↑", "↓"], pair: true, pad: ["dpad"], label: "move" },
  { id: "mods.edges", scope: "mods", keys: ["Home", "End"], pair: true, label: "edges" },
  { id: "mods.tick", scope: "mods", keys: ["Space"], pad: ["x"], label: "tick", hint: "tickMod" },
  { id: "mods.update", scope: "mods", keys: ["U"], pad: ["y"], label: "updateMod", hint: "update" },
  { id: "mods.managed", scope: "mods", keys: ["M"], label: "managed" },
  { id: "mods.delete", scope: "mods", keys: ["Del"], label: "deleteMod" },
  { id: "mods.close", scope: "mods", keys: ["Esc"], pad: ["b"], label: "deselect", hint: "back" },

  // ── news ────────────────────────────────────────────────────────────────
  {
    id: "news.move",
    scope: "news",
    keys: ["↑", "↓", "J", "K"],
    pair: true,
    pad: ["dpad"],
    label: "article",
  },
  {
    id: "news.open",
    scope: "news",
    keys: ["O"],
    pad: ["x"],
    label: "openArticle",
    hint: "openArticle",
  },

  // ── a view's own controller buttons ─────────────────────────────────────
  {
    id: "servers.refresh",
    scope: "servers",
    keys: [],
    pad: ["y"],
    label: "refresh",
    hint: "refresh",
  },
  { id: "connect.query", scope: "connect", keys: [], pad: ["x"], label: "query", hint: "query" },
  {
    id: "connect.join",
    scope: "connect",
    keys: ["Enter"],
    pad: ["y"],
    label: "join",
    hint: "join",
  },
  { id: "offline.launch", scope: "offline", keys: [], pad: ["x"], label: "launch", hint: "launch" },
  {
    id: "offline.install",
    scope: "offline",
    keys: [],
    pad: ["y"],
    label: "installMode",
    hint: "installMode",
  },
  {
    id: "options.apply",
    scope: "options",
    keys: [],
    pad: ["x"],
    label: "applyRecommended",
    hint: "applyRecommended",
  },
  {
    id: "options.active",
    scope: "options",
    keys: [],
    pad: ["y"],
    label: "activeOnly",
    hint: "activeOnly",
  },

  // ── the search palette ──────────────────────────────────────────────────
  {
    id: "palette.move",
    scope: "palette",
    keys: ["↑", "↓"],
    pair: true,
    pad: ["dpad"],
    label: "move",
  },
  {
    id: "palette.run",
    scope: "palette",
    keys: ["Enter"],
    pad: ["a"],
    label: "run",
    hint: "select",
  },
  {
    id: "palette.section",
    scope: "palette",
    keys: ["Tab", "Shift+Tab"],
    pair: true,
    pad: ["lb", "rb"],
    label: "section",
    hint: "sections",
  },
  {
    id: "palette.keyboard",
    scope: "palette",
    keys: [],
    pad: ["y"],
    label: "keyboard",
    hint: "keyboard",
  },
  {
    id: "palette.close",
    scope: "palette",
    keys: ["Esc"],
    pad: ["b", "menu"],
    label: "close",
    hint: "back",
  },

  // ── dialogs ─────────────────────────────────────────────────────────────
  {
    id: "dialog.confirm",
    scope: "dialog",
    keys: [],
    pad: ["x"],
    label: "confirm",
    hint: "confirm",
  },
  { id: "dialog.close", scope: "dialog", keys: ["Esc"], pad: ["b"], label: "close", hint: "back" },

  // ── the controller's own moves ──────────────────────────────────────────
  { id: "pad.select", scope: "pad", keys: [], pad: ["a"], label: "press", hint: "select" },
  { id: "pad.scroll", scope: "pad", keys: [], pad: ["rs"], label: "scroll" },
];

const BY_ID = new Map(SHORTCUTS.map((s) => [s.id, s]));

export function shortcut(id: string): Shortcut {
  const s = BY_ID.get(id);
  if (!s) throw new Error(`no shortcut "${id}"`);
  return s;
}

export const inScope = (scope: Scope) => SHORTCUTS.filter((s) => s.scope === scope);

/** The order the sheet and the About page list them in. */
export const KEY_GROUPS: readonly Scope[] = [
  "global",
  "views",
  "list",
  "saved",
  "mods",
  "news",
  "palette",
  "dialog",
];
export const PAD_GROUPS: readonly Scope[] = [
  "pad",
  "global",
  "views",
  "list",
  "mods",
  "news",
  "servers",
  "connect",
  "offline",
  "options",
  "palette",
  "dialog",
];

// ── keys ──────────────────────────────────────────────────────────────────

/** The names written in the list, against the names the browser gives (`KeyboardEvent.key`). */
const NAMES: Record<string, string> = {
  Esc: "Escape",
  Del: "Delete",
  Space: " ",
  PgUp: "PageUp",
  PgDn: "PageDown",
  "↑": "ArrowUp",
  "↓": "ArrowDown",
  "←": "ArrowLeft",
  "→": "ArrowRight",
};

type Combo = { ctrl: boolean; shift: boolean; alt: boolean; key: string };

export function parse(combo: string): Combo {
  // "Ctrl+," splits to ["Ctrl", ","]; a lone "+" is not used.
  const parts = combo.split("+");
  const key = parts.pop()!;
  return {
    ctrl: parts.includes("Ctrl"),
    shift: parts.includes("Shift"),
    alt: parts.includes("Alt"),
    key: NAMES[key] ?? key,
  };
}

/** The same combination however it is written, for comparing. */
export const normal = (combo: string) => {
  const c = parse(combo);
  return `${c.ctrl ? "C" : ""}${c.shift ? "S" : ""}${c.alt ? "A" : ""}:${c.key.toLowerCase()}`;
};

/**
 * Whether a key press is this combination. Letters and punctuation ignore
 * Shift unless Ctrl is held (`?` is Shift+/ on one layout and its own key
 * on another); digits also match by their physical key, since an AZERTY
 * keyboard types `&` for Ctrl+1. Ctrl and ⌘ are the same.
 */
export function matches(e: KeyboardEvent, combo: string): boolean {
  const c = parse(combo);
  if ((e.ctrlKey || e.metaKey) !== c.ctrl || e.altKey !== c.alt) return false;
  const printable = c.key.length === 1 && c.key !== " ";
  if (c.shift || !printable || c.ctrl) {
    if (e.shiftKey !== c.shift) return false;
  }
  if (c.key.length === 1) {
    if (e.key.toLowerCase() === c.key.toLowerCase()) return true;
    return /\d/.test(c.key) && e.code === `Digit${c.key}`;
  }
  return e.key === c.key;
}

/** The shortcut of `scope` this key press is, if any. */
export function find(e: KeyboardEvent, scope: Scope): Shortcut | null {
  for (const s of SHORTCUTS) if (s.scope === scope && s.keys.some((k) => matches(e, k))) return s;
  return null;
}

/** The id of the shortcut of `scope` this key press is, if any. */
export const idOf = (e: KeyboardEvent, scope: Scope) => find(e, scope)?.id ?? null;

/** The element takes typing: single keys belong to it, not to the window. */
export const TYPING = "input, textarea, select, [contenteditable]:not([contenteditable=false])";
export const typing = (target: EventTarget | null) =>
  !!(target as Element | null)?.closest?.(TYPING);

/** A combination split into the chips the lists draw: "Ctrl+K" is "Ctrl", "K". */
export const chips = (combo: string) => (combo === "+" ? ["+"] : combo.split("+"));

/** The first combination of a shortcut, written for a tooltip: "Ctrl+K". */
export const keyText = (id: string) => BY_ID.get(id)?.keys[0];

/** The key that goes to a view, for the rail's tooltips and the palette. */
export const viewKey = (view: ViewId) => keyText(`view.${view}`);

/** The view shortcuts' targets, in order. */
export const VIEW_ORDER = VIEW_KEYS.map(([v]) => v);
