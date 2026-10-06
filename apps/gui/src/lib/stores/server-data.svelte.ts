/**
 * Live details about one server, fetched on demand and cached: A2S (who is
 * on, the rules, the mods it reports), DayZ Metrics (rank, schedules, fake
 * verdict, player counts; no key). Requests for the same server share one
 * flight, and the caches are bounded like the backend's own.
 */
import { SvelteMap } from "svelte/reactivity";
import {
  queryA2s,
  fetchServerMetrics,
  fetchMetricsHistory,
  fetchMetricsRankHistory,
  fetchMetricsHeatmap,
  type MetricsRange,
} from "$lib/ipc/servers";
import { errorText } from "$lib/ipc/core";
import type { A2sDetailsDto, HeatCell, ServerMetrics } from "$lib/ipc/types";
import { servers } from "./servers.svelte";

type Entry<T> = {
  data: T | null;
  loading: boolean;
  error: string | null;
  fetchedAt: number | null;
};

const A2S_TTL_MS = 30_000;
const MAX_A2S = 200;
const METRICS_TTL_MS = 300_000;
const MAX_METRICS = 100;
/** The longer views change slowly: a week's history, a month's rank, the busy hours. */
const LONG_TTL_MS = 900_000;
const MAX_LONG = 30;

const blank = <T>(): Entry<T> => ({ data: null, loading: false, error: null, fetchedAt: null });

function evict<T, K>(cache: Map<K, Entry<T>>, max: number) {
  if (cache.size <= max) return;
  const oldest = [...cache.entries()]
    .filter(([, e]) => !e.loading)
    .sort((a, b) => (a[1].fetchedAt ?? 0) - (b[1].fetchedAt ?? 0))
    .slice(0, cache.size - max);
  for (const [k] of oldest) cache.delete(k);
}

class ServerData {
  #a2s = new SvelteMap<string, Entry<A2sDetailsDto>>();
  #a2sFlight = new Map<string, Promise<A2sDetailsDto | null>>();
  #metrics = new SvelteMap<string, Entry<ServerMetrics>>();
  #metricsFlight = new Map<string, Promise<ServerMetrics | null>>();
  #history = new SvelteMap<string, Entry<[number, number][]>>();
  #historyFlight = new Map<string, Promise<[number, number][] | null>>();
  #rank = new SvelteMap<number, Entry<[string, number][]>>();
  #rankFlight = new Map<number, Promise<[string, number][] | null>>();
  #heat = new SvelteMap<number, Entry<HeatCell[]>>();
  #heatFlight = new Map<number, Promise<HeatCell[] | null>>();

