/**
 * Whether a controller or the pointer is driving the window, the pads
 * connected, and what is focused while a pad drives.
 *
 * `mode` follows the last input used ("auto"), or is held by the setting.
 * In gamepad mode the root carries `data-input="gamepad"`: the focus ring
 * shows, the hint bar appears, and the interface is drawn a little larger.
 */
import type { PadInfo, PadKind } from "$lib/ipc/gamepad";
import { prefs } from "$lib/stores/prefs.svelte";

export type InputMode = "pointer" | "gamepad";

class PadState {
  /** The last kind of input used, for "auto". */
  #last = $state<InputMode>("pointer");
  /** Pads could be opened at all. */
  available = $state(false);
  pads = $state.raw<PadInfo[]>([]);
  /** Inside Steam's big-screen interface: its on-screen keyboard is there. */
  steamUi = $state(false);
  /** The focused element while a pad drives, for hints that depend on it. */
  focused = $state.raw<Element | null>(null);

  mode: InputMode = $derived(
    prefs.padMode === "always" ? "gamepad" : prefs.padMode === "never" ? "pointer" : this.#last,
  );
  /** Glyphs to draw: the first pad's, Xbox's letters when unknown. */
  kind: PadKind = $derived(this.pads[0]?.kind ?? "xbox");
  /** Pad input is acted on at all. */
  get enabled() {
    return prefs.padMode !== "never";
  }

  /** A pad did something. */
  usedPad() {
    this.#last = "gamepad";
  }
  /** The mouse did something. */
  usedPointer() {
    this.#last = "pointer";
  }
}

export const pad = new PadState();
