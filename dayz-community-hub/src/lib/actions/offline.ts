import { invoke } from '@tauri-apps/api/core';
import { app as s } from '$lib/state.svelte';
import * as m from '$lib/paraglide/messages.js';

export async function loadOfflineMissions() {
  s.offlineLoading = true;
  s.offlineStatus = '';
  try {
    s.offlineMissions = await invoke<string[]>('get_offline_missions');
    if (s.offlineMissions.length === 0) {
      s.offlineStatus = m.offline_status_no_missions();
      s.offlineStatusKind = 'warning';
    } else {
      const count = s.offlineMissions.length;
      s.offlineStatus = count === 1 ? m.offline_status_available_one({ count }) : m.offline_status_available({ count });
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
  s.offlineStatus = m.offline_status_downloading();
  s.offlineStatusKind = 'info';
  await invoke('update_offline_mode').catch((e) => {
    s.offlineStatus = String(e);
    s.offlineStatusKind = 'error';
    s.offlineLoading = false;
  });
}

export async function launchOfflineMission(mission: string) {
  s.setStatus(m.offline_status_launching({ mission }), 'info');
  invoke('launch_offline_mission', { mission }).catch((e) =>
    s.setStatus(m.offline_status_launch_failed({ error: String(e) }), 'error'),
  );
}

export function removeOfflineMode() {
  s.confirmDialog = {
    title: m.offline_dialog_remove_title(),
    message: m.offline_dialog_remove_message(),
    confirmLabel: m.offline_dialog_remove_confirm(),
    confirmVariant: 'error',
    onConfirm: async () => {
      try {
        s.offlineLoading = true;
        s.offlineStatus = m.offline_status_removing_all();
        s.offlineStatusKind = 'info';
        const n = await invoke<number>('remove_offline_mode');
        await loadOfflineMissions();
        s.offlineStatus =
          n > 0
            ? n === 1
              ? m.offline_status_removed_folders_one({ count: n })
              : m.offline_status_removed_folders({ count: n })
            : m.offline_status_nothing_to_remove();
        s.offlineStatusKind = 'success';
      } catch (e) {
        s.offlineStatus = m.offline_status_remove_failed({ error: String(e) });
        s.offlineStatusKind = 'error';
        s.offlineLoading = false;
      }
    },
  };
}

export function clearOfflineSaves() {
  s.confirmDialog = {
    title: m.offline_dialog_clear_title(),
    message: m.offline_dialog_clear_message(),
    confirmLabel: m.offline_dialog_clear_confirm(),
    confirmVariant: 'warning',
    onConfirm: async () => {
      try {
        const n = await invoke<number>('clear_offline_saves');
        s.offlineStatus =
          n > 0
            ? n === 1
              ? m.offline_status_cleared_saves_one({ count: n })
              : m.offline_status_cleared_saves({ count: n })
            : m.offline_status_no_saves();
        s.offlineStatusKind = 'success';
      } catch (e) {
        s.offlineStatus = m.offline_status_clear_failed({ error: String(e) });
        s.offlineStatusKind = 'error';
      }
    },
  };
}

export function removeMission(mission: string) {
  s.confirmDialog = {
    title: m.offline_dialog_remove_mission_title(),
    message: m.offline_dialog_remove_mission_message({ mission }),
    confirmLabel: m.offline_dialog_remove_mission_confirm(),
    confirmVariant: 'error',
    onConfirm: async () => {
      try {
        s.offlineLoading = true;
        s.offlineStatus = m.offline_status_removing_mission({ mission });
        s.offlineStatusKind = 'info';
        await invoke('remove_mission', { mission });
        await loadOfflineMissions();
        s.offlineStatus = m.offline_status_removed_mission({ mission });
        s.offlineStatusKind = 'success';
      } catch (e) {
        s.offlineStatus = m.offline_status_remove_failed({ error: String(e) });
        s.offlineStatusKind = 'error';
        s.offlineLoading = false;
      }
    },
  };
}
