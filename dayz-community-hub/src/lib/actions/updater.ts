import { invoke, Channel } from '@tauri-apps/api/core';
import { app as s } from '$lib/state.svelte';
import type { UpdateInfo, DownloadEvent } from '$lib/state.svelte';

export async function checkForUpdate() {
  s.updateState = 'checking';
  s.updateError = '';
  try {
    const info = await invoke<UpdateInfo | null>('check_for_update');
    if (info) {
      s.updateInfo = info;
      s.updateState = 'available';
    } else {
      s.updateState = 'up_to_date';
    }
  } catch (e) {
    s.updateError = String(e);
    s.updateState = 'error';
  }
}

export async function installUpdate() {
  s.updateState = 'downloading';
  s.dlReceived = 0;
  s.dlTotal = 0;
  s.updateError = '';
  const onEvent = new Channel<DownloadEvent>();
  onEvent.onmessage = (ev) => {
    if (ev.event === 'Started') {
      s.dlTotal = ev.data.contentLength ?? 0;
    } else if (ev.event === 'Progress') {
      s.dlReceived += ev.data.chunkLength;
    } else if (ev.event === 'Finished') {
      s.updateState = 'done';
    }
  };
  try {
    await invoke('install_update', { onEvent });
  } catch (e) {
    s.updateError = String(e);
    s.updateState = 'error';
  }
}
