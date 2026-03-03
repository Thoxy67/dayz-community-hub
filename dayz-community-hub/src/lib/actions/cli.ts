import { invoke } from '@tauri-apps/api/core';
import type { CliArgs, DzchConfig } from '$lib/types';
import { app as s } from '$lib/state.svelte';
import { connectDirect, connectByAddress } from './connect';
import * as m from '$lib/paraglide/messages.js';

export async function handleCliArgs(args: CliArgs) {
  if (!s.initialized) {
    s.pendingCliArgs = args;
    return;
  }

  if (args.open) {
    await handleDzchOpen(args.open);
  } else if (args.connect) {
    const raw = args.connect.trim();
    const lastColon = raw.lastIndexOf(':');
    let ip = raw;
    let port = 2302;
    if (lastColon !== -1) {
      const maybPort = parseInt(raw.slice(lastColon + 1), 10);
      if (!isNaN(maybPort)) {
        ip = raw.slice(0, lastColon);
        port = maybPort;
      }
    }
    s.activeTab = 'connect';
    await new Promise((r) => setTimeout(r, 150));
    connectDirect(ip, port);
  } else if (args.reconnect) {
    const last = s.profile?.history?.[0] ?? null;
    if (last) {
      connectByAddress(last.ip, last.port, last.name);
    } else {
      s.setStatus(m.cli_no_history(), 'warning');
    }
  }
}

export async function handleDzchOpen(raw: string) {
  try {
    let config: DzchConfig;
    if (raw.startsWith('dzch://')) {
      config = await invoke<DzchConfig>('parse_dzch_url', { url: raw });
    } else {
      config = await invoke<DzchConfig>('read_dzch_file', { path: raw });
    }

    s.activeTab = 'connect';
    await new Promise((r) => setTimeout(r, 150));
    connectDirect(config.ip, config.port, config.password ?? undefined);
  } catch (e) {
    s.setStatus(m.cli_dzch_open_failed({ error: String(e) }), 'error');
  }
}
