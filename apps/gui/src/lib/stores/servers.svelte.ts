/**
 * The server list and everything live about it: pings, player counts, the
 * background scan.
 *
 * The list is `$state.raw`: eighteen thousand servers made deeply reactive
 * were eighteen thousand proxies, and every ping result used to write into
 * one of them. What changes after loading (a ping, a player count, whether a
 * query failed) lives in its own reactive maps keyed by "ip:query_port", so
 * a result touches one entry and the rows that read it.
 */
import { SvelteMap, SvelteSet } from "svelte/reactivity";
import * as ipc from "$lib/ipc/servers";
import { Channel } from "$lib/ipc/core";
import type { AppStatsDto, PingResult, ServerDto } from "$lib/ipc/types";
import { words } from "$lib/i18n";
import { profile } from "./profile.svelte";
import { say, errorText } from "./say";

/** A server's key everywhere in the app. */
export const keyOf = (s: Pick<ServerDto, "ip" | "query_port">) => `${s.ip}:${s.query_port}`;

export type Live = { players: number; max: number; bots: number };

/** Data older than this earns a warning and a refresh button. */
export const STALE_MS = 10 * 60 * 1000;

class Servers {
  list = $state.raw<ServerDto[]>([]);
  loading = $state(false);
  refreshing = $state(false);
  lastRefreshed = $state(0);
  stats = $state<AppStatsDto | null>(null);
  steamPlayers = $state<number | null>(null);

  /** Round trip per server, in ms (≥ 5000 is a timeout). */
  ping = new SvelteMap<string, number>();
  pending = new SvelteSet<string>();
  /** Consecutive timeouts, so a dead server stops being asked on scroll. */
  timeouts = new SvelteMap<string, number>();
  /** Servers whose last query failed: a "full" there is unconfirmed. */
  a2sFailures = new SvelteSet<string>();
  /** Player counts from pings and queries, over what the list said. */
  live = new SvelteMap<string, Live>();

  scan = $state<{ total: number; done: number } | null>(null);
  scanPaused = $state(false);

  // Raw state: replaced whole when the list is, so `find` is reactive to that.
  #byKey = $state.raw(new Map<string, ServerDto>());
  #batch: PingResult[] = [];
  #raf: number | null = null;

