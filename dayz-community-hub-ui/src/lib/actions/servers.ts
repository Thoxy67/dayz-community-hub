import { invoke, Channel } from '@tauri-apps/api/core';
import type { ServerDto, PingResult } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import * as m from '$lib/paraglide/messages.js';
import { createServerLookup } from '$lib/utils/server-lookup';
import { serverData } from '$lib/services/server-data.svelte';

// ── Ping batch handling with RAF coalescing ───────────────────────────────

let pendingPingBatch: PingResult[] = [];
let pingRafId: number | null = null;
let serverLookupCache: Map<string, ServerDto> | null = null;

/** Handle a batch of ping results from Channel, coalesced via requestAnimationFrame */
function handlePingBatch(results: PingResult[]) {
  pendingPingBatch.push(...results);

  if (!pingRafId) {
    pingRafId = requestAnimationFrame(() => {
      flushPendingPingBatch();
      pingRafId = null;
    });
  }
}

/** Flush accumulated ping results to state */
function flushPendingPingBatch() {
  if (pendingPingBatch.length === 0) return;

  const batchSize = pendingPingBatch.length;

  // Lazily create server lookup (invalidated when servers change)
  if (!serverLookupCache) {
    serverLookupCache = createServerLookup(s.servers);
  }

  for (const r of pendingPingBatch) {
    const key = `${r.ip}:${r.port}`;
    s.pingPending.delete(key);
    s.pingCache.set(key, r.ms);

    if (r.failed) {
      s.a2sFailures.add(key);
      s.pingTimeouts.set(key, (s.pingTimeouts.get(key) ?? 0) + 1);
    } else {
      s.a2sFailures.delete(key);
      s.pingTimeouts.delete(key);
      // Update player count on success
      if (r.players !== undefined) {
        const server = serverLookupCache.get(key);
        if (server) {
          server.players = r.players;
          if (r.max_players !== undefined) server.max_players = r.max_players;
        }
      }
    }
  }

  // Update ping session progress (if active)
  if (s.pingSession) {
    const newCompleted = s.pingSession.completed + batchSize;
    if (newCompleted >= s.pingSession.total) {
      // All batches received - clear session
      s.pingSession = null;
    } else {
      s.pingSession = {
        ...s.pingSession,
        completed: newCompleted,
      };
    }
  }

  // Trigger Svelte reactivity once per RAF frame (all mutated collections)
  s.pingCache = new Map(s.pingCache);
  s.pingPending = new Set(s.pingPending);
  s.a2sFailures = new Set(s.a2sFailures);
  s.pingTimeouts = new Map(s.pingTimeouts);

  pendingPingBatch = [];
}

/** Invalidate server lookup cache (call when servers list changes) */
export function invalidateServerLookup() {
  serverLookupCache = null;
}

export async function loadServers() {
  s.serversLoading = true;
  try {
    s.servers = await invoke<ServerDto[]>('get_servers');
    s.serversLastRefreshed = Date.now();
  } catch (e) {
    s.setStatus(m.servers_load_failed({ error: String(e) }), 'error');
  } finally {
    s.serversLoading = false;
  }
}

export async function refreshServers() {
  if (s.serversRefreshing) return;

  // Cancel any ongoing ping scan before refreshing
  await cancelPing();

  s.serversRefreshing = true;
  s.serversLoading = true;
  s.setStatus(m.servers_refreshing(), 'info');
  try {
    s.servers = await invoke<ServerDto[]>('refresh_servers');
    s.serversLastRefreshed = Date.now();
    s.setStatus(m.servers_loaded({ count: s.servers.length }), 'success');
    // Refresh titlebar counters (servers, in-game, Steam players)
    loadStats();
    loadSteamPlayers();
    // Re-ping all servers with fresh data
    startBackgroundPing();
  } catch (e) {
    s.setStatus(m.servers_refresh_failed({ error: String(e) }), 'error');
  } finally {
    s.serversLoading = false;
    s.serversRefreshing = false;
  }
}

