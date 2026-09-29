/**
 * Live details about one server, fetched on demand and cached: A2S (who is
 * on, the rules, the mods it reports) and BattleMetrics (rank, uptime, a
 * day of player counts). Requests for the same server share one flight, and
 * the caches are bounded like the backend's own.
 */
import { SvelteMap } from "svelte/reactivity";
import { queryA2s, fetchBattleMetrics } from "$lib/ipc/servers";
import { errorText } from "$lib/ipc/core";
import type { A2sDetailsDto, BattleMetricsDto } from "$lib/ipc/types";
import { servers } from "./servers.svelte";

type Entry<T> = { data: T | null; loading: boolean; error: string | null; fetchedAt: number | null };

const A2S_TTL_MS = 30_000;
const BM_TTL_MS = 300_000;
const MAX_A2S = 200;
const MAX_BM = 50;

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
    const key = `${ip}:${port}:${queryPort}`;
    const cached = this.#bm.get(key);
    if (!force && cached?.data && cached.fetchedAt && Date.now() - cached.fetchedAt < BM_TTL_MS) {
      return Promise.resolve(cached.data);
    }
    const inflight = this.#bmFlight.get(key);
    if (inflight) return inflight;
    const p = (async () => {
      const prev = this.#bm.get(key) ?? blank<BattleMetricsDto>();
      this.#bm.set(key, { ...prev, loading: true, error: null });
      try {
        const data = await fetchBattleMetrics(ip, port, queryPort, name);
        this.#bm.set(key, { data, loading: false, error: null, fetchedAt: Date.now() });
        evict(this.#bm, MAX_BM);
        return data;
      } catch (e) {
        this.#bm.set(key, { ...prev, loading: false, error: errorText(e) });
        return null;
      }
    })().finally(() => this.#bmFlight.delete(key));
    this.#bmFlight.set(key, p);
    return p;
  }
}

export const serverData = new ServerData();
