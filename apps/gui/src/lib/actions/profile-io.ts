import { invoke } from '@tauri-apps/api/core';
import { save as saveDialog, open as openDialog } from '@tauri-apps/plugin-dialog';
import { app as s } from '$lib/state.svelte';
import { loadProfile } from './profile';
import { loadStats } from './servers';
import { confirmAction } from '$lib/utils/dialog-helpers';
import * as m from '$lib/paraglide/messages.js';

export async function exportProfile(includeMods: boolean) {
  const path = await saveDialog({
    title: m.profile_export_title(),
    defaultPath: 'dayz-community-hub-profile.dchub',
    filters: [{ name: m.profile_filter_name(), extensions: ['dchub'] }],
  });
  if (!path) return;
  try {
    await invoke('export_profile', { path, includeMods });
    s.setStatus(m.profile_exported(), 'success');
  } catch (e) {
    s.setStatus(m.profile_export_failed({ error: String(e) }), 'error');
  }
}

export async function importProfile() {
  const selected = await openDialog({
    title: m.profile_import_title(),
    multiple: false,
    filters: [{ name: m.profile_filter_name(), extensions: ['dchub'] }],
  });
  if (!selected) return;
  const path = typeof selected === 'string' ? selected : selected[0];
  confirmAction(
    m.profile_import_confirm_title(),
    m.profile_import_confirm_message(),
    async () => {
      await invoke('import_profile', { path });
      await invoke('restart_app');
    },
    '',
    m.profile_import_failed(),
  );
}

export function resetProfile() {
  confirmAction(
    m.profile_reset_title(),
    m.profile_reset_message(),
    async () => {
      await invoke('reset_profile');
      await invoke('restart_app');
    },
    '',
    m.profile_reset_failed(),
  );
}
