/**
 * The server list, as the interface sees it: never the whole list.
 *
 * The backend holds the nine-thousand-odd servers, their pings and live
 * counts, and filters, sorts and counts them (`servers_query`). This store
 * keeps only what is on screen: the rows a view asked for, and the few
 * servers other parts of the window mention (favourites, history, the rejoin
 * card), fetched by address in batches. When the backend says something
 * changed (`servers-changed`, at most twice a second), whatever is shown is
 * asked for again.
 */
import { SvelteMap, SvelteSet } from "svelte/reactivity";
import { listen } from "@tauri-apps/api/event";
import * as ipc from "$lib/ipc/servers";
import type { MapCount, ScanProgress, ServerRow } from "$lib/ipc/servers";
import { Channel, inTauri } from "$lib/ipc/core";
import type { AppStatsDto, PingResult } from "$lib/ipc/types";
import { words } from "$lib/i18n";
import { profile } from "./profile.svelte";
import { say, errorText } from "./say";

export type { ServerRow };

/** A server's key everywhere in the app. */
export const keyOf = (s: { ip: string; query_port: number }) => `${s.ip}:${s.query_port}`;

export type Live = { players: number; max: number; bots: number };

/** Data older than this earns a warning and a refresh button. */
export const STALE_MS = 10 * 60 * 1000;

class Servers {
  /** How many servers the backend holds. */
  total = $state(0);
  maps = $state.raw<MapCount[]>([]);
  loading = $state(false);
  refreshing = $state(false);
  lastRefreshed = $state(0);
  /** The title bar's totals (servers, players in game, account). */
  stats = $state<AppStatsDto | null>(null);
  steamPlayers = $state<number | null>(null);
  /** Bumped when the backend's data changed: views re-query on it. */
  generation = $state(0);

  scan = $state<{ total: number; done: number } | null>(null);
  scanPaused = $state(false);

  /** Rows seen, by "ip:query_port" and "ip:game_port". */
  known = new SvelteMap<string, ServerRow>();
  /** Round trip per server, for the servers other views show (ms; ≥ 5000 is a timeout). */
  ping = new SvelteMap<string, number>();
  pending = new SvelteSet<string>();
  timeouts = new SvelteMap<string, number>();
  a2sFailures = new SvelteSet<string>();

  // ── rows ────────────────────────────────────────────────────────────────
  /** Put rows from a query or a lookup where `find` and the pings see them. */
  remember(rows: readonly ServerRow[]) {
    for (const r of rows) {
      this.known.set(`${r.ip}:${r.query_port}`, r);
      this.known.set(`${r.ip}:${r.game_port}`, r);
      const k = keyOf(r);
      if (r.ping_ms != null) this.ping.set(k, r.ping_failed ? 9999 : r.ping_ms);
      if (r.unverified_full) this.a2sFailures.add(k);
      else if (this.a2sFailures.has(k)) this.a2sFailures.delete(k);
    }
  }

  #wanted = new Set<string>();
  #watched = new Set<string>();
  #lookupTimer: ReturnType<typeof setTimeout> | null = null;

  /**
   * A server by address, whether `port` is its query or its game port. What
   * is not known yet is fetched (batched with others asked for at the same
   * moment) and appears when it arrives; `undefined` meanwhile, and for
   * servers the list does not have.
   */
  find(ip: string, port: number): ServerRow | undefined {
    const key = `${ip}:${port}`;
    this.#watched.add(key);
    const row = this.known.get(key);
    if (!row && !this.#missing.has(key)) this.#want(key);
    return row;
  }

  /** A server by address, asking the backend now if it is not known yet. */
  async resolve(ip: string, port: number): Promise<ServerRow | undefined> {
    const key = `${ip}:${port}`;
    const row = this.known.get(key);
    if (row) return row;
    try {
      const [r] = await ipc.serversLookup([key]);
      if (r) this.remember([r]);
      return r ?? undefined;
    } catch {
      return undefined;
    }
  }

  /** Addresses the backend said it does not list, until the next change. */
  #missing = new Set<string>();

  #want(key: string) {
    this.#wanted.add(key);
    this.#lookupTimer ??= setTimeout(() => void this.#lookup(), 30);
  }

