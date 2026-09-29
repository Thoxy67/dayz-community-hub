/**
 * What X, Y, Menu and View do, which depends on the view. A view registers
 * its own with `padActions(view, {...})` during its initialisation (removed
 * when it is destroyed); what it does not register falls back to the app's:
 * Menu opens the settings, View goes to the search field.
 *
 * `when` says whether an action applies now (Y "favourite" needs a server
 * under the focus); the hint bar shows only those that do. It may read
 * `pad.focused`, which changes as the focus moves.
 */
import { untrack } from "svelte";
import type { ViewId } from "$lib/stores/app.svelte";

export type PadCommand = {
  /** Shown in the hint bar beside the button's glyph. */
  label: () => string;
  run: () => void;
  when?: () => boolean;
};

export type PadCommands = Partial<Record<"primary" | "secondary" | "menu" | "view", PadCommand>>;

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
  /** The command a button runs in a view, if it applies now. */
  get(view: ViewId, button: keyof PadCommands): PadCommand | null {
    const c = this.#by[view]?.[button];
    return c && (c.when?.() ?? true) ? c : null;
  }
}

export const registry = new Registry();

/**
 * Give X, Y, Menu or View a meaning in a view, for as long as the calling
 * component lives. Call during component initialisation.
 */
export function padActions(view: ViewId | (() => ViewId), commands: PadCommands) {
  $effect(() => {
    const v = typeof view === "function" ? view() : view;
    registry.set(v, commands);
    return () => registry.unset(v, commands);
  });
}
