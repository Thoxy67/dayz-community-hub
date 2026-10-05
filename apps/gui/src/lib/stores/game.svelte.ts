/**
 * Whether DayZ is running, for the rail's "close the game" block. Asked at
 * start, then every few seconds from a launch until the game has come and
 * gone: a process scan is cheap, but not worth running all day.
 */
import { gameRunning, killGame } from "$lib/ipc/launch";

/** How long after a launch the game may take to appear. */
const START_WINDOW_MS = 3 * 60_000;
const EVERY_MS = 5_000;

class Game {
  running = $state(false);
  closing = $state(false);
  #timer: ReturnType<typeof setTimeout> | undefined;
  #watchUntil = 0;

  async #check() {
    try {
      this.running = await gameRunning();
    } catch {
      this.running = false;
    }
    // Keep looking while it runs, or while it may still be starting.
    if (this.running || Date.now() < this.#watchUntil) this.#schedule();
  }

  #schedule() {
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => void this.#check(), EVERY_MS);
  }

  /** At start: the game may already be open. */
  init() {
    void this.#check();
  }

  /** A launch was handed to Steam: watch for the game. */
  launched() {
    this.#watchUntil = Date.now() + START_WINDOW_MS;
    this.#schedule();
  }

  async close() {
    this.closing = true;
    try {
      await killGame();
    } finally {
      this.closing = false;
      await this.#check();
    }
  }
}

export const game = new Game();
