/**
 * Whether a controller or the pointer is driving the window, the pads
 * connected, and what is focused while a pad drives.
 *
 * `mode` follows the last input used ("auto"), or is held by the setting.
 * In gamepad mode the root carries `data-input="gamepad"`: the focus ring
 * shows, the hint bar appears, and the interface is drawn a little larger.
 */
import type { PadAction, PadInfo, PadKind } from "$lib/ipc/gamepad";
import { prefs } from "$lib/stores/prefs.svelte";

export type InputMode = "pointer" | "gamepad";
/** Which maker's labels to draw: the pad's own, or one picked in the settings. */
export type GlyphChoice = "auto" | Exclude<PadKind, "generic">;

const GLYPHS_KEY = "dzch.padGlyphs";
const GLYPHS: readonly GlyphChoice[] = ["auto", "xbox", "playStation", "nintendo", "steamDeck"];
function savedGlyphs(): GlyphChoice {
  try {
    const v = localStorage.getItem(GLYPHS_KEY) as GlyphChoice | null;
    return v && GLYPHS.includes(v) ? v : "auto";
  } catch {
    return "auto";
  }
}

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
  /** The labels chosen in the settings; "auto" follows the pad. */
  #glyphs = $state<GlyphChoice>(savedGlyphs());
  /**
   * Glyphs to draw: the setting's, else the first pad's. Steam Input hands
   * the app a generic virtual pad; inside Steam's big-screen interface that
   * is most likely a Deck, whose shoulders read L1/R1.
   */
  kind: PadKind = $derived.by(() => {
    if (this.#glyphs !== "auto") return this.#glyphs;
    const k = this.pads[0]?.kind ?? "generic";
    if (k !== "generic") return k;
    return this.steamUi ? "steamDeck" : "xbox";
  });
  /** The last action a pad sent and when, for the settings' live test. */
  last = $state.raw<{ action: PadAction; at: number } | null>(null);

  get glyphs() {
    return this.#glyphs;
  }
  set glyphs(v: GlyphChoice) {
    this.#glyphs = v;
    try {
      localStorage.setItem(GLYPHS_KEY, v);
    } catch {
      // Kept for this session.
    }
  }
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
