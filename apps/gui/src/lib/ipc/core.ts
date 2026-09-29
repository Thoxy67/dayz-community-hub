/**
 * The one door to the Rust side. Every command is the generated
 * `commands.*` from `bindings.ts` (so a wrong name or argument is a type
 * error), passed through `run`, so a failure is always an `Error` whose
 * message is the backend's own words.
 */
import { Channel } from "@tauri-apps/api/core";

export { Channel };
export { commands, events } from "./bindings";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Await a command; whatever it rejects with becomes an `Error`. */
export async function run<T>(p: Promise<T>): Promise<T> {
  try {
    return await p;
  } catch (e) {
    throw e instanceof Error ? e : new Error(typeof e === "string" ? e : JSON.stringify(e));
  }
}

/** The message of whatever was thrown, for a toast. */
export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
