import { invoke, Channel } from '@tauri-apps/api/core';
import type { InstalledModDto, ModProgressEvent } from '$lib/types';
import { app as s, MOD_UPDATES_TTL_MS } from '$lib/state.svelte';
import { loadStats } from './servers';
import { confirmAction } from '$lib/utils/dialog-helpers';
import * as m from '$lib/paraglide/messages.js';

export async function loadMods() {
  s.modsLoading = true;
  try {
    s.installedMods = await invoke<InstalledModDto[]>('get_installed_mods');
  } catch (e) {
    s.setStatus(m.mods_load_failed({ error: String(e) }), 'error');
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
    s.setStatus(m.mods_update_check_failed({ error: String(e) }), 'error');
  } finally {
    s.modsChecking = false;
  }
}

export function deleteMod(mod_item: InstalledModDto) {
  confirmAction(
    m.mods_delete_single_title(),
    m.mods_delete_single_message({ name: mod_item.name, id: String(mod_item.id), size: mod_item.size_human }),
    async () => {
      await invoke('delete_mod', { modId: mod_item.id });
      await loadMods();
    },
    m.mods_deleted_single({ name: mod_item.name }),
  );
}

export async function toggleModManaged(mod_item: InstalledModDto) {
  try {
    const managed = await invoke<boolean>('toggle_mod_managed', { modId: mod_item.id });
    await loadMods();
    const message = managed ? m.mods_linked({ name: mod_item.name }) : m.mods_unlinked({ name: mod_item.name });
    s.setStatus(message, 'success');
  } catch (e) {
    s.setStatus(m.mods_toggle_failed({ error: String(e) }), 'error');
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
  confirmAction(
    m.mods_cleanup_title(),
    m.mods_cleanup_message(),
    async () => {
      const result = await invoke<string>('cleanup_mods');
      await loadMods();
      s.setStatus(result, 'success');
    },
    '',
    m.mods_cleanup_failed(),
  );
}

export function deleteSelectedMods(ids: number[]) {
  if (ids.length === 0) return;
  const totalSize = s.installedMods.filter((m) => ids.includes(m.id)).reduce((acc, m) => acc + m.size, 0);
  const sizeMb = totalSize / 1024 / 1024;
  const sizeStr = sizeMb >= 1024 ? `${(sizeMb / 1024).toFixed(1)} GB` : `${sizeMb.toFixed(1)} MB`;
  const message =
    ids.length > 1
      ? m.mods_delete_selected_message_plural({ count: ids.length, size: sizeStr })
      : m.mods_delete_selected_message({ count: ids.length, size: sizeStr });
  const successMsg =
    ids.length > 1
      ? m.mods_deleted_selected_plural({ count: ids.length })
      : m.mods_deleted_selected({ count: ids.length });
  confirmAction(
    m.mods_delete_selected_title(),
    message,
    async () => {
      await invoke('delete_mods_bulk', { modIds: ids });
      await loadMods();
    },
    successMsg,
    m.mods_delete_failed(),
    m.mods_delete_button({ count: ids.length }),
    'error',
  );
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
    currentName: m.mods_preparing(),
    completed: [],
    ok: 0,
    failed: 0,
    hint: null,
    log: [],
  };

  const onProgress = new Channel<ModProgressEvent>();
  // True when the last appended log entry was a transient '\r' progress line,
  // so the next progress update overwrites it in place instead of appending.
  let lastWasProgress = false;
  onProgress.onmessage = (payload) => {
    switch (payload.kind) {
      case 'shutting_down_steam':
        s.modOp.phase = 'shutting_down';
        s.modOp.currentName = m.mods_closing_steam();
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
          lastWasProgress = false;
        }
        break;
      case 'log_progress':
        if (payload.log_line) {
          // Overwrite the previous transient progress line in place; otherwise
          // append a fresh one. Keeps download progress to a single live line.
          s.modOp.log = lastWasProgress
            ? [...s.modOp.log.slice(0, -1), payload.log_line]
            : [...s.modOp.log, payload.log_line];
          lastWasProgress = true;
        }
        break;
      case 'finished':
        s.modOp.phase = 'finished';
        s.modOp.ok = payload.ok;
        s.modOp.failed = payload.failed;
        s.modOp.hint = payload.hint;
        if (!payload.hint && payload.failed === 0) {
          s.setStatus(m.mods_updated_successfully({ count: payload.ok }), 'success');
          onSuccess?.();
        } else if (payload.failed > 0) {
          s.setStatus(m.mods_update_results({ ok: payload.ok, failed: payload.failed }), 'warning');
        }
        break;
    }
  };

  invoke('start_mod_operation', { opType, ...args, onProgress }).catch((e) => {
    s.setStatus(m.mods_operation_failed({ error: String(e) }), 'error');
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
    s.setStatus(m.mods_password_send_failed({ error: String(e) }), 'error');
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
