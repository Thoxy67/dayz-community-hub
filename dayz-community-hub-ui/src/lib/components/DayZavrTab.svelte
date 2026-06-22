<script lang="ts">
  import Icon from '@iconify/svelte';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import * as m from '$lib/paraglide/messages.js';
  import { app as s } from '$lib/state.svelte';
  import { playerFill, playerBarColor } from '$lib/utils';
  import { createSimpleCopyState } from '$lib/utils/clipboard.svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import {
    loadDayzavrServers,
    loadDayzavrInstalledMods,
    serverModsInstalled,
    installDayzavrMods,
    joinDayzavrServer,
    cancelDayzavrInstall,
    dismissDayzavrInstall,
    setDayzavrDayzPath,
    autodetectDayzPath,
    clearDayzavrMods,
  } from '$lib/actions/dayzavr';
  import type { DayzavrServer } from '$lib/types';

  // Load servers + auto-detect the DayZ path on first mount, then check which
  // mods are installed (so we know which servers are joinable).
  loadDayzavrServers();
  autodetectDayzPath().then(loadDayzavrInstalledMods);
  let detecting = $state(false);
  async function detect() {
    detecting = true;
    try {
      const path = await autodetectDayzPath(true);
      if (!path) s.setStatus(m.dayzavr_detect_failed(), 'warning');
    } finally {
      detecting = false;
    }
  }

  const cp = createSimpleCopyState();
  let expanded = $state<number | null>(null);

  let dayzPath = $derived(s.profile?.dayzavr_dayz_path ?? null);
  let install = $derived(s.dayzavrInstall);
  // Sorted by name, numeric-aware so "2." sorts before "13.".
  let sortedServers = $derived(
    s.dayzavrServers.slice().sort((a, b) => a.name.localeCompare(b.name, undefined, { numeric: true })),
  );

  function pct(s: DayzavrServer): number {
    return s.max_players > 0 ? Math.round((s.players / s.max_players) * 100) : 0;
  }
  function fmtBytes(b: number): string {
    if (b >= 1024 ** 3) return `${(b / 1024 ** 3).toFixed(1)} GB`;
    if (b >= 1024 ** 2) return `${(b / 1024 ** 2).toFixed(0)} MB`;
    return `${(b / 1024).toFixed(0)} KB`;
  }
  function installPct(): number {
    if (!install || install.totalBytes === 0) return 0;
    return Math.min(100, Math.round((install.downloadedBytes / install.totalBytes) * 100));
  }
  function fmtSpeed(mbps: number): string {
    return mbps >= 1 ? `${mbps.toFixed(1)} MiB/s` : `${(mbps * 1024).toFixed(0)} KiB/s`;
  }

  async function browsePath() {
    const sel = await openDialog({ directory: true, multiple: false, title: m.dayzavr_browse() });
    if (typeof sel === 'string') setDayzavrDayzPath(sel);
  }
</script>

