import { invoke } from '@tauri-apps/api/core';
import type { ProfileDto } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import { loadStats } from './servers';

export async function loadProfile() {
  try {
    s.profile = await invoke<ProfileDto>('get_profile');
  } catch (e) {
    s.setStatus(`Failed to load profile: ${e}`, 'error');
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
) {
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
    });
    const tasks: Promise<unknown>[] = [loadProfile(), loadStats()];
    if (steamApiKey && steamId) {
      tasks.push(
        invoke<string | null>('fetch_steam_avatar')
          .then((url) => { s.avatarUrl = url; })
          .catch(() => {}),
      );
    } else {
      s.avatarUrl = null;
    }
    await Promise.all(tasks);
    s.setStatus('Settings saved', 'success');
  } catch (e) {
    s.setStatus(`Failed to save settings: ${e}`, 'error');
  }
}

export async function toggleOption(key: string) {
  const prev = s.profile?.options.find((o) => o.key === key)?.enabled;
  if (s.profile) {
    s.profile.options = s.profile.options.map((o) =>
      o.key === key ? { ...o, enabled: !o.enabled } : o
    );
  }
  try {
    await invoke<boolean>('toggle_launch_option', { key });
  } catch (e) {
    if (s.profile) {
      s.profile.options = s.profile.options.map((o) =>
        o.key === key ? { ...o, enabled: prev ?? o.enabled } : o
      );
    }
    s.setStatus(`Failed: ${e}`, 'error');
  }
}

export async function setOptionValue(key: string, value: string | null) {
  const prevOpt = s.profile?.options.find((o) => o.key === key);
  if (s.profile) {
    s.profile.options = s.profile.options.map((o) =>
      o.key === key ? { ...o, value, enabled: value !== null ? true : o.enabled } : o
    );
  }
  try {
    await invoke('set_launch_option_value', { key, value });
  } catch (e) {
    if (s.profile && prevOpt) {
      s.profile.options = s.profile.options.map((o) =>
        o.key === key ? { ...prevOpt } : o
      );
    }
    s.setStatus(`Failed: ${e}`, 'error');
  }
}
