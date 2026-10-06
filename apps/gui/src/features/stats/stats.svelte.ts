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
  /** The day the history is narrowed to (days since 1970-01-01, local), if any. */
  day = $state<number | null>(null);
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
      // The day, from local midnight to the next, in Unix seconds.
      const from =
        this.day === null ? null : this.day * 86_400 + new Date().getTimezoneOffset() * 60;
      const page = await ipc.playSessions(
        this.search,
        from,
        from === null ? null : from + 86_400,
        more ? this.rows.length : 0,
        PAGE,
      );
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

  /** Narrow the history to one day (`null`: every day). */
  setDay(day: number | null) {
    this.day = day;
    void this.loadHistory(false);
  }

  /** Narrow the history to what matches `text` at once (a server, a map). */
  pickSearch(text: string) {
    clearTimeout(this.#searchTimer);
    this.search = text;
    void this.loadHistory(false);
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
