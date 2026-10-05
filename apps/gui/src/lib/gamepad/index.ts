/**
 * Controller navigation: the pads (read in Rust), the spatial focus moves
 * they drive, the lists they walk row by row, and the hints. `start` is
 * called once from the root; views use `padActions` and `padList`.
 */
import { gamepadStatus, onGamepadInput, onGamepadPads } from "$lib/ipc/gamepad";
import type { ViewId } from "$lib/stores/app.svelte";
import { handle, install } from "./nav";
import { pad } from "./state.svelte";

export { pad } from "./state.svelte";
export { padActions, type PadCommand, type PadCommands } from "./actions.svelte";
export { padList, indexOf, type PadList } from "./list";
export { handle, dialogConfirm } from "./nav";

/** Start listening to the pads. Returns the teardown. */
export function start(views: readonly ViewId[]): () => void {
  const uninstall = install(views);
  let disposed = false;
  const off: Array<() => void> = [];
  const keep = (p: Promise<() => void>) =>
    void p.then((u) => (disposed ? u() : off.push(u))).catch(() => {});

  void gamepadStatus()
    .then((s) => {
      if (!s) return;
      pad.available = s.available;
      pad.pads = s.pads;
      pad.steamUi = s.steam_ui;
      // Game Mode on a Steam Deck: there is no mouse to start with.
      if (s.steam_ui) pad.usedPad();
    })
    .catch(() => {});
  keep(onGamepadInput((action, repeat) => handle(action, repeat)));
  keep(
    onGamepadPads((pads) => {
      pad.available = true;
      pad.pads = pads;
    }),
  );

  return () => {
    disposed = true;
    off.forEach((u) => u());
    uninstall();
  };
}
