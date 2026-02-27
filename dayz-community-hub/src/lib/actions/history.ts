import { invoke } from '@tauri-apps/api/core';
import type { HistoryDto } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import { loadProfile } from './profile';
import { profileAddFavorite } from './favorites';

export function removeHistoryEntry(h: HistoryDto) {
  s.confirmDialog = {
    title: 'Remove Entry',
    message: `Remove '${h.name}' from history?`,
    onConfirm: async () => {
      try {
        if (s.profile) s.profile.history = s.profile.history.filter((e) => !(e.ip === h.ip && e.port === h.port));
        await invoke('remove_history_entry', { ip: h.ip, port: h.port });
        s.setStatus('Removed from history', 'success');
      } catch (e) {
        await loadProfile();
        s.setStatus(`Failed: ${e}`, 'error');
      }
    },
  };
}

export function clearHistory() {
  s.confirmDialog = {
    title: 'Clear History',
    message: `Clear all ${s.profile?.history.length ?? 0} history entries?`,
    onConfirm: async () => {
      try {
        if (s.profile) s.profile.history = [];
        await invoke('clear_history');
        s.setStatus('History cleared', 'success');
      } catch (e) {
        await loadProfile();
        s.setStatus(`Failed: ${e}`, 'error');
      }
    },
  };
}

export async function addFavoriteFromHistory(h: HistoryDto) {
  try {
    profileAddFavorite(h.name, h.ip, h.port);
    await invoke('add_favorite', { name: h.name, ip: h.ip, port: h.port, password: null });
    s.setStatus(`Added ${h.name} to favorites`, 'success');
  } catch (e) {
    await loadProfile();
    s.setStatus(`Failed: ${e}`, 'error');
  }
}
