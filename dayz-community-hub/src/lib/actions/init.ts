import { invoke } from '@tauri-apps/api/core';
import type { TabId } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import { loadServers, refreshServersBackground, startPinging, loadStats, loadSteamPlayers } from './servers';
import { loadProfile } from './profile';
import { loadMods, checkModUpdates } from './mods';
import { loadNews } from './news';
import { loadOfflineMissions } from './offline';
import { checkForUpdate } from './updater';
import { handleCliArgs } from './cli';

export async function doInitialize() {
  try {
    const result = await invoke<{ server_count: number; from_cache: boolean; is_first_launch: boolean }>('initialize');
    s.initialized = true;
    if (result.is_first_launch) s.showWizard = true;

    await Promise.all([loadProfile(), loadStats(), loadSteamPlayers(), loadMods()]);

    if (s.profile?.steam_api_key) checkModUpdates();

    checkForUpdate();

    if (s.pendingCliArgs) {
      const queued = s.pendingCliArgs;
      s.pendingCliArgs = null;
      handleCliArgs(queued);
    }

    if (s.profile?.steam_api_key && s.profile?.steam_id) {
      invoke<string | null>('fetch_steam_avatar').then((url) => { s.avatarUrl = url; }).catch(() => {});
    }

    s.serversLoading = true;
    const fromCache = result.from_cache;
    loadServers().then(() => {
      s.serversLoading = false;
      startPinging();
      if (fromCache) {
        refreshServersBackground();
      }
    });
  } catch (e) {
    s.initError = String(e);
    s.setStatus(`Initialization failed: ${e}`, 'error');
  }
}

export function selectTab(id: TabId) {
  s.activeTab = id;
  if (id === 'mods') {
    if (s.installedMods.length === 0) loadMods().then(() => { if (s.profile?.steam_api_key) checkModUpdates(); });
    else if (s.profile?.steam_api_key) checkModUpdates();
  }
  if (id === 'news' && s.articles.length === 0) loadNews();
  if (id === 'offline' && s.offlineMissions.length === 0) loadOfflineMissions();
}
