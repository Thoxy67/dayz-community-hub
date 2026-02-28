import { invoke, Channel } from '@tauri-apps/api/core';
import type { InstalledModDto, ModProgressEvent } from '$lib/types';
import { app as s, MOD_UPDATES_TTL_MS } from '$lib/state.svelte';
import { loadStats } from './servers';

export async function loadMods() {
  s.modsLoading = true;
  try {
    s.installedMods = await invoke<InstalledModDto[]>('get_installed_mods');
  } catch (e) {
    s.setStatus(`Failed to load mods: ${e}`, 'error');
  } finally {
    s.modsLoading = false;
  }
}

export async function checkModUpdates(force = false) {
  const now = Date.now();
  if (!force && now - s.modUpdatesLastChecked < MOD_UPDATES_TTL_MS) return;
  s.modsChecking = true;
  try {
    s.installedMods = await invoke<InstalledModDto[]>('check_mod_updates');
    s.modUpdatesLastChecked = Date.now();
  } catch (e) {
    s.setStatus(`Update check failed: ${e}`, 'error');
  } finally {
    s.modsChecking = false;
  }
}

export function deleteMod(mod_item: InstalledModDto) {
  s.confirmDialog = {
    title: 'Delete Mod',
    message: `Delete '${mod_item.name}' (${mod_item.id})?\nSize: ${mod_item.size_human}`,
    onConfirm: async () => {
      await invoke('delete_mod', { modId: mod_item.id });
      await loadMods();
      s.setStatus(`Deleted ${mod_item.name}`, 'success');
    },
  };
}

export async function toggleModManaged(mod_item: InstalledModDto) {
  try {
    const managed = await invoke<boolean>('toggle_mod_managed', { modId: mod_item.id });
    await loadMods();
    s.setStatus(`${mod_item.name} ${managed ? 'linked (symlink created)' : 'unlinked (symlink removed)'}`, 'success');
  } catch (e) {
    s.setStatus(`Failed: ${e}`, 'error');
  }
}

export function updateMod(mod_item: InstalledModDto) {
  startModOp('update_one', { modId: mod_item.id, modName: mod_item.name });
}

export function updateAllMods() {
  startModOp('update_all', {});
}

export function updateStaleMods() {
  startModOp('update_stale', {});
}

export function cleanupMods() {
  s.confirmDialog = {
    title: 'Cleanup Mods',
    message: 'Remove all managed mods and symlinks?',
    onConfirm: async () => {
      try {
        const result = await invoke<string>('cleanup_mods');
        await loadMods();
        s.setStatus(result, 'success');
      } catch (e) {
        s.setStatus(`Cleanup failed: ${e}`, 'error');
      }
    },
  };
}

export function deleteSelectedMods(ids: number[]) {
  if (ids.length === 0) return;
  const totalSize = s.installedMods.filter((m) => ids.includes(m.id)).reduce((acc, m) => acc + m.size, 0);
  const sizeMb = totalSize / 1024 / 1024;
  const sizeStr = sizeMb >= 1024 ? `${(sizeMb / 1024).toFixed(1)} GB` : `${sizeMb.toFixed(1)} MB`;
  s.confirmDialog = {
    title: 'Delete Mods',
    message: `Delete ${ids.length} mod${ids.length > 1 ? 's' : ''}?\nTotal size: ${sizeStr}`,
    confirmLabel: `Delete ${ids.length}`,
    confirmVariant: 'error',
    onConfirm: async () => {
      try {
        await invoke('delete_mods_bulk', { modIds: ids });
        await loadMods();
        s.setStatus(`Deleted ${ids.length} mod${ids.length > 1 ? 's' : ''}`, 'success');
      } catch (e) {
        s.setStatus(`Delete failed: ${e}`, 'error');
      }
    },
  };
}

export function updateSelectedMods(ids: number[]) {
  if (ids.length === 0) return;
  startModOp('update_selected', { modIds: ids });
}

export function installMods(workshopIds: number[]) {
  if (workshopIds.length === 0) return;
  startModOp('install_manual', {
    modIds: workshopIds,
    modNames: workshopIds.map(String),
  });
}

// ── Mod operation progress via Channel ────────────────────────────────────
export function startModOp(opType: string, args: Record<string, unknown>, onSuccess?: () => void) {
  s.modOp = {
    active: true,
    phase: 'downloading',
    current: 0,
    total: 0,
    currentName: 'Preparing…',
    completed: [],
    ok: 0,
    failed: 0,
    hint: null,
    log: [],
  };

  const onProgress = new Channel<ModProgressEvent>();
  onProgress.onmessage = (payload) => {
    switch (payload.kind) {
      case 'shutting_down_steam':
        s.modOp.phase = 'shutting_down';
        s.modOp.currentName = 'Closing Steam…';
        break;
      case 'steam_guard_mobile_required':
        s.modOp.phase = 'steam_guard_mobile';
        break;
      case 'password_required':
        s.modOp.phase = 'password_required';
        break;
      case 'starting':
        s.modOp.phase = 'downloading';
        s.modOp.current = payload.current;
        s.modOp.total = payload.total;
        s.modOp.currentName = payload.name;
        break;
      case 'done':
        s.modOp.current = payload.current;
        s.modOp.total = payload.total;
        s.modOp.completed = [...s.modOp.completed, { id: payload.mod_id, name: payload.name, ok: true }];
        break;
      case 'failed':
        s.modOp.current = payload.current;
        s.modOp.total = payload.total;
        s.modOp.completed = [...s.modOp.completed, { id: payload.mod_id, name: payload.name, ok: false }];
        break;
      case 'log_line':
        if (payload.log_line) {
          s.modOp.log = [...s.modOp.log, payload.log_line];
        }
        break;
      case 'finished':
        s.modOp.phase = 'finished';
        s.modOp.ok = payload.ok;
        s.modOp.failed = payload.failed;
        s.modOp.hint = payload.hint;
        if (!payload.hint && payload.failed === 0) {
          s.setStatus(`Mods: ${payload.ok} updated successfully`, 'success');
          onSuccess?.();
        } else if (payload.failed > 0) {
          s.setStatus(`Mods: ${payload.ok} OK, ${payload.failed} failed`, 'warning');
        }
        break;
    }
  };

  invoke('start_mod_operation', { opType, ...args, onProgress }).catch((e) => {
    s.setStatus(`Mod operation failed: ${e}`, 'error');
    s.modOp.active = false;
  });
}

export function dismissModOp() {
  s.modOp.active = false;
  if (s.profile?.steam_api_key) checkModUpdates(true);
  else loadMods();
  loadStats();
}

export async function sendSteamcmdPassword(password: string) {
  try {
    await invoke('send_steamcmd_input', { input: password });
  } catch (e) {
    s.setStatus(`Failed to send password: ${e}`, 'error');
  }
}

export async function cancelModOperation() {
  try {
    await invoke('cancel_mod_operation');
  } catch (e) {
    console.warn('cancel_mod_operation:', e);
  }
  dismissModOp();
}
