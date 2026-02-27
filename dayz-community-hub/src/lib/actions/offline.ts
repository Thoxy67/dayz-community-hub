import { invoke } from '@tauri-apps/api/core';
import { app as s } from '$lib/state.svelte';

export async function loadOfflineMissions() {
  s.offlineLoading = true;
  s.offlineStatus = '';
  try {
    s.offlineMissions = await invoke<string[]>('get_offline_missions');
    if (s.offlineMissions.length === 0) {
      s.offlineStatus = "No missions found. Click 'Install / Update' to download.";
      s.offlineStatusKind = 'warning';
    } else {
      s.offlineStatus = `${s.offlineMissions.length} mission(s) available`;
      s.offlineStatusKind = 'success';
    }
  } catch (e) {
    s.offlineStatus = String(e);
    s.offlineStatusKind = 'error';
    s.offlineMissions = [];
  } finally {
    s.offlineLoading = false;
  }
}

export async function updateOfflineMode() {
  s.offlineLoading = true;
  s.offlineStatus = 'Downloading DayZCommunityOfflineMode…';
  s.offlineStatusKind = 'info';
  await invoke('update_offline_mode').catch((e) => {
    s.offlineStatus = String(e);
    s.offlineStatusKind = 'error';
    s.offlineLoading = false;
  });
}

export async function launchOfflineMission(mission: string) {
  s.setStatus(`Launching offline: ${mission}`, 'info');
  invoke('launch_offline_mission', { mission }).catch((e) =>
    s.setStatus(`Launch failed: ${e}`, 'error')
  );
}

export function removeOfflineMode() {
  s.confirmDialog = {
    title: 'Remove Offline Mode',
    message: 'Delete all DayZCommunityOfflineMode mission folders?\nThis cannot be undone. You can reinstall with "Install / Update".',
    confirmLabel: 'Remove all',
    confirmVariant: 'error',
    onConfirm: async () => {
      try {
        s.offlineLoading = true;
        s.offlineStatus = 'Removing offline mode…';
        s.offlineStatusKind = 'info';
        const n = await invoke<number>('remove_offline_mode');
        await loadOfflineMissions();
        s.offlineStatus = n > 0 ? `Removed ${n} mission folder${n > 1 ? 's' : ''}` : 'Nothing to remove';
        s.offlineStatusKind = 'success';
      } catch (e) {
        s.offlineStatus = `Remove failed: ${e}`;
        s.offlineStatusKind = 'error';
        s.offlineLoading = false;
      }
    },
  };
}

export function clearOfflineSaves() {
  s.confirmDialog = {
    title: 'Clear Saves',
    message: 'Delete all offline save data (storage_-1/) for every mission?\nThis wipes loot, player state and world state. The missions themselves are kept.',
    confirmLabel: 'Clear saves',
    confirmVariant: 'warning',
    onConfirm: async () => {
      try {
        const n = await invoke<number>('clear_offline_saves');
        s.offlineStatus = n > 0 ? `Cleared saves for ${n} mission${n > 1 ? 's' : ''}` : 'No saves found';
        s.offlineStatusKind = 'success';
      } catch (e) {
        s.offlineStatus = `Clear failed: ${e}`;
        s.offlineStatusKind = 'error';
      }
    },
  };
}
