import { invoke } from '@tauri-apps/api/core';
import type { ServerDto, ServerFullDto, ModDto } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import { startModOp } from './mods';

export async function connectToServer(server: ServerDto) {
  let fullServer: ServerFullDto | null = null;
  if (server.mods_count > 0) {
    try {
      fullServer = await invoke<ServerFullDto>('get_server_details', { ip: server.ip, port: server.query_port });
    } catch {
      /* non-fatal */
    }
  }

  const installedIds = new Set(s.installedMods.map((m) => m.id));
  const missingIds: number[] = (fullServer?.mods ?? [])
    .map((m: ModDto) => m.steam_workshop_id)
    .filter((id: number) => !installedIds.has(id));

  if (missingIds.length > 0) {
    s.connectRequest = {
      serverName: server.name,
      kind: 'missing',
      modCount: missingIds.length,
      onConnect: (updateMods: boolean) => {
        if (updateMods) doInstallAndLaunch(server);
        else doLaunchDirect(server);
      },
    };
  } else if (server.mods_count > 0) {
    s.connectRequest = {
      serverName: server.name,
      kind: 'update',
      modCount: server.mods_count,
      onConnect: (updateMods: boolean) => {
        if (updateMods) doUpdateAndLaunch(server);
        else doLaunchDirect(server);
      },
    };
  } else {
    doLaunchDirect(server);
  }
}

export function connectByAddress(ip: string, port: number, _name: string) {
  const server = s.servers.find((sv) => sv.ip === ip && (sv.query_port === port || sv.game_port === port));
  if (server) {
    connectToServer(server);
  } else {
    connectDirect(ip, port);
  }
}

export async function connectDirect(ip: string, port: number, password?: string, extraArgs?: string[]) {
  const inList = s.servers.find((sv) => sv.ip === ip && (sv.query_port === port || sv.game_port === port));

  let serverMods: ModDto[] = [];
  if (inList && inList.mods_count > 0) {
    try {
      const full = await invoke<ServerFullDto>('get_server_details', {
        ip,
        port: inList.query_port,
      });
      serverMods = full.mods;
    } catch {
      /* non-fatal */
    }
  }

  if (serverMods.length === 0) {
    doLaunchDirectByAddress(ip, port, password, extraArgs);
    return;
  }

  const installedIds = new Set(s.installedMods.map((m) => m.id));
  const missingIds = serverMods.map((m) => m.steam_workshop_id).filter((id) => !installedIds.has(id));

  const serverName = inList?.name ?? `${ip}:${port}`;

  const allModIds = serverMods.map((m) => m.steam_workshop_id);
  const allModNames = serverMods.map((m) => m.name || String(m.steam_workshop_id));
  const missingNames = serverMods
    .filter((m) => !installedIds.has(m.steam_workshop_id))
    .map((m) => m.name || String(m.steam_workshop_id));

  if (missingIds.length > 0) {
    s.connectRequest = {
      serverName,
      kind: 'missing',
      modCount: missingIds.length,
      onConnect: (updateMods: boolean) => {
        if (updateMods) {
          startModOp('update_selected', { modIds: missingIds, modNames: missingNames }, () =>
            doLaunchDirectByAddress(ip, port, password, extraArgs),
          );
        } else {
          doLaunchDirectByAddress(ip, port, password, extraArgs);
        }
      },
    };
  } else if (serverMods.length > 0) {
    s.connectRequest = {
      serverName,
      kind: 'update',
      modCount: serverMods.length,
      onConnect: (updateMods: boolean) => {
        if (updateMods) {
          startModOp('update_selected', { modIds: allModIds, modNames: allModNames }, () =>
            doLaunchDirectByAddress(ip, port, password, extraArgs),
          );
        } else {
          doLaunchDirectByAddress(ip, port, password, extraArgs);
        }
      },
    };
  } else {
    doLaunchDirectByAddress(ip, port, password, extraArgs);
  }
}

export async function doLaunchDirect(server: ServerDto) {
  s.setStatus(`Launching ${server.name}…`, 'info');
  try {
    await invoke('setup_mod_symlinks', { ip: server.ip, port: server.query_port }).catch(() => {});
    await invoke('launch_server', { ip: server.ip, port: server.query_port, password: null });
    s.setStatus('Waiting for Steam to open DayZ…', 'info');
  } catch (e) {
    s.setStatus(`Launch failed: ${e}`, 'error');
  }
}

export async function doInstallAndLaunch(server: ServerDto) {
  startModOp('install_server', { ip: server.ip, port: server.query_port }, () => doLaunchDirect(server));
}

export async function doUpdateAndLaunch(server: ServerDto) {
  startModOp('update_server', { ip: server.ip, port: server.query_port }, () => doLaunchDirect(server));
}

export async function doLaunchDirectByAddress(ip: string, port: number, password?: string, extraArgs?: string[]) {
  s.setStatus(`Launching ${ip}:${port}…`, 'info');
  try {
    await invoke('launch_direct', {
      ip,
      gamePort: port,
      password: password ?? null,
      extraArgs: extraArgs ?? null,
    });
    s.setStatus('Waiting for Steam to open DayZ…', 'info');
  } catch (e) {
    s.setStatus(`Launch failed: ${e}`, 'error');
  }
}

export function openInDirectConnect(ip: string, gamePort: number, queryPort?: number) {
  s.directConnectPrefill = { ip, port: gamePort, queryPort };
  s.activeTab = 'connect';
}