/** Silent background refresh — updates servers without blocking UI */
export async function refreshServersBackground() {
  if (s.serversRefreshing) return;

  // Cancel any ongoing ping scan before refreshing
  await cancelPing();

  s.serversRefreshing = true;
  try {
    const freshServers = await invoke<ServerDto[]>('refresh_servers');
    s.servers = freshServers;
    s.serversLastRefreshed = Date.now();
    loadStats();
    loadSteamPlayers();
    // Re-ping all servers with fresh data
    startBackgroundPing();
  } catch {
    // Non-fatal — we already have cached data
  } finally {
    s.serversRefreshing = false;
  }
}

/** Start background pinging with streaming results via Channel.
 * Pings in priority order: favorites first, then history, then all other servers.
 * Results are streamed progressively in batches.
 * Respects ping_scan_* settings to allow user to choose which tabs to ping.
 */
export async function startBackgroundPing() {
  const allTargets = s.servers.map((sv) => `${sv.ip}:${sv.query_port}`);
  if (allTargets.length === 0) return;

  // Get ping scope settings (defaults to true for all)
  const scanFavorites = s.profile?.ping_scan_favorites ?? true;
  const scanHistory = s.profile?.ping_scan_history ?? true;
  const scanServers = s.profile?.ping_scan_servers ?? true;

  // If all scopes are disabled, don't ping anything
  if (!scanFavorites && !scanHistory && !scanServers) return;

  // Reset server lookup cache for fresh ping session
  invalidateServerLookup();

  // Build separate sets for favorites and history
  const favoritesSet = new Set<string>();
  for (const fav of s.profile?.favorites ?? []) {
    favoritesSet.add(`${fav.ip}:${fav.port}`);
  }
  const historySet = new Set<string>();
  for (const h of s.profile?.history ?? []) {
    // Don't add if already in favorites (avoid duplicates)
    const key = `${h.ip}:${h.port}`;
    if (!favoritesSet.has(key)) {
      historySet.add(key);
    }
  }

  // Split into priority tiers (only include enabled scopes)
  const favoriteTargets = scanFavorites ? allTargets.filter((t) => favoritesSet.has(t)) : [];
  const historyTargets = scanHistory ? allTargets.filter((t) => historySet.has(t)) : [];
  const restTargets = scanServers ? allTargets.filter((t) => !favoritesSet.has(t) && !historySet.has(t)) : [];

  // Calculate total targets to ping
  const totalTargets = favoriteTargets.length + historyTargets.length + restTargets.length;
  if (totalTargets === 0) return;

  // Start ping session for progress bar
  s.pingSession = { active: true, total: totalTargets, completed: 0 };

  // Get ping settings from profile
  const concurrency = s.profile?.ping_concurrency ?? 25;
  const timeoutMs = s.profile?.ping_timeout_auto ?? 2000;

  // 1. Ping favorites first
  if (favoriteTargets.length > 0) {
    const onProgress = new Channel<PingResult[]>();
    onProgress.onmessage = handlePingBatch;
    await invoke('ping_all_background', { targets: favoriteTargets, concurrency, timeoutMs, onProgress }).catch(
      () => {},
    );
  }

  // 2. Then ping history
  if (historyTargets.length > 0) {
    const onProgress = new Channel<PingResult[]>();
    onProgress.onmessage = handlePingBatch;
    await invoke('ping_all_background', { targets: historyTargets, concurrency, timeoutMs, onProgress }).catch(
      () => {},
    );
  }

  // 3. Then ping the rest
  if (restTargets.length > 0) {
    const onProgress = new Channel<PingResult[]>();
    onProgress.onmessage = handlePingBatch;
    await invoke('ping_all_background', { targets: restTargets, concurrency, timeoutMs, onProgress }).catch(() => {});
  }

  // Session is cleared by flushPendingPingBatch when completed >= total
  // If something goes wrong and batches never arrive, bar will stay visible
  // (acceptable tradeoff - user can refresh to reset)
}

/** Ping visible servers with longer timeout - streams results via Channel. */
export async function pingVisibleServers(targets: string[]) {
  if (targets.length === 0) return;

  // Get ping settings from profile
  const concurrency = s.profile?.ping_concurrency ?? 25;
  const timeoutMs = s.profile?.ping_timeout_auto ?? 2000;

  const onProgress = new Channel<PingResult[]>();
  onProgress.onmessage = handlePingBatch;
  await invoke('ping_servers', { targets, concurrency, timeoutMs, onProgress }).catch(() => {});
}