  async #lookup() {
    this.#lookupTimer = null;
    const keys = [...this.#wanted];
    this.#wanted.clear();
    if (keys.length === 0 || !this.total) return;
    try {
      const rows = await ipc.serversLookup(keys);
      const found: ServerRow[] = [];
      rows.forEach((r, i) => (r ? found.push(r) : this.#missing.add(keys[i]!)));
      this.remember(found);
    } catch {
      // Unknown for now; asked again on the next change.
    }
  }

  /** The freshest head-count for a row. */
  count(s: { players: number; max_players: number; bots?: number | null }): Live {
    return { players: s.players, max: s.max_players, bots: s.bots ?? 0 };
  }

  /** A query answered by the server itself: shown at once, before the next change. */
  applyLiveCount(key: string, players: number, max: number, bots = 0) {
    const r = this.known.get(key);
    if (r) this.remember([{ ...r, players, max_players: max, bots }]);
  }

  // ── loading ─────────────────────────────────────────────────────────────
  #listening = false;

  /** After startup: the maps, the backend's change events, the first scan. */
  async load() {
    this.loading = true;
    try {
      this.maps = await ipc.serverMaps();
      this.total = this.maps.reduce((n, m) => n + m.count, 0);
      this.lastRefreshed = Date.now();
      this.generation++;
      if (inTauri && !this.#listening) {
        this.#listening = true;
        void listen<{ generation: number }>("servers-changed", (e) => this.#changed(e.payload.generation));
      }
    } catch (e) {
      say.err(words("servers").loadFailed({ error: errorText(e) }));
    } finally {
      this.loading = false;
    }
  }

  #changed(generation: number) {
    this.generation = Math.max(this.generation + 1, generation);
    this.#missing.clear();
    // Whatever other views show is asked for again, pings included.
    const watched = [...this.#watched].slice(-300);
    this.#watched = new Set(watched);
    for (const k of watched) this.#wanted.add(k);
    if (watched.length) this.#lookupTimer ??= setTimeout(() => void this.#lookup(), 30);
  }

  /** Fetch a fresh list into the backend. `quiet` for the background refresh after a cached start. */
  async refresh(quiet = false) {
    if (this.refreshing) return;
    this.refreshing = true;
    if (!quiet) {
      this.loading = true;
      say.info(words("servers").refreshing);
    }
    try {
      await this.cancelScan();
      const count = await ipc.refreshServers();
      this.known.clear();
      this.ping.clear();
      await this.load();
      if (!quiet) say.ok(words("servers").loaded({ count }));
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
  /** The backend pings everything in its own order; only progress comes back. */
  async startScan() {
    const ch = new Channel<ScanProgress>();
    ch.onmessage = (p) => {
      this.scanPaused = p.paused;
      this.scan = p.running ? { total: p.total, done: p.done } : null;
    };
    await ipc.startScan(ch).catch(() => (this.scan = null));
  }

  #apply = (results: PingResult[]) => {
    for (const r of results) {
      const key = `${r.ip}:${r.port}`;
      this.pending.delete(key);
      this.ping.set(key, r.ms);
      if (r.failed) {
        this.a2sFailures.add(key);
        this.timeouts.set(key, (this.timeouts.get(key) ?? 0) + 1);
      } else {
        if (this.a2sFailures.has(key)) this.a2sFailures.delete(key);
        if (this.timeouts.has(key)) this.timeouts.delete(key);
        if (r.players != null) this.applyLiveCount(key, r.players, r.max_players ?? 0, r.bots ?? 0);
      }
    }
  };

  /** Ping a few servers that have never answered (favourites, history). */
  pingVisible(keys: string[]) {
    const todo = keys.filter((k) => !this.pending.has(k) && !this.ping.has(k));
    if (todo.length === 0) return;
    for (const k of todo) this.pending.add(k);
    const ch = new Channel<PingResult[]>();
    ch.onmessage = this.#apply;
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
      this.ping.set(key, 9999);
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
}

export const servers = new Servers();
