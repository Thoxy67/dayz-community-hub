import { invoke } from '@tauri-apps/api/core';
import type { ServerDto, FavoriteDto, HistoryDto } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import { loadProfile } from './profile';

/** Append a favorite to local profile state without a round-trip. */
export function profileAddFavorite(name: string, ip: string, port: number, password?: string) {
  if (!s.profile) return;
  const existing = s.profile.favorites.find((f) => f.ip === ip && f.port === port);
  if (existing) {
    existing.name = name;
    if (password !== undefined) existing.password = password || null;
    s.profile.favorites = [...s.profile.favorites];
  } else {
    s.profile.favorites = [...s.profile.favorites, { name, ip, port, password: password ?? null }];
  }
}

/** Remove a favorite from local profile state without a round-trip. */
export function profileRemoveFavorite(ip: string, port: number) {
  if (!s.profile) return;
  s.profile.favorites = s.profile.favorites.filter((f) => !(f.ip === ip && f.port === port));
}

export async function addFavorite(server: ServerDto) {
  try {
    profileAddFavorite(server.name, server.ip, server.query_port);
    await invoke('add_favorite', {
      name: server.name,
      ip: server.ip,
      port: server.query_port,
      password: null,
    });
    s.setStatus(`Added ${server.name} to favorites`, 'success');
  } catch (e) {
    await loadProfile();
    s.setStatus(`Failed: ${e}`, 'error');
  }
}

export async function addFavoriteDirect(name: string, ip: string, port: number, password?: string) {
  try {
    profileAddFavorite(name, ip, port, password);
    await invoke('add_favorite', { name, ip, port, password: password ?? null });
    s.setStatus(`Added ${name} to favorites`, 'success');
  } catch (e) {
    await loadProfile();
    s.setStatus(`Failed: ${e}`, 'error');
  }
}

export function removeFavorite(fav: FavoriteDto) {
  s.confirmDialog = {
    title: 'Remove Favorite',
    message: `Remove '${fav.name}' from favorites?`,
    onConfirm: async () => {
      try {
        profileRemoveFavorite(fav.ip, fav.port);
        await invoke('remove_favorite', { ip: fav.ip, port: fav.port });
        s.setStatus('Removed from favorites', 'success');
      } catch (e) {
        await loadProfile();
        s.setStatus(`Failed: ${e}`, 'error');
      }
    },
  };
}

export async function removeFavoriteQuick(ip: string, port: number) {
  try {
    profileRemoveFavorite(ip, port);
    await invoke('remove_favorite', { ip, port });
    s.setStatus('Removed from favorites', 'success');
  } catch (e) {
    await loadProfile();
    s.setStatus(`Failed: ${e}`, 'error');
  }
}

// ── IP exclusion ──────────────────────────────────────────────────────────

export async function excludeIp(ip: string) {
  if (!s.profile) return;
  if (!s.profile.excluded_ips.includes(ip)) {
    s.profile.excluded_ips = [...s.profile.excluded_ips, ip];
  }
  try {
    await invoke('add_excluded_ip', { ip });
    s.setStatus(`${ip} excluded from server list`, 'info');
  } catch (e) {
    await loadProfile();
    s.setStatus(`Failed to exclude IP: ${e}`, 'error');
  }
}

export async function unexcludeIp(ip: string) {
  if (!s.profile) return;
  s.profile.excluded_ips = s.profile.excluded_ips.filter((e) => e !== ip);
  try {
    await invoke('remove_excluded_ip', { ip });
    s.setStatus(`${ip} removed from exclusions`, 'success');
  } catch (e) {
    await loadProfile();
    s.setStatus(`Failed: ${e}`, 'error');
  }
}
