/**
 * What X, Y, Menu, View and the stick clicks do, which depends on the view.
 * A view hands `padActions(view, {...})` its handlers by shortcut id
 * (`lib/shortcuts/registry.ts`); the registry says which button each one is
 * on, so the hint bar, the sheet and the buttons cannot disagree. What a
 * view does not claim falls back to the app's own: Menu the search palette,
 * View the shortcuts, L3 rejoining the last server.
 *
 * `when` says whether an action applies now (X "favourite" needs a server
 * under the focus); the hint bar shows only those that do. It may read
 * `pad.focused`, which changes as the focus moves. When several of a view's
 * actions share a button, the first that applies wins (Y pings the row under
 * the focus, else refreshes the list).
 */
import { untrack } from "svelte";
import type { ViewId } from "$lib/stores/app.svelte";
import { shortcut, type PadButton } from "$lib/shortcuts/registry";

export type PadCommand = {
  run: () => void;
  when?: () => boolean;
};

export type PadButtonCommand =
  "primary" | "secondary" | "menu" | "view" | "leftStick" | "rightStick";

/** Handlers by shortcut id, in the order they are tried. */
export type PadCommands = Record<string, PadCommand>;

/** The button a pad action comes from. */
export const BUTTON: Record<PadButtonCommand, PadButton> = {
  primary: "x",
  secondary: "y",
  menu: "menu",
  view: "view",
  leftStick: "l3",
  rightStick: "r3",
};

class Registry {
  // Raw and replaced whole: the commands are kept as given (a deep proxy
  // would not be the object `unset` compares with), and writing from a
  // view's effect must not make that effect depend on the registry.
  #by = $state.raw<Partial<Record<ViewId, PadCommands>>>({});

  set(view: ViewId, commands: PadCommands) {
    untrack(() => (this.#by = { ...this.#by, [view]: commands }));
  }
  unset(view: ViewId, commands: PadCommands) {
    untrack(() => {
      if (this.#by[view] !== commands) return;
      const next = { ...this.#by };
      delete next[view];
      this.#by = next;
    });
  }
  /** The command a button runs in a view, with its shortcut id, if one applies now. */
  get(view: ViewId, button: PadButton): { id: string; command: PadCommand } | null {
    const all = this.#by[view];
    if (!all) return null;
    for (const [id, command] of Object.entries(all)) {
      if (!shortcut(id).pad?.includes(button)) continue;
      if (command.when?.() ?? true) return { id, command };
    }
    return null;
  }
}

export const registry = new Registry();

/**
 * Give a view's shortcuts their controller buttons, for as long as the
 * calling component lives. Call during component initialisation.
 */
export function padActions(view: ViewId | (() => ViewId), commands: PadCommands) {
  $effect(() => {
    const v = typeof view === "function" ? view() : view;
    registry.set(v, commands);
    return () => registry.unset(v, commands);
  });
}
