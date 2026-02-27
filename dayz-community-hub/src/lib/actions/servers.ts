import { invoke } from '@tauri-apps/api/core';
import type { ServerDto } from '$lib/types';
import { app as s } from '$lib/state.svelte';

export async function loadServers() {
  s.serversLoading = true;
  try {
    s.servers = await invoke<ServerDto[]>('get_servers');
  } catch (e) {
    s.setStatus(`Failed to load servers: ${e}`, 'error');
  } finally {
    s.serversLoading = false;
  }
}

export async function refreshServers() {
  if (s.serversRefreshing) return;
  s.serversRefreshing = true;
  s.serversLoading = true;
  s.setStatus('Refreshing server list…', 'info');
  try {
    s.servers = await invoke<ServerDto[]>('refresh_servers');
    s.setStatus(`Loaded ${s.servers.length} servers`, 'success');
    startPinging();
    // Refresh titlebar counters (servers, in-game, Steam players)
    loadStats();
    loadSteamPlayers();
  } catch (e) {
    s.setStatus(`Refresh failed: ${e}`, 'error');
  } finally {
    s.serversLoading = false;
    s.serversRefreshing = false;
  }
}

/** Silent background refresh — updates servers without blocking UI */
export async function refreshServersBackground() {
  if (s.serversRefreshing) return;
  s.serversRefreshing = true;
  try {
    const freshServers = await invoke<ServerDto[]>('refresh_servers');
    s.servers = freshServers;
    loadStats();
    loadSteamPlayers();
  } catch {
    // Non-fatal — we already have cached data
  } finally {
    s.serversRefreshing = false;
  }
}

export function startPinging() {
  const portMap = new Map<string, number>();
  for (const sv of s.servers) {
    portMap.set(`${sv.ip}:${sv.game_port}`, sv.query_port);
    portMap.set(`${sv.ip}:${sv.query_port}`, sv.query_port);
  }

  const seen = new Set<string>();
  const targets: string[] = [];

  function addTarget(ip: string, port: number) {
    const resolvedPort = portMap.get(`${ip}:${port}`) ?? port;
    const key = `${ip}:${resolvedPort}`;
    if (!seen.has(key)) {
      seen.add(key);
      targets.push(key);
    }
  }

  if (s.profile) {
    for (const f of s.profile.favorites) addTarget(f.ip, f.port);
    for (const h of s.profile.history) addTarget(h.ip, h.port);
  }

  invoke('start_pinging', { targets }).catch(() => {});
}

export async function pingSingle(ip: string, port: number) {
  const key = `${ip}:${port}`;
  try {
    const ms = await invoke<number>('ping_single', { ip, port, timeoutMs: 10_000 });
    s.pingCache.set(key, ms);
    s.pingCache = new Map(s.pingCache);
  } catch {
    s.pingCache.delete(key);
    s.pingCache = new Map(s.pingCache);
  }
}

export async function loadStats() {
  try {
    s.stats = await invoke<import('$lib/types').AppStatsDto>('get_app_stats');
  } catch { /* non-fatal */ }
}

export async function loadSteamPlayers() {
  try {
    s.steamPlayers = await invoke<number>('fetch_steam_player_count');
  } catch { /* non-fatal */ }
}
