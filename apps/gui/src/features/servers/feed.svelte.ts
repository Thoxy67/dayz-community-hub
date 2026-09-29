/**
 * The rows behind the browser's one long list. The list is as tall as every
 * matching server, but only the blocks around what is on screen are asked
 * for, a hundred rows at a time; what has not arrived yet is drawn as a
 * placeholder. A new search or order starts over; a change in the backend's
 * data (pings, counts) re-asks for the blocks in view and swaps them in place,
 * so nothing flickers while a scan runs.
 */
import { SvelteMap } from "svelte/reactivity";
import { serversQuery, type ServerQuery, type ServerRow, type ServerStats } from "$lib/ipc/servers";
import { servers } from "$lib/stores/servers.svelte";

const BLOCK = 100;

class Feed {
  rows = new SvelteMap<number, ServerRow>();
  total = $state(0);
  stats = $state<ServerStats | null>(null);
  /** True until the first block of a new request has arrived. */
  loading = $state(true);

  #request = "";
  #req: Omit<ServerQuery, "offset" | "limit"> | null = null;
  #epoch = 0;
  #inflight = new Set<number>();
  #fetched = new Map<number, number>(); // block → generation it was fetched at
  #view: [number, number] = [0, 0];

  /** A new search, filter or order: start over. */
  setRequest(req: Omit<ServerQuery, "offset" | "limit">) {
    const key = JSON.stringify(req);
    if (key === this.#request) return;
    this.#request = key;
    this.#req = req;
    this.#epoch++;
    this.#inflight.clear();
    this.#fetched.clear();
    this.loading = true;
    // The list goes back to the top; old rows stay until the first block
    // replaces them, so there is no blank frame.
    this.#view = [0, this.#view[1] - this.#view[0]];
    void this.#fetch(0, true);
  }

  /** The rows on screen, `first` to `last` (exclusive): fetch what is missing or stale. */
  show(first: number, last: number) {
    this.#view = [first, last];
    const from = Math.max(0, Math.floor(first / BLOCK) - 1);
    const to = Math.floor(Math.max(first, last - 1) / BLOCK) + 1;
    for (let b = from; b <= to; b++) {
      if (b * BLOCK >= Math.max(this.total, BLOCK)) break;
      const at = this.#fetched.get(b);
      if (at === undefined || at < servers.generation) void this.#fetch(b);
    }
  }

  /** The backend's data changed: re-ask for what is on screen. */
  refresh() {
    this.show(...this.#view);
  }

  async #fetch(block: number, reset = false) {
    const req = this.#req;
    if (!req || this.#inflight.has(block)) return;
    this.#inflight.add(block);
    const epoch = this.#epoch;
    const generation = servers.generation;
    try {
      const page = await serversQuery({ ...req, offset: block * BLOCK, limit: BLOCK });
      if (epoch !== this.#epoch) return;
      if (reset) this.rows.clear();
      this.total = page.total;
      this.stats = page.stats;
      page.rows.forEach((r, i) => this.rows.set(block * BLOCK + i, r));
      servers.remember(page.rows);
      this.#fetched.set(block, generation);
      this.loading = false;
      // Rows past the new end of a shrinking list are dropped.
      if (reset) for (const k of this.rows.keys()) if (k >= page.total) this.rows.delete(k);
    } catch {
      if (epoch === this.#epoch) this.loading = false;
    } finally {
      this.#inflight.delete(block);
    }
  }
}

export const feed = new Feed();