  #set(list: ServerDto[]) {
    const m = new Map<string, ServerDto>();
    for (const s of list) {
      m.set(`${s.ip}:${s.query_port}`, s);
      if (!m.has(`${s.ip}:${s.game_port}`)) m.set(`${s.ip}:${s.game_port}`, s);
    }
    this.#byKey = m;
    this.list = list;
    this.lastRefreshed = Date.now();
    this.#queueFullChecks();
  }

  /** A server by address, whether `port` is its query port or its game port. */
  find(ip: string, port: number): ServerDto | undefined {
    return this.#byKey.get(`${ip}:${port}`);
  }

  /** Current players of a listed server. */
  count(s: ServerDto): Live {
    return this.live.get(keyOf(s)) ?? { players: s.players, max: s.max_players, bots: s.bots ?? 0 };
  }

  applyLiveCount(key: string, players: number, max: number, bots = 0) {
    this.live.set(key, { players, max, bots });
  }

  // ── loading ─────────────────────────────────────────────────────────────
  async load() {
    this.loading = true;
    try {
      this.#set(await ipc.getServers());
    } catch (e) {
      say.err(words("servers").loadFailed({ error: errorText(e) }));
    } finally {
      this.loading = false;
    }
  }

  /** Fetch a fresh list. `quiet` for the background refresh after a cached start. */
  async refresh(quiet = false) {
    if (this.refreshing) return;
    await this.cancelScan();
    this.refreshing = true;
    if (!quiet) {
      this.loading = true;
      say.info(words("servers").refreshing);
    }
    try {
      this.#set(await ipc.refreshServers());
      this.live.clear();
      if (!quiet) say.ok(words("servers").loaded({ count: this.list.length }));
      void this.loadStats();
      void this.loadSteamPlayers();
      void this.startScan();
    } catch (e) {
      if (!quiet) say.err(words("servers").refreshFailed({ error: errorText(e) }));
    } finally {
      this.loading = false;
      this.refreshing = false;
    }
  }

  async loadStats() {
    try {
      this.stats = await ipc.getAppStats();
    } catch {
      // The title bar shows dashes.
    }
  }

  async loadSteamPlayers() {
    try {
      this.steamPlayers = await ipc.fetchSteamPlayerCount();
    } catch {
      // Same.
    }
  }

  // ── pings ───────────────────────────────────────────────────────────────
  /** Results arrive in batches over a Channel; they are applied once per frame. */
  #onBatch = (results: PingResult[]) => {
    this.#batch.push(...results);
    this.#raf ??= requestAnimationFrame(() => {
      this.#raf = null;
      this.#flush();
    });
  };

  #flush() {
    const batch = this.#batch;
    this.#batch = [];
    for (const r of batch) {
      const key = `${r.ip}:${r.port}`;
      this.pending.delete(key);
      this.ping.set(key, r.ms);
      if (r.failed) {
        this.a2sFailures.add(key);
        this.timeouts.set(key, (this.timeouts.get(key) ?? 0) + 1);
      } else {
        if (this.a2sFailures.has(key)) this.a2sFailures.delete(key);
        if (this.timeouts.has(key)) this.timeouts.delete(key);
        if (r.players != null) {
          const sv = this.#byKey.get(key);
          this.live.set(key, { players: r.players, max: r.max_players ?? sv?.max_players ?? 0, bots: r.bots ?? 0 });
        }
      }
    }
    if (this.scan) {
      const done = this.scan.done + batch.length;
      this.scan = done >= this.scan.total ? null : { ...this.scan, done };
    }
  }

  /**
   * Ping every server, favourites first, then history, then the rest, as one
   * backend call: the backend starts tasks in list order, and each call
   * aborts the one before, so three calls would cancel the priority tiers.
   */
  async startScan() {
    const p = profile.data;
    const favs = new Set((p?.favorites ?? []).map((f) => `${f.ip}:${f.port}`));
    const hist = new Set((p?.history ?? []).map((h) => `${h.ip}:${h.port}`).filter((k) => !favs.has(k)));
    const all = this.list.map(keyOf);
    const scanFav = p?.ping_scan_favorites ?? true;
    const scanHist = p?.ping_scan_history ?? true;
    const scanRest = p?.ping_scan_servers ?? true;
    const targets = [
      ...(scanFav ? all.filter((k) => favs.has(k)) : []),
      ...(scanHist ? all.filter((k) => hist.has(k)) : []),
      ...(scanRest ? all.filter((k) => !favs.has(k) && !hist.has(k)) : []),
    ];
    if (targets.length === 0) return;
    this.scan = { total: targets.length, done: 0 };
    const ch = new Channel<PingResult[]>();
    ch.onmessage = this.#onBatch;
    await ipc
      .pingAllBackground(targets, p?.ping_concurrency ?? 64, p?.ping_timeout_auto ?? 2000, ch)
      .catch(() => (this.scan = null));
  }

  /** Ping the rows on screen that have never answered. */
  pingVisible(keys: string[]) {
    const todo = keys.filter((k) => !this.pending.has(k) && !this.ping.has(k));
    if (todo.length === 0) return;
    for (const k of todo) this.pending.add(k);
    const ch = new Channel<PingResult[]>();
    ch.onmessage = this.#onBatch;
    const p = profile.data;
    void ipc.pingServers(todo, p?.ping_concurrency ?? 64, p?.ping_timeout_auto ?? 2000, ch).catch(() => {
      for (const k of todo) this.pending.delete(k);
    });
  }

  /** One server, asked by the player, with the longer manual timeout. */
  async pingOne(ip: string, port: number) {
    const key = `${ip}:${port}`;
    this.timeouts.delete(key);
    this.pending.add(key);
    try {
      this.ping.set(key, await ipc.pingSingle(ip, port, profile.data?.ping_timeout_manual ?? 10_000));
      this.a2sFailures.delete(key);
    } catch {
      this.ping.delete(key);
      this.a2sFailures.add(key);
    } finally {
      this.pending.delete(key);
    }
  }

  async cancelScan() {
    await ipc.cancelPing().catch(() => {});
    this.scan = null;
    this.scanPaused = false;
  }

  async toggleScanPause() {
    try {
      this.scanPaused = await ipc.togglePingPause();
    } catch {
      // Stays as it was.
    }
  }

  // ── full servers ────────────────────────────────────────────────────────
  // The list marks many servers full optimistically; each is asked directly,
  // eight at a time, so a stale "full" is corrected or flagged.
  #checked = new Set<string>();
  #queue: ServerDto[] = [];
  #draining = false;

  #queueFullChecks() {
    for (const s of this.list) {
      if (s.players > 0 && s.players === s.max_players) {
        const k = keyOf(s);
        if (!this.#checked.has(k)) {
          this.#checked.add(k);
          this.#queue.push(s);
        }
      }
    }
    void this.#drain();
  }

  async #drain() {
    if (this.#draining) return;
    this.#draining = true;
    const { serverData } = await import("./server-data.svelte");
    await Promise.all(
      Array.from({ length: 8 }, async () => {
        for (let s = this.#queue.shift(); s; s = this.#queue.shift()) {
          await serverData.refreshA2s(s.ip, s.query_port).catch(() => {});
        }
      }),
    );
    this.#draining = false;
  }

  /** Warm the detail cache for the busiest servers, gently. */
  async prefetchTop(count = 20) {
    const { serverData } = await import("./server-data.svelte");
    const top = [...this.list].sort((a, b) => b.players - a.players).slice(0, count);
    for (const s of top) {
      if (serverData.hasFreshA2s(s.ip, s.query_port)) continue;
      void serverData.refreshA2s(s.ip, s.query_port);
      await new Promise((r) => setTimeout(r, 100));
    }
  }
}

export const servers = new Servers();
