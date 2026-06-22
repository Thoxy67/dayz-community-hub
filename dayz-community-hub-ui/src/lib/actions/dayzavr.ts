import { invoke, Channel } from '@tauri-apps/api/core';
import type { DayzavrServer } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import * as m from '$lib/paraglide/messages.js';
import { saveDayzavrSettings } from './profile';

/** Fetch the DayZavr community server list (cached in state). */
export async function loadDayzavrServers(force = false) {
  if (s.dayzavrLoading) return;
  if (!force && s.dayzavrServers.length > 0) return;
  s.dayzavrLoading = true;
  try {
    s.dayzavrServers = await invoke<DayzavrServer[]>('fetch_dayzavr_servers');
  } catch (e) {
    s.setStatus(m.dayzavr_load_failed({ error: String(e) }), 'error');
  } finally {
    s.dayzavrLoading = false;
  }
}

/** Progress event streamed from the Rust torrent installer. */
interface DayzavrInstallProgress {
  status: string;
  downloaded_bytes: number;
  total_bytes: number;
  uploaded_bytes: number;
  download_mbps: number;
  upload_mbps: number;
  peers_live: number;
  peers_connecting: number;
  peers_seen: number;
  eta: string | null;
  done: boolean;
  error: string | null;
}

/** Install/update the mods a DayZavr server requires into the configured DayZ path. */
export async function installDayzavrMods(server: DayzavrServer) {
  const dayzPath = s.profile?.dayzavr_dayz_path;
  if (!dayzPath) {
    s.setStatus(m.dayzavr_set_path_first(), 'warning');
    return;
  }
  if (s.dayzavrInstall?.active) return;

  s.dayzavrInstall = {
    active: true,
    serverName: server.name,
    status: m.dayzavr_install_starting(),
    downloadedBytes: 0,
    totalBytes: 0,
    uploadedBytes: 0,
    downloadMbps: 0,
    uploadMbps: 0,
    peersLive: 0,
    peersConnecting: 0,
    peersSeen: 0,
    eta: null,
    done: false,
    error: null,
  };

  const onProgress = new Channel<DayzavrInstallProgress>();
  onProgress.onmessage = (p) => {
    if (!s.dayzavrInstall) return;
    s.dayzavrInstall = {
      ...s.dayzavrInstall,
      status: p.status,
      downloadedBytes: p.downloaded_bytes,
      totalBytes: p.total_bytes,
      uploadedBytes: p.uploaded_bytes,
      downloadMbps: p.download_mbps,
      uploadMbps: p.upload_mbps,
      peersLive: p.peers_live,
      peersConnecting: p.peers_connecting,
      peersSeen: p.peers_seen,
      eta: p.eta,
      done: p.done,
      error: p.error,
      active: !p.done && !p.error,
    };
    // Refresh installed set when a download finishes successfully.
    if (p.done && !p.error) loadDayzavrInstalledMods();
  };

  try {
    await invoke('install_dayzavr_mods', { mods: server.mods, dayzPath, onProgress });
  } catch (e) {
    if (s.dayzavrInstall) {
      s.dayzavrInstall = { ...s.dayzavrInstall, active: false, error: String(e) };
    }
  }
}

/** Refresh the set of fully-installed DayZavr mods (gates the Play button). */
export async function loadDayzavrInstalledMods() {
  const dayzPath = s.profile?.dayzavr_dayz_path;
  if (!dayzPath) {
    s.dayzavrInstalledMods = [];
    return;
  }
  try {
    s.dayzavrInstalledMods = await invoke<string[]>('dayzavr_installed_mods', { dayzPath });
  } catch {
    /* non-fatal (e.g. offline) */
  }
}

/** Whether every mod a server requires is installed (so it can be joined). */
export function serverModsInstalled(server: DayzavrServer): boolean {
  const set = new Set(s.dayzavrInstalledMods);
  return server.mods.length > 0 && server.mods.every((m) => set.has(m));
}

/** Launch DayZ and connect to a DayZavr server (loads its mods from !Workshop). */
export async function joinDayzavrServer(server: DayzavrServer) {
  if (!s.profile?.dayzavr_dayz_path) {
    s.setStatus(m.dayzavr_set_path_first(), 'warning');
    return;
  }
  try {
    await invoke('launch_dayzavr_server', {
      host: server.host,
      gamePort: server.game_port,
      password: null,
      mods: server.mods,
    });
    s.setStatus(m.dayzavr_launching({ name: server.name }), 'info');
  } catch (e) {
    s.setStatus(m.dayzavr_launch_failed({ error: String(e) }), 'error');
  }
}

/** Cancel an in-flight DayZavr mod install. */
export async function cancelDayzavrInstall() {
  try {
    await invoke('cancel_dayzavr_install');
  } catch {
    /* non-fatal */
  }
  if (s.dayzavrInstall) s.dayzavrInstall = { ...s.dayzavrInstall, active: false };
}

/** Dismiss the install progress panel. */
export function dismissDayzavrInstall() {
  s.dayzavrInstall = null;
}

/** Remove all installed DayZavr mods from the configured DayZ folder (with confirm). */
export function clearDayzavrMods() {
  const dayzPath = s.profile?.dayzavr_dayz_path;
  if (!dayzPath) {
    s.setStatus(m.dayzavr_set_path_first(), 'warning');
    return;
  }
  s.confirmDialog = {
    title: m.dayzavr_clear_title(),
    message: m.dayzavr_clear_confirm(),
    confirmLabel: m.dayzavr_clear_action(),
    confirmVariant: 'error',
    onConfirm: async () => {
      try {
        const removed = await invoke<string[]>('clear_dayzavr_mods', { dayzPath });
        s.dayzavrInstalledMods = [];
        s.setStatus(m.dayzavr_clear_done({ count: removed.length }), 'success');
      } catch (e) {
        s.setStatus(m.dayzavr_clear_failed({ error: String(e) }), 'error');
      }
    },
  };
}

/** Persist the DayZ install path used for mod installation. */
export async function setDayzavrDayzPath(path: string | null) {
  await saveDayzavrSettings(s.profile?.dayzavr_enabled ?? true, path);
  loadDayzavrInstalledMods();
}

/** Auto-detect the DayZ install via Steam and persist it. Returns the path or null.
 * When `force` is false, does nothing if a path is already configured. */
export async function autodetectDayzPath(force = false): Promise<string | null> {
  if (!force && s.profile?.dayzavr_dayz_path) return s.profile.dayzavr_dayz_path;
  try {
    const path = await invoke<string | null>('detect_dayz_path');
    if (path) await setDayzavrDayzPath(path);
    return path;
  } catch {
    return null;
  }
}
