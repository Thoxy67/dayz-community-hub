import { invoke } from '@tauri-apps/api/core';
import { save as saveDialog, open as openDialog } from '@tauri-apps/plugin-dialog';
import { app as s } from '$lib/state.svelte';
import { loadProfile } from './profile';
import { loadStats } from './servers';

export async function exportProfile(includeMods: boolean) {
  const path = await saveDialog({
    title: 'Export profile',
    defaultPath: 'dayz-community-hub-profile.dchub',
    filters: [{ name: 'DayZ Community Hub profile', extensions: ['dchub'] }],
  });
  if (!path) return;
  try {
    await invoke('export_profile', { path, includeMods });
    s.setStatus('Profile exported successfully', 'success');
  } catch (e) {
    s.setStatus(`Export failed: ${e}`, 'error');
  }
}

export async function importProfile() {
  const selected = await openDialog({
    title: 'Import profile',
    multiple: false,
    filters: [{ name: 'DayZ Community Hub profile', extensions: ['dchub'] }],
  });
  if (!selected) return;
  const path = typeof selected === 'string' ? selected : selected[0];
  s.confirmDialog = {
    title: 'Import Profile',
    message: 'This will overwrite your current profile, favorites, history and launch options. The app will restart to apply the changes. Continue?',
    onConfirm: async () => {
      try {
        await invoke('import_profile', { path });
        await invoke('restart_app');
      } catch (e) {
        s.setStatus(`Import failed: ${e}`, 'error');
      }
    },
  };
}

export function resetProfile() {
  s.confirmDialog = {
    title: 'Reset to Factory Defaults',
    message: 'This will delete all settings, favorites, history, cached server lists, and launch options — as if the app was never launched. Mods downloaded to your Steam workshop folder are not affected. The app will restart and the setup wizard will reappear.',
    onConfirm: async () => {
      try {
        await invoke('reset_profile');
        await invoke('restart_app');
      } catch (e) {
        s.setStatus(`Reset failed: ${e}`, 'error');
      }
    },
  };
}
