import { invoke } from '@tauri-apps/api/core';
import type { ProfileDto } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import { loadStats } from './servers';
import * as m from '$lib/paraglide/messages.js';

export async function loadProfile() {
  try {
    s.profile = await invoke<ProfileDto>('get_profile');
  } catch (e) {
    s.setStatus(m.profile_load_failed({ error: String(e) }), 'error');
  }
}

export async function saveProfileSettings(
  player: string | null,
  steamLogin: string | null,
  steamPassword: string | null,
  steamRoot: string | null,
  steamcmdEnabled: boolean,
  steamcmdPath: string | null,
  steamApiKey: string | null,
  steamId: string | null,
  battlemetricsApiKey: string | null,
  userLocation: [number, number] | null,
  pingConcurrency?: number,
  pingTimeoutAuto?: number,
  pingTimeoutManual?: number,
) {
  // The Rust command requires every ping field. Callers that don't touch
  // ping settings (TitleBar settings modal) should preserve the existing
  // values from the profile rather than overwriting them with defaults.
  const p = s.profile;
  try {
    await invoke('save_profile_settings', {
      player,
      steamLogin,
      steamPassword,
      steamRoot,
      steamcmdEnabled,
      steamcmdPath,
      steamApiKey,
      steamId,
      battlemetricsApiKey,
      userLocation,
      pingConcurrency: pingConcurrency ?? p?.ping_concurrency ?? 64,
      pingTimeoutAuto: pingTimeoutAuto ?? p?.ping_timeout_auto ?? 2000,
      pingTimeoutManual: pingTimeoutManual ?? p?.ping_timeout_manual ?? 10000,
      pingMaxRetries: p?.ping_max_retries ?? 0,
      pingScanFavorites: p?.ping_scan_favorites ?? true,
      pingScanHistory: p?.ping_scan_history ?? true,
      pingScanServers: p?.ping_scan_servers ?? true,
    });
    const tasks: Promise<unknown>[] = [loadProfile(), loadStats()];
    if (steamApiKey && steamId) {
      tasks.push(
        invoke<string | null>('fetch_steam_avatar')
          .then((url) => {
            s.avatarUrl = url;
          })
          .catch(() => {}),
      );
    } else {
      s.avatarUrl = null;
    }
    await Promise.all(tasks);
    s.setStatus(m.profile_settings_saved(), 'success');
  } catch (e) {
    s.setStatus(m.profile_settings_save_failed({ error: String(e) }), 'error');
  }
}

/** Save only ping settings (for auto-save from sliders). */
export async function savePingSettings(
  pingConcurrency: number,
  pingTimeoutAuto: number,
  pingTimeoutManual: number,
  pingMaxRetries: number,
  pingScanFavorites: boolean,
  pingScanHistory: boolean,
  pingScanServers: boolean,
) {
  if (!s.profile) return;
  const p = s.profile;
  try {
    await invoke('save_profile_settings', {
      player: p.player,
      steamLogin: p.steam_login,
      steamPassword: p.steam_password,
      steamRoot: p.steam_root,
      steamcmdEnabled: p.steamcmd_enabled,
      steamcmdPath: p.steamcmd_path,
      steamApiKey: p.steam_api_key,
      steamId: p.steam_id,
      battlemetricsApiKey: p.battlemetrics_api_key,
      userLocation: p.user_location,
      pingConcurrency,
      pingTimeoutAuto,
      pingTimeoutManual,
      pingMaxRetries,
      pingScanFavorites,
      pingScanHistory,
      pingScanServers,
    });
    // Update local state
    s.profile = {
      ...p,
      ping_concurrency: pingConcurrency,
      ping_timeout_auto: pingTimeoutAuto,
      ping_timeout_manual: pingTimeoutManual,
      ping_max_retries: pingMaxRetries,
      ping_scan_favorites: pingScanFavorites,
      ping_scan_history: pingScanHistory,
      ping_scan_servers: pingScanServers,
    };
  } catch (e) {
    s.setStatus(m.profile_settings_save_failed({ error: String(e) }), 'error');
  }
}

export async function toggleOption(key: string) {
  const prev = s.profile?.options.find((o) => o.key === key)?.enabled;
  if (s.profile) {
    s.profile.options = s.profile.options.map((o) => (o.key === key ? { ...o, enabled: !o.enabled } : o));
  }
  try {
    await invoke<boolean>('toggle_launch_option', { key });
  } catch (e) {
    if (s.profile) {
      s.profile.options = s.profile.options.map((o) => (o.key === key ? { ...o, enabled: prev ?? o.enabled } : o));
    }
    s.setStatus(m.profile_option_toggle_failed({ error: String(e) }), 'error');
  }
}

export async function setOptionValue(key: string, value: string | null) {
  const prevOpt = s.profile?.options.find((o) => o.key === key);
  if (s.profile) {
    s.profile.options = s.profile.options.map((o) =>
      o.key === key ? { ...o, value, enabled: value !== null ? true : o.enabled } : o,
    );
  }
  try {
    await invoke('set_launch_option_value', { key, value });
  } catch (e) {
    if (s.profile && prevOpt) {
      s.profile.options = s.profile.options.map((o) => (o.key === key ? { ...prevOpt } : o));
    }
    s.setStatus(m.profile_option_set_failed({ error: String(e) }), 'error');
  }
}
