/**
 * The one door to the Rust side. Every command goes through `call`, so a
 * failure is always an `Error` whose message is the backend's own words, and
 * a missing Tauri runtime (the page opened in a browser) says so once.
 */
import { invoke, Channel } from "@tauri-apps/api/core";

export { Channel };

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw e instanceof Error ? e : new Error(typeof e === "string" ? e : JSON.stringify(e));
  }
}

/** The message of whatever was thrown, for a toast. */
export function errorText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}