/** Fetch all ping results from Rust cache and update state.
 * Fallback method - primary flow uses Channel streaming.
 * Useful for recovering missed results or manual refresh.
 */
export async function fetchPingResults() {
  const targets = s.servers.map((sv) => `${sv.ip}:${sv.query_port}`);
  if (targets.length === 0) return;

  try {
    const results = await invoke<PingResult[]>('get_pings', { targets });
    if (results.length === 0) return;

    // Single state update - create lookup once
    const serverLookup = createServerLookup(s.servers);

    for (const r of results) {
      const key = `${r.ip}:${r.port}`;
      s.pingPending.delete(key);
      s.pingCache.set(key, r.ms);

      if (r.failed) {
        s.a2sFailures.add(key);
        s.pingTimeouts.set(key, (s.pingTimeouts.get(key) ?? 0) + 1);
      } else {
        s.a2sFailures.delete(key);
        s.pingTimeouts.delete(key);
        // Update player count only on success
        if (r.players !== undefined) {
          const server = serverLookup.get(key);
          if (server) {
            server.players = r.players;
            if (r.max_players !== undefined) server.max_players = r.max_players;
          }
        }
      }
    }

    // Single reactivity trigger for all mutated collections
    s.pingCache = new Map(s.pingCache);
    s.pingPending = new Set(s.pingPending);
    s.a2sFailures = new Set(s.a2sFailures);
    s.pingTimeouts = new Map(s.pingTimeouts);
  } catch {
    // Non-fatal
  }
}

export async function pingSingle(ip: string, port: number) {
  const key = `${ip}:${port}`;
  const timeoutMs = s.profile?.ping_timeout_manual ?? 10_000;
  // Reset timeout counter on manual ping so server gets re-enabled for auto-ping
  s.pingTimeouts.delete(key);
  try {
    const ms = await invoke<number>('ping_single', { ip, port, timeoutMs });
    s.pingCache.set(key, ms);
    s.a2sFailures.delete(key);
  } catch {
    s.pingCache.delete(key);
    s.a2sFailures.add(key);
  }
  // Single reactivity trigger for all mutated collections
  s.pingCache = new Map(s.pingCache);
  s.pingTimeouts = new Map(s.pingTimeouts);
  s.a2sFailures = new Set(s.a2sFailures);
}

export async function loadStats() {
  try {
    s.stats = await invoke<import('$lib/types').AppStatsDto>('get_app_stats');
  } catch {
    /* non-fatal */
  }
}

export async function loadSteamPlayers() {
  try {
    s.steamPlayers = await invoke<number>('fetch_steam_player_count');
  } catch {
    /* non-fatal */
  }
}

/** Cancel any ongoing ping scan and clear session. */
export async function cancelPing() {
  try {
    await invoke('cancel_ping');
  } catch {
    /* non-fatal */
  }
  s.pingSession = null;
  s.pingPaused = false;
}

/** Toggle ping pause state. */
export async function togglePingPause() {
  try {
    const paused = await invoke<boolean>('toggle_ping_pause');
    s.pingPaused = paused;
  } catch {
    /* non-fatal */
  }
}

/** Prefetch A2S data for top servers by player count.
 * Improves perceived performance when opening server detail panels.
 * Called after servers are loaded, runs in background with staggered requests.
 */
export async function prefetchTopServersA2s(count = 20) {
  const servers = s.servers;
  if (servers.length === 0) return;

  // Get top servers by player count
  const top = servers
    .slice()
    .sort((a, b) => b.players - a.players)
    .slice(0, count);

  // Prefetch A2S data with staggered requests to avoid overwhelming the network
  for (const server of top) {
    // Skip if already cached
    if (serverData.hasValidA2s(server.ip, server.query_port)) continue;

    // Fire and forget - don't wait for result
    serverData.refreshA2s(server.ip, server.query_port).catch(() => {});

    // Small delay between requests to avoid burst
    await new Promise((r) => setTimeout(r, 100));
  }
}
