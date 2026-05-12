// ─── Unified Server Data Service ────────────────────────────────────────────
// Centralizes A2S and BattleMetrics data management with caching and debouncing.

import { invoke } from '@tauri-apps/api/core';
import { app } from '$lib/state.svelte';
import { SvelteMap } from 'svelte/reactivity';
import type { A2sDetailsDto, BattleMetricsDto, ServerDto } from '$lib/types';

// ── Cache entry types ───────────────────────────────────────────────────────

interface A2sCacheEntry {
  data: A2sDetailsDto | null;
  loading: boolean;
  error: string | null;
  fetchedAt: number | null;
}

interface BmCacheEntry {
  data: BattleMetricsDto | null;
  loading: boolean;
  error: string | null;
  fetchedAt: number | null;
}

// ── Service class ───────────────────────────────────────────────────────────

class ServerDataService {
  // ── Centralized reactive state ────────────────────────────────────────────

  /** A2S data cache: key → { data, loading, error, fetchedAt } */
  // SvelteMap is reactive on .set/.delete — saves the per-mutation
  // `new Map(...)` clone we previously needed to trigger reactivity.
  private _a2sCache = new SvelteMap<string, A2sCacheEntry>();

  /** BattleMetrics cache: key → { data, loading, error, fetchedAt } */
  private _bmCache = new SvelteMap<string, BmCacheEntry>();

  /** Player counts from A2S (overrides list/ping values) */
  private _a2sPlayers = new SvelteMap<string, { players: number; maxPlayers: number }>();

  /** Keys currently refreshing (debounce) */
  private _refreshingKeys = new Set<string>();

  /** In-flight A2S requests for deduplication (concurrent callers share same promise) */
  private _inflightA2s = new Map<string, Promise<A2sDetailsDto | null>>();

  /** In-flight BattleMetrics requests for deduplication */
  private _inflightBm = new Map<string, Promise<BattleMetricsDto | null>>();

  // ── Configuration ─────────────────────────────────────────────────────────

  // Cache configuration (aligned with backend: state.rs)
  readonly A2S_TTL_MS = 30_000; // 30s, matches backend
  readonly BM_TTL_MS = 300_000; // 5min, matches backend
  readonly MAX_A2S_ENTRIES = 200; // Match backend cache size
  readonly MAX_BM_ENTRIES = 50; // Match backend cache size
  readonly A2S_REFRESH_DEBOUNCE_MS = 1000;
  readonly BM_REFRESH_DEBOUNCE_MS = 100;

  // ── Public API: A2S ───────────────────────────────────────────────────────

  /** Get cached A2S data (no fetch) */
  getA2s(ip: string, port: number): A2sCacheEntry | null {
    const key = this._resolveKey(ip, port);
    return this._a2sCache.get(key) ?? null;
  }

  /** Check if A2S data is cached and fresh */
  hasValidA2s(ip: string, port: number): boolean {
    const key = this._resolveKey(ip, port);
    const entry = this._a2sCache.get(key);
    if (!entry?.data || !entry.fetchedAt) return false;
    return Date.now() - entry.fetchedAt < this.A2S_TTL_MS;
  }

  /** Check if A2S is currently loading */
  isA2sLoading(ip: string, port: number): boolean {
    const key = this._resolveKey(ip, port);
    return this._a2sCache.get(key)?.loading ?? false;
  }

  /** Get A2S error if any */
  getA2sError(ip: string, port: number): string | null {
    const key = this._resolveKey(ip, port);
    return this._a2sCache.get(key)?.error ?? null;
  }

  /** Refresh A2S data for a server (with request deduplication) */
  async refreshA2s(ip: string, port: number): Promise<A2sDetailsDto | null> {
    const key = this._resolveKey(ip, port);

    // Return existing in-flight request (deduplication)
    const inflight = this._inflightA2s.get(key);
    if (inflight) {
      return inflight;
    }

    // Create and track new request
    const promise = this._doRefreshA2s(ip, port, key);
    this._inflightA2s.set(key, promise);

    try {
      return await promise;
    } finally {
      this._inflightA2s.delete(key);
    }
  }