  /**
   * A cached long-view lookup: the cached value while fresh (unless `force`),
   * one shared flight per key, the error kept beside the last good value.
   */
  #cached<T, K extends string | number = string>(
    cache: SvelteMap<K, Entry<T>>,
    flights: Map<K, Promise<T | null>>,
    key: K,
    ttl: number,
    max: number,
    force: boolean,
    fetch: () => Promise<T>,
  ): Promise<T | null> {
    const cached = cache.get(key);
    if (!force && cached?.data && cached.fetchedAt && Date.now() - cached.fetchedAt < ttl) {
      return Promise.resolve(cached.data);
    }
    const inflight = flights.get(key);
    if (inflight) return inflight;
    const p = (async () => {
      const prev = cache.get(key) ?? blank<T>();
      cache.set(key, { ...prev, loading: true, error: null });
      try {
        const data = await fetch();
        cache.set(key, { data, loading: false, error: null, fetchedAt: Date.now() });
        evict(cache, max);
        return data;
      } catch (e) {
        cache.set(key, { ...prev, loading: false, error: errorText(e) });
        return null;
      }
    })().finally(() => flights.delete(key));
    flights.set(key, p);
    return p;
  }

  /** The key a server's A2S data is kept under: its query port when known. */
  key(ip: string, port: number): string {
    return `${ip}:${servers.find(ip, port)?.query_port ?? port}`;
  }

  a2s(ip: string, port: number): Entry<A2sDetailsDto> {
    return this.#a2s.get(this.key(ip, port)) ?? blank();
  }

  hasFreshA2s(ip: string, port: number): boolean {
    const e = this.#a2s.get(this.key(ip, port));
    return !!e?.data && !!e.fetchedAt && Date.now() - e.fetchedAt < A2S_TTL_MS;
  }

  /**
   * Ask the server itself. Concurrent callers share one query. The address
   * may carry the game port (a favourite, a history entry): the row is
   * resolved first, so the query goes to the query port and the answer is
   * kept under the key `a2s()` reads once the row is known.
   */
  refreshA2s(ip: string, port: number): Promise<A2sDetailsDto | null> {
    const asked = this.key(ip, port);
    const inflight = this.#a2sFlight.get(asked);
    if (inflight) return inflight;
    const p = (async () => {
      const shown = this.#a2s.get(asked) ?? blank<A2sDetailsDto>();
      this.#a2s.set(asked, { ...shown, loading: true, error: null });
      const sv = servers.find(ip, port) ?? (await servers.resolve(ip, port));
      const key = `${ip}:${sv?.query_port ?? port}`;
      if (key !== asked) this.#a2s.set(asked, { ...shown, loading: false });
      return this.#queryA2s(ip, sv?.query_port ?? port, sv?.game_port ?? null, key);
    })().finally(() => this.#a2sFlight.delete(asked));
    this.#a2sFlight.set(asked, p);
    return p;
  }

  async #queryA2s(ip: string, queryPort: number, gamePort: number | null, key: string) {
    const prev = this.#a2s.get(key) ?? blank<A2sDetailsDto>();
    this.#a2s.set(key, { ...prev, loading: true, error: null });
    try {
      const data = await queryA2s(ip, queryPort, gamePort);
      this.#a2s.set(key, { data, loading: false, error: null, fetchedAt: Date.now() });
      servers.applyLiveCount(key, data.players, data.max_players, data.bots);
      servers.a2sFailures.delete(key);
      evict(this.#a2s, MAX_A2S);
      return data;
    } catch (e) {
      this.#a2s.set(key, { ...prev, loading: false, error: errorText(e) });
      servers.a2sFailures.add(key);
      return null;
    }
  }

  /** Players right now: from A2S when it has answered, else the list. */
  players(ip: string, port: number): { players: number; max: number; bots: number } {
    const d = this.#a2s.get(this.key(ip, port))?.data;
    if (d) return { players: d.players, max: d.max_players, bots: d.bots };
    const sv = servers.find(ip, port);
    return { players: sv?.players ?? 0, max: sv?.max_players ?? 0, bots: sv?.bots ?? 0 };
  }

  metrics(ip: string, gamePort: number, queryPort: number): Entry<ServerMetrics> {
    return this.#metrics.get(`${ip}:${gamePort}:${queryPort}`) ?? blank();
  }

  /** DayZ Metrics, from the cache while it is fresh unless `force`. */
  /** `name` is how the list calls the server: the site finds it fastest by name. */
  fetchMetrics(ip: string, gamePort: number, queryPort: number, name: string, force = false) {
    return this.#cached(
      this.#metrics,
      this.#metricsFlight,
      `${ip}:${gamePort}:${queryPort}`,
      METRICS_TTL_MS,
      MAX_METRICS,
      force,
      () => fetchServerMetrics(ip, gamePort, queryPort, name),
    );
  }

  // ── the longer views, by the site's id (from `metrics().data.id`) ────────
  // Specta writes f64 as `number | null`: points without a value are dropped.

  history(id: number, range: MetricsRange): Entry<[number, number][]> {
    return this.#history.get(`${id}:${range}`) ?? blank();
  }

  fetchHistory(id: number, range: MetricsRange) {
    return this.#cached(
      this.#history,
      this.#historyFlight,
      `${id}:${range}`,
      LONG_TTL_MS,
      MAX_LONG,
      false,
      async () =>
        (await fetchMetricsHistory(id, range)).filter((p): p is [number, number] => p[1] != null),
    );
  }

  rankHistory(id: number): Entry<[string, number][]> {
    return this.#rank.get(id) ?? blank();
  }

  fetchRankHistory(id: number) {
    return this.#cached(this.#rank, this.#rankFlight, id, LONG_TTL_MS, MAX_LONG, false, async () =>
      (await fetchMetricsRankHistory(id)).filter((p): p is [string, number] => p[1] != null),
    );
  }

  heatmap(id: number): Entry<HeatCell[]> {
    return this.#heat.get(id) ?? blank();
  }

  fetchHeatmap(id: number) {
    return this.#cached(this.#heat, this.#heatFlight, id, LONG_TTL_MS, MAX_LONG, false, () =>
      fetchMetricsHeatmap(id),
    );
  }
}

export const serverData = new ServerData();
