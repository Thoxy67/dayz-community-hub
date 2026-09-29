/** Controllers: what is connected, and the actions they send (see features/gamepad in Rust). */
import type { GamepadStatus, PadAction, PadInfo, PadKind } from "./bindings";
import { commands, events, run } from "./core";

export type { GamepadStatus, PadAction, PadInfo, PadKind };

export const gamepadStatus = () => run(commands.gamepadStatus());

/** A pad pressed something: `gamepad-input`. */
export const onGamepadInput = (f: (action: PadAction, repeat: boolean) => void) =>
  events.gamepadInput.listen((e) => f(e.payload.action, e.payload.repeat));

/** A pad came or went: `gamepad-pads`, with every pad connected now. */
export const onGamepadPads = (f: (pads: PadInfo[]) => void) =>
  events.gamepadPads.listen((e) => f(e.payload));
