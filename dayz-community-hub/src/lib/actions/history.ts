import { invoke } from '@tauri-apps/api/core';
import type { HistoryDto } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import { loadProfile } from './profile';
import { profileAddFavorite } from './favorites';
import { withProfileFallback } from '$lib/utils/action-helpers';
import { confirmActionWithProfile } from '$lib/utils/dialog-helpers';
import * as m from '$lib/paraglide/messages.js';

export function removeHistoryEntry(h: HistoryDto) {
  confirmActionWithProfile(
    m.history_remove_title(),
    m.history_remove_message({ name: h.name }),
    async () => {
      if (s.profile) s.profile.history = s.profile.history.filter((e) => !(e.ip === h.ip && e.port === h.port));
      await invoke('remove_history_entry', { ip: h.ip, port: h.port });
    },
    m.history_removed(),
  );
}

export function clearHistory() {
  confirmActionWithProfile(
    m.history_clear_title(),
    m.history_clear_message({ count: s.profile?.history.length ?? 0 }),
    async () => {
      if (s.profile) s.profile.history = [];
      await invoke('clear_history');
    },
    m.history_cleared(),
  );
}

export async function addFavoriteFromHistory(h: HistoryDto) {
  await withProfileFallback(
    async () => {
      profileAddFavorite(h.name, h.ip, h.port);
      await invoke('add_favorite', { name: h.name, ip: h.ip, port: h.port, password: null });
    },
    m.history_added_to_favorites({ name: h.name }),
  );
}
