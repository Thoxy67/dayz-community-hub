/**
 * The stats view's state: the figures for the chosen period and the full
 * history, a page at a time. Both are worked out by the backend
 * (`play_stats`, `play_sessions`); they are asked again when a session
 * starts, ends or runs on (`play-sessions-changed`).
 */
import * as ipc from "$lib/ipc/stats";
import { events, inTauri, errorText } from "$lib/ipc/core";
import type { PlayStatsDto, SessionDto, StatsRange } from "$lib/ipc/types";
import { say } from "$lib/stores/say";

const PAGE = 100;

class Stats {
  range = $state<StatsRange>("all");
  data = $state.raw<PlayStatsDto | null>(null);
  loading = $state(false);
  error = $state<string | null>(null);

  search = $state("");
  rows = $state.raw<SessionDto[]>([]);
  total = $state(0);

  #listening = false;
  #searchTimer: ReturnType<typeof setTimeout> | undefined;

  /** Once: ask again whenever the sessions change. */
  listen() {
    if (this.#listening || !inTauri) return;
    this.#listening = true;
    void events.playSessionsChanged.listen(() => {
      void this.load();
      void this.loadHistory(false);
    });
  }

  async load() {
    this.loading = this.data === null;
    try {
      this.data = await ipc.playStats(this.range);
      this.error = null;
    } catch (e) {
      this.error = errorText(e);
    } finally {
      this.loading = false;
    }
  }

  setRange(r: StatsRange) {
    this.range = r;
    void this.load();
  }

  /** The first page again (`more` false), or the next one. */
  async loadHistory(more: boolean) {
    try {
      const page = await ipc.playSessions(this.search, more ? this.rows.length : 0, PAGE);
      this.rows = more ? [...this.rows, ...page.rows] : page.rows;
      this.total = page.total;
    } catch (e) {
      say.err(errorText(e));
    }
  }

  setSearch(v: string) {
    this.search = v;
    clearTimeout(this.#searchTimer);
    this.#searchTimer = setTimeout(() => void this.loadHistory(false), 200);
  }

  get nextPage() {
    return Math.min(PAGE, this.total - this.rows.length);
  }

  async forget(s: SessionDto) {
    try {
      await ipc.deleteSession(s.start, s.name);
      // Outside Tauri no event follows.
      if (!inTauri) this.rows = this.rows.filter((r) => r !== s);
    } catch (e) {
      say.err(errorText(e));
    }
  }
}

export const stats = new Stats();