<div class="flex flex-col h-full overflow-hidden">
  <!-- Toolbar -->
  <div class="flex-shrink-0 flex flex-col gap-2 px-4 py-2.5 bg-base-200 border-b border-base-300">
    <div class="flex items-center gap-3">
      <span class="flex items-center gap-1.5 text-sm font-semibold">
        <Icon icon="ph:skull" class="size-4 text-primary" />
        DayZavr
      </span>
      <span class="text-xs text-base-content/50">{m.dayzavr_count({ count: s.dayzavrServers.length })}</span>
      <button
        class="btn btn-ghost btn-xs gap-1"
        onclick={() => loadDayzavrServers(true)}
        disabled={s.dayzavrLoading}
        title={m.dayzavr_refresh()}
      >
        {#if s.dayzavrLoading}
          <span class="loading loading-spinner loading-xs"></span>
        {:else}
          <Icon icon="ph:arrows-clockwise" class="size-3.5" />
        {/if}
        {m.dayzavr_refresh()}
      </button>
      <button
        class="btn btn-ghost btn-xs gap-1 text-error/60 hover:text-error"
        onclick={clearDayzavrMods}
        title={m.dayzavr_clear_title()}
      >
        <Icon icon="ph:trash" class="size-3.5" />
        {m.dayzavr_clear_action()}
      </button>

      <!-- DayZ install path -->
      <div class="ml-auto flex items-center gap-1.5 min-w-0">
        <span class="text-xs text-base-content/50 shrink-0">{m.dayzavr_dayz_path_label()}</span>
        <input
          type="text"
          class="input input-xs input-bordered w-64 font-mono text-xs"
          placeholder={m.dayzavr_dayz_path_placeholder()}
          value={dayzPath ?? ''}
          onchange={(e) => setDayzavrDayzPath(e.currentTarget.value.trim() || null)}
        />
        <button
          class="btn btn-ghost btn-xs btn-square"
          onclick={detect}
          disabled={detecting}
          title={m.dayzavr_detect()}
        >
          {#if detecting}
            <span class="loading loading-spinner loading-xs"></span>
          {:else}
            <Icon icon="ph:magic-wand" class="size-3.5" />
          {/if}
        </button>
        <button class="btn btn-ghost btn-xs btn-square" onclick={browsePath} title={m.dayzavr_browse()}>
          <Icon icon="ph:folder-open" class="size-3.5" />
        </button>
      </div>
    </div>

    <!-- Disclaimer: legitimate use only -->
    <p class="flex items-start gap-1.5 text-[11px] text-base-content/45 leading-snug">
      <Icon icon="ph:info" class="size-3.5 shrink-0 mt-px" />
      {m.dayzavr_disclaimer()}
    </p>
  </div>

  <!-- Install progress panel -->
  {#if install}
    <div class="flex-shrink-0 px-4 py-2.5 border-b border-primary/30 bg-primary/5">
      <div class="flex items-center gap-2 text-xs">
        {#if install.error}
          <Icon icon="ph:warning-circle" class="size-4 text-error shrink-0" />
          <span class="text-error flex-1 truncate">{install.error}</span>
        {:else if install.done}
          <Icon icon="ph:check-circle" class="size-4 text-success shrink-0" />
          <span class="flex-1 truncate text-success">{m.dayzavr_install_done({ name: install.serverName })}</span>
        {:else}
          <span class="loading loading-spinner loading-xs text-primary shrink-0"></span>
          <span class="flex-1 truncate"
            >{install.serverName} — {install.status}
            {#if install.totalBytes > 0}
              ({fmtBytes(install.downloadedBytes)} / {fmtBytes(install.totalBytes)}){/if}</span
          >
        {/if}
        <span class="font-mono tabular-nums text-base-content/60">{installPct()}%</span>
        {#if install.active}
          <button class="btn btn-ghost btn-xs text-error/70" onclick={cancelDayzavrInstall}>{m.dayzavr_cancel()}</button>
        {:else}
          <button class="btn btn-ghost btn-xs" onclick={dismissDayzavrInstall}>
            <Icon icon="ph:x" class="size-3.5" />
          </button>
        {/if}
      </div>
      <div class="mt-1.5 h-1 w-full rounded-full bg-base-300 overflow-hidden">
        <div
          class="h-full rounded-full transition-[width] duration-300 {install.error
            ? 'bg-error'
            : install.done
              ? 'bg-success'
              : 'bg-primary'}"
          style="width:{installPct()}%"
        ></div>
      </div>
      <!-- Live transfer metrics -->
      {#if install.active && !install.done && !install.error}
        <div class="flex items-center flex-wrap gap-x-3 gap-y-0.5 mt-1.5 text-[11px] text-base-content/55 font-mono">
          <span class="flex items-center gap-1 text-success/80" title={m.dayzavr_speed_down()}>
            <Icon icon="ph:arrow-down" class="size-3" />{fmtSpeed(install.downloadMbps)}
          </span>
          <span class="flex items-center gap-1" title={m.dayzavr_speed_up()}>
            <Icon icon="ph:arrow-up" class="size-3" />{fmtSpeed(install.uploadMbps)}
          </span>
          <span class="flex items-center gap-1" title={m.dayzavr_peers()}>
            <Icon icon="ph:users-three" class="size-3" />{install.peersLive}
            <span class="text-base-content/30"
              >/ {install.peersConnecting}{m.dayzavr_peers_connecting_suffix()} / {install.peersSeen}{m.dayzavr_peers_seen_suffix()}</span
            >
          </span>
          {#if install.eta}
            <span class="flex items-center gap-1" title={m.dayzavr_eta()}>
              <Icon icon="ph:hourglass-medium" class="size-3" />{install.eta}
            </span>
          {/if}
          {#if install.uploadedBytes > 0}
            <span class="text-base-content/30">↑ {fmtBytes(install.uploadedBytes)}</span>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  <!-- Server list -->
  <div class="flex-1 overflow-y-auto p-3 space-y-2">
    {#if s.dayzavrLoading && s.dayzavrServers.length === 0}
      <div class="flex items-center justify-center h-full gap-2 text-base-content/50">
        <span class="loading loading-spinner loading-md"></span>
        <span class="text-sm">{m.dayzavr_loading()}</span>
      </div>
    {:else if s.dayzavrServers.length === 0}
      <div class="flex flex-col items-center justify-center h-full gap-2 text-base-content/30">
        <Icon icon="ph:skull" class="size-10 opacity-30" />
        <span class="text-sm">{m.dayzavr_empty()}</span>
      </div>
    {:else}
      {#each sortedServers as server (`${server.host}:${server.game_port}`)}
        {@const installed = serverModsInstalled(server)}
        <div class="rounded-xl border border-base-300 bg-base-100 overflow-hidden">
          <div class="flex items-stretch gap-3 p-3">
            <!-- Banner -->
            {#if server.image}
              <img
                src={server.image}
                alt=""
                class="w-20 h-20 rounded-lg object-cover shrink-0 bg-base-300"
                loading="lazy"
              />
            {/if}
            <!-- Info -->
            <div class="flex-1 min-w-0">
              <div class="flex items-center gap-1.5 min-w-0">
                <span class="truncate font-medium text-base-content/90">{server.name}</span>
                {#if server.password}
                  <span title={m.servers_password_protected()}
                    ><Icon icon="mdi:lock" class="size-3 text-error shrink-0" /></span
                  >
                {/if}
              </div>
              <!-- Players bar -->
              <div class="flex items-center gap-2 mt-1 max-w-xs">
                <span class="tabular-nums font-mono text-xs {playerFill(server.players, server.max_players)} w-12 shrink-0"
                  >{server.players}<span class="text-base-content/30">/{server.max_players}</span></span
                >
                <div class="flex-1 h-1 rounded-full bg-base-300 overflow-hidden">
                  <div
                    class="h-full rounded-full {playerBarColor(server.players, server.max_players)}"
                    style="width:{pct(server)}%"
                  ></div>
                </div>
              </div>
              <!-- Meta -->
              <div class="flex items-center flex-wrap gap-x-3 gap-y-0.5 mt-1.5 text-[11px] text-base-content/50">
                <span class="flex items-center gap-1">
                  <Icon icon="mdi:puzzle-outline" class="size-3" />
                  <button class="hover:text-base-content" onclick={() => (expanded = expanded === server.id ? null : server.id)}>
                    {m.dayzavr_mods_count({ count: server.mods.length })}
                  </button>
                </span>
                {#if server.time}<span class="flex items-center gap-1"><Icon icon="ph:clock" class="size-3" />{server.time}</span>{/if}
                {#if server.restart}<span class="flex items-center gap-1" title={m.dayzavr_restart()}><Icon icon="ph:arrow-counter-clockwise" class="size-3" />{server.restart}</span>{/if}
                <button
                  class="flex items-center gap-1 hover:text-base-content font-mono"
                  onclick={() => cp.copy(`${server.host}:${server.game_port}`)}
                  title={m.dayzavr_connect_copy()}
                >
                  <Icon icon={cp.copied ? 'ph:check' : 'ph:copy'} class="size-3" />{server.host}:{server.game_port}
                </button>
                {#if server.discord}
                  <button class="flex items-center gap-1 text-info/70 hover:text-info" onclick={() => openUrl(server.discord!)}>
                    <Icon icon="ic:baseline-discord" class="size-3" />Discord
                  </button>
                {/if}
              </div>
            </div>
            <!-- Actions -->
            <div class="flex flex-col gap-1.5 shrink-0 justify-center">
              {#if installed}
                <!-- Play only when every required mod is installed -->
                <button
                  class="btn btn-success btn-xs gap-1.5"
                  onclick={() => joinDayzavrServer(server)}
                  title={m.dayzavr_join_title()}
                >
                  <Icon icon="ph:play-fill" class="size-3.5" />
                  {m.dayzavr_join()}
                </button>
              {/if}
              <button
                class="btn btn-xs gap-1.5 {installed ? 'btn-ghost' : 'btn-primary'}"
                onclick={() => installDayzavrMods(server)}
                disabled={install?.active || !dayzPath}
                title={dayzPath ? m.dayzavr_install_title() : m.dayzavr_set_path_warning()}
              >
                <Icon icon={installed ? 'ph:arrows-clockwise' : 'ph:download-simple'} class="size-3.5" />
                {installed ? m.dayzavr_update() : m.dayzavr_install()}
              </button>
            </div>
          </div>
          <!-- Expanded mod list -->
          {#if expanded === server.id}
            <div class="px-3 pb-3 pt-0 flex flex-wrap gap-1">
              {#each server.mods as mod}
                <span class="font-mono text-[10px] bg-base-200 text-base-content/60 px-1.5 py-0.5 rounded">{mod}</span>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>