  /** Internal A2S refresh implementation */
  private async _doRefreshA2s(ip: string, port: number, key: string): Promise<A2sDetailsDto | null> {
    // Set loading state
    this._setA2sState(key, { loading: true, error: null });

    try {
      // Resolve query port from server list
      const server = this._findServer(ip, port);
      const queryPort = server?.query_port ?? port;
      const gamePort = server?.game_port ?? null;

      const data = await invoke<A2sDetailsDto>('query_a2s', {
        ip,
        queryPort,
        gamePort,
      });

      this._setA2sState(key, {
        data,
        loading: false,
        error: null,
        fetchedAt: Date.now(),
      });

      // Update player cache (SvelteMap — reactive on .set).
      this._a2sPlayers.set(key, {
        players: data.players,
        maxPlayers: data.max_players,
      });

      // Clear A2S failure flag (SvelteSet — reactive on delete).
      app.a2sFailures.delete(key);

      this._evictOldEntries(this._a2sCache, this.MAX_A2S_ENTRIES);
      return data;
    } catch (e) {
      const error = String(e);
      this._setA2sState(key, { loading: false, error });

      // Set A2S failure flag (SvelteSet — reactive on add).
      app.a2sFailures.add(key);

      return null;
    }
  }

  /** Refresh only player count (calls full A2S but only returns players) */
  async refreshPlayers(ip: string, port: number): Promise<number | null> {
    const data = await this.refreshA2s(ip, port);
    return data?.players ?? null;
  }

  /** Get best available player count for a server */
  getPlayers(ip: string, port: number): { players: number; maxPlayers: number; source: 'a2s' | 'ping' | 'list' } {
    const key = this._resolveKey(ip, port);

    // Priority 1: A2S player data
    const a2sPlayerData = this._a2sPlayers.get(key);
    if (a2sPlayerData) {
      return { ...a2sPlayerData, source: 'a2s' };
    }

    // Priority 2: A2S cache data
    const a2sEntry = this._a2sCache.get(key);
    if (a2sEntry?.data) {
      return {
        players: a2sEntry.data.players,
        maxPlayers: a2sEntry.data.max_players,
        source: 'a2s',
      };
    }

    // Priority 3: Server list data
    const server = this._findServer(ip, port);
    return {
      players: server?.players ?? 0,
      maxPlayers: server?.max_players ?? 0,
      source: 'list',
    };
  }

  // ── Public API: BattleMetrics ─────────────────────────────────────────────

  /** Get cached BattleMetrics data (no fetch) */
  getBm(ip: string, port: number, queryPort: number): BmCacheEntry | null {
    const key = `${ip}:${port}:${queryPort}`;
    return this._bmCache.get(key) ?? null;
  }

  /** Check if BattleMetrics data is cached and fresh */
  hasValidBm(ip: string, port: number, queryPort: number): boolean {
    const key = `${ip}:${port}:${queryPort}`;
    const entry = this._bmCache.get(key);
    if (!entry?.data || !entry.fetchedAt) return false;
    return Date.now() - entry.fetchedAt < this.BM_TTL_MS;
  }

  /** Check if BattleMetrics is currently loading */
  isBmLoading(ip: string, port: number, queryPort: number): boolean {
    const key = `${ip}:${port}:${queryPort}`;
    return this._bmCache.get(key)?.loading ?? false;
  }

  /** Get BattleMetrics error if any */
  getBmError(ip: string, port: number, queryPort: number): string | null {
    const key = `${ip}:${port}:${queryPort}`;
    return this._bmCache.get(key)?.error ?? null;
  }

  /** Fetch BattleMetrics data (with cache check and request deduplication) */
  async fetchBattleMetrics(
    ip: string,
    port: number,
    queryPort: number,
    name: string,
  ): Promise<BattleMetricsDto | null> {
    const key = `${ip}:${port}:${queryPort}`;

    // Check cache freshness
    const cached = this._bmCache.get(key);
    if (cached?.data && cached.fetchedAt && Date.now() - cached.fetchedAt < this.BM_TTL_MS) {
      return cached.data;
    }

    // Return existing in-flight request (deduplication)
    const inflight = this._inflightBm.get(key);
    if (inflight) {
      return inflight;
    }

    // Create and track new request
    const promise = this._doFetchBattleMetrics(ip, port, queryPort, name, key);
    this._inflightBm.set(key, promise);

    try {
      return await promise;
    } finally {
      this._inflightBm.delete(key);
    }
  }

