/**
 * Live details about one server, fetched on demand and cached: A2S (who is
 * on, the rules, the mods it reports), DayZ Metrics (rank, schedules, fake
 * verdict, a day of player counts; no key) and BattleMetrics (only with a
 * paid token). Requests for the same server share one flight, and the caches
 * are bounded like the backend's own.
 */
import { SvelteMap } from "svelte/reactivity";
import { queryA2s, fetchBattleMetrics, fetchServerMetrics } from "$lib/ipc/servers";
import { errorText } from "$lib/ipc/core";
import type { A2sDetailsDto, BattleMetricsDto, ServerMetrics } from "$lib/ipc/types";
import { servers } from "./servers.svelte";

type Entry<T> = { data: T | null; loading: boolean; error: string | null; fetchedAt: number | null };

const A2S_TTL_MS = 30_000;
const BM_TTL_MS = 300_000;
const MAX_A2S = 200;
const MAX_BM = 50;
const METRICS_TTL_MS = 300_000;
const MAX_METRICS = 100;

const blank = <T>(): Entry<T> => ({ data: null, loading: false, error: null, fetchedAt: null });

function evict<T>(cache: Map<string, Entry<T>>, max: number) {
  if (cache.size <= max) return;
  const oldest = [...cache.entries()]
    .sort((a, b) => (a[1].fetchedAt ?? 0) - (b[1].fetchedAt ?? 0))
    .slice(0, cache.size - max);
  for (const [k] of oldest) cache.delete(k);
}

class ServerData {
  #a2s = new SvelteMap<string, Entry<A2sDetailsDto>>();
  #bm = new SvelteMap<string, Entry<BattleMetricsDto>>();
  #a2sFlight = new Map<string, Promise<A2sDetailsDto | null>>();
  #bmFlight = new Map<string, Promise<BattleMetricsDto | null>>();
  #metrics = new SvelteMap<string, Entry<ServerMetrics>>();
  #metricsFlight = new Map<string, Promise<ServerMetrics | null>>();

  /**
   * A cached long-view lookup: the cached value while fresh (unless `force`),
   * one shared flight per key, the error kept beside the last good value.
   */
  #cached<T>(
    cache: SvelteMap<string, Entry<T>>,
    flights: Map<string, Promise<T | null>>,
    key: string,
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

  /** Ask the server itself. Concurrent callers share one query. */
  refreshA2s(ip: string, port: number): Promise<A2sDetailsDto | null> {
    const key = this.key(ip, port);
    const inflight = this.#a2sFlight.get(key);
    if (inflight) return inflight;
    const p = this.#queryA2s(ip, port, key).finally(() => this.#a2sFlight.delete(key));
    this.#a2sFlight.set(key, p);
    return p;
  }

  async #queryA2s(ip: string, port: number, key: string) {
    const prev = this.#a2s.get(key) ?? blank<A2sDetailsDto>();
    this.#a2s.set(key, { ...prev, loading: true, error: null });
    const sv = servers.find(ip, port);
    try {
      const data = await queryA2s(ip, sv?.query_port ?? port, sv?.game_port ?? null);
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

  bm(ip: string, port: number, queryPort: number): Entry<BattleMetricsDto> {
    return this.#bm.get(`${ip}:${port}:${queryPort}`) ?? blank();
  }

  /** BattleMetrics, from the cache while it is fresh unless `force`. */
  fetchBm(ip: string, port: number, queryPort: number, name: string, force = false) {
    return this.#cached(this.#bm, this.#bmFlight, `${ip}:${port}:${queryPort}`, BM_TTL_MS, MAX_BM, force, () =>
      fetchBattleMetrics(ip, port, queryPort, name),
    );
  }

  metrics(ip: string, gamePort: number, queryPort: number): Entry<ServerMetrics> {
    return this.#metrics.get(`${ip}:${gamePort}:${queryPort}`) ?? blank();
  }

  /** DayZ Metrics, from the cache while it is fresh unless `force`. */
  fetchMetrics(ip: string, gamePort: number, queryPort: number, force = false) {
    return this.#cached(
      this.#metrics,
      this.#metricsFlight,
      `${ip}:${gamePort}:${queryPort}`,
      METRICS_TTL_MS,
      MAX_METRICS,
      force,
      () => fetchServerMetrics(ip, gamePort, queryPort),
    );
  }
}

export const serverData = new ServerData();
