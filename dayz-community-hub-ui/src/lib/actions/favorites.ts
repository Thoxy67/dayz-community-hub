import { invoke } from '@tauri-apps/api/core';
import type { ServerDto, FavoriteDto, HistoryDto } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import { loadProfile } from './profile';
import { withProfileFallback } from '$lib/utils/action-helpers';
import { confirmActionWithProfile } from '$lib/utils/dialog-helpers';
import * as m from '$lib/paraglide/messages.js';

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
  await withProfileFallback(
    async () => {
      profileAddFavorite(server.name, server.ip, server.query_port);
      await invoke('add_favorite', {
        name: server.name,
        ip: server.ip,
        port: server.query_port,
        password: null,
      });
    },
    m.favorites_added({ name: server.name }),
  );
}

export async function addFavoriteDirect(name: string, ip: string, port: number, password?: string) {
  await withProfileFallback(async () => {
    profileAddFavorite(name, ip, port, password);
    await invoke('add_favorite', { name, ip, port, password: password ?? null });
  }, m.favorites_added({ name }));
}

export function removeFavorite(fav: FavoriteDto) {
  confirmActionWithProfile(
    m.favorites_remove_title(),
    m.favorites_remove_message({ name: fav.name }),
    async () => {
      profileRemoveFavorite(fav.ip, fav.port);
      await invoke('remove_favorite', { ip: fav.ip, port: fav.port });
    },
    m.favorites_removed(),
  );
}

export async function removeFavoriteQuick(ip: string, port: number) {
  await withProfileFallback(async () => {
    profileRemoveFavorite(ip, port);
    await invoke('remove_favorite', { ip, port });
  }, m.favorites_removed());
}

// ── IP exclusion ──────────────────────────────────────────────────────────

export async function excludeIp(ip: string) {
  if (!s.profile) return;
  if (!s.profile.excluded_ips.includes(ip)) {
    s.profile.excluded_ips = [...s.profile.excluded_ips, ip];
  }
  await withProfileFallback(
    () => invoke('add_excluded_ip', { ip }),
    m.favorites_ip_excluded({ ip }),
    m.favorites_ip_exclude_failed(),
  );
}

export async function unexcludeIp(ip: string) {
  if (!s.profile) return;
  s.profile.excluded_ips = s.profile.excluded_ips.filter((e) => e !== ip);
  await withProfileFallback(() => invoke('remove_excluded_ip', { ip }), m.favorites_ip_unexcluded({ ip }));
}