  /** Internal BattleMetrics fetch implementation */
  private async _doFetchBattleMetrics(
    ip: string,
    port: number,
    queryPort: number,
    name: string,
    key: string,
  ): Promise<BattleMetricsDto | null> {
    this._setBmState(key, { loading: true, error: null });

    try {
      const data = await invoke<BattleMetricsDto>('fetch_battlemetrics_server', { ip, port, queryPort, name });
      this._setBmState(key, { data, loading: false, error: null, fetchedAt: Date.now() });
      this._evictOldEntries(this._bmCache, this.MAX_BM_ENTRIES);
      return data;
    } catch (e) {
      this._setBmState(key, { loading: false, error: String(e) });
      return null;
    }
  }

  /** Force refresh BattleMetrics (ignore cache) */
  async refreshBattleMetrics(
    ip: string,
    port: number,
    queryPort: number,
    name: string,
  ): Promise<BattleMetricsDto | null> {
    const key = `${ip}:${port}:${queryPort}`;
    // Clear cached fetchedAt to force refresh
    const existing = this._bmCache.get(key);
    if (existing) {
      this._bmCache.set(key, { ...existing, fetchedAt: null });
    }
    return this.fetchBattleMetrics(ip, port, queryPort, name);
  }

  // ── Public API: Utilities ─────────────────────────────────────────────────

  /** Generate canonical cache key for a server */
  serverKey(ip: string, port: number): string {
    return this._resolveKey(ip, port);
  }

  /** Clear all caches */
  clearCaches(): void {
    this._a2sCache.clear();
    this._bmCache.clear();
    this._a2sPlayers.clear();
    this._refreshingKeys.clear();
  }

  /** Clear cache for a specific server (SvelteMap.delete is reactive). */
  clearServerCache(ip: string, port: number): void {
    const key = this._resolveKey(ip, port);
    this._a2sCache.delete(key);
    this._a2sPlayers.delete(key);
    this._bmCache.delete(`${ip}:${port}`);
  }

  // ── Private helpers ───────────────────────────────────────────────────────

  private _findServer(ip: string, port: number): ServerDto | null {
    return app.servers.find((s) => s.ip === ip && (s.query_port === port || s.game_port === port)) ?? null;
  }

  private _resolveKey(ip: string, port: number): string {
    const server = this._findServer(ip, port);
    return `${ip}:${server?.query_port ?? port}`;
  }

  private _setA2sState(key: string, updates: Partial<A2sCacheEntry>): void {
    const existing = this._a2sCache.get(key) ?? {
      data: null,
      loading: false,
      error: null,
      fetchedAt: null,
    };
    // SvelteMap — .set already triggers reactivity.
    this._a2sCache.set(key, { ...existing, ...updates });
  }

  private _setBmState(key: string, updates: Partial<BmCacheEntry>): void {
    const existing = this._bmCache.get(key) ?? {
      data: null,
      loading: false,
      error: null,
      fetchedAt: null,
    };
    this._bmCache.set(key, { ...existing, ...updates });
  }

  private _evictOldEntries<T extends { fetchedAt: number | null }>(cache: Map<string, T>, maxEntries: number): void {
    if (cache.size <= maxEntries) return;

    // Sort by fetchedAt, evict oldest
    const entries = [...cache.entries()].sort((a, b) => (a[1].fetchedAt ?? 0) - (b[1].fetchedAt ?? 0));

    const toRemove = entries.slice(0, cache.size - maxEntries);
    for (const [key] of toRemove) {
      cache.delete(key);
    }
  }
}

// ── Singleton export ────────────────────────────────────────────────────────

export const serverData = new ServerDataService();
