<script lang="ts">
  import type { ServerDto, ServerFullDto, ModDto, A2sDetailsDto, InstalledModDto } from '$lib/types';
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '@iconify/svelte';

  interface Props {
    server: ServerDto;
    a2s: A2sDetailsDto | null;
    a2sLoading: boolean;
    a2sError: string;
    installedMods: InstalledModDto[];
    pingMs: number | null;
    onClose: () => void;
    onQueryA2s: () => void;
  }

  let { server, a2s, a2sLoading, a2sError, installedMods, pingMs, onClose, onQueryA2s }: Props = $props();

  let installedIds = $derived(new Set(installedMods.map((m) => m.id)));

  // On-demand mod fetching
  let mods = $state<ModDto[]>([]);
  let modsLoading = $state(false);
  let lastFetchedKey = $state('');

  // Fetch full details when server changes
  $effect(() => {
    const key = `${server.ip}:${server.query_port}`;
    if (key !== lastFetchedKey) {
      lastFetchedKey = key;
      mods = [];
      if (server.mods_count > 0) {
        modsLoading = true;
        invoke<ServerFullDto>('get_server_details', { ip: server.ip, port: server.query_port })
          .then((full) => {
            // Guard: only apply if we're still looking at the same server
            if (`${server.ip}:${server.query_port}` === key) {
              mods = full.mods;
            }
          })
          .catch(() => {
            mods = [];
          })
          .finally(() => {
            modsLoading = false;
          });
      }
    }
  });

  function pingColor(ms: number | null): string {
    if (ms === null) return 'text-base-content/40';
    if (ms < 50) return 'text-success';
    if (ms < 100) return 'text-warning';
    return 'text-error';
  }

  function playerFill(players: number, max: number): string {
    if (players === 0) return 'text-base-content/40';
    if (players >= max) return 'text-error';
    if (players > max / 2) return 'text-warning';
    return 'text-success';
  }
</script>

<div class="flex flex-col h-full overflow-hidden border-l border-base-300 bg-base-100">
  <!-- Header -->
  <div class="flex items-center justify-between px-3 py-2 bg-base-200 border-b border-base-300 flex-shrink-0">
    <span class="font-semibold text-sm truncate text-base-content">{server.name}</span>
    <button class="btn btn-ghost btn-xs" onclick={onClose}>✕</button>
  </div>

  <div class="flex-1 overflow-y-auto p-3 space-y-3 text-sm">
    <!-- Core info grid -->
    <div class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
      <div class="text-base-content/50">IP</div>
      <div class="font-mono text-base-content">{server.ip}:{server.game_port}</div>

      <div class="text-base-content/50">Query port</div>
      <div class="font-mono text-base-content">{server.query_port}</div>

      <div class="text-base-content/50">Players</div>
      <div class="font-bold {playerFill(server.players, server.max_players)}">
        {server.players}/{server.max_players}
      </div>

      <div class="text-base-content/50">Map</div>
      <div class="text-teal-400">{server.map}</div>

      <div class="text-base-content/50">Version</div>
      <div class="text-base-content/70">{server.version}</div>

      <div class="text-base-content/50">Time</div>
      <div class="text-base-content/70">{server.time}</div>

      <div class="text-base-content/50">Ping</div>
      <div class="font-mono {pingColor(pingMs)}">
        {pingMs !== null ? `${pingMs} ms` : '—'}
      </div>

      <div class="text-base-content/50">Platform</div>
      <div class="{server.environment === 'w' ? 'text-info' : 'text-success'}">
        {server.environment === 'w' ? 'Windows' : 'Linux'}
      </div>

      <div class="text-base-content/50">1st-person</div>
      <div>{server.first_person_only ? 'Yes' : 'No'}</div>

      <div class="text-base-content/50">Password</div>
      <div class="{server.password ? 'text-error' : 'text-base-content/40'}">
        {server.password ? 'Yes' : 'No'}
      </div>

      <div class="text-base-content/50">VAC</div>
      <div>{server.vac ? 'Yes' : 'No'}</div>

      {#if server.battl_eye !== null}
        <div class="text-base-content/50">BattlEye</div>
        <div>{server.battl_eye ? 'Yes' : 'No'}</div>
      {/if}
    </div>

    <!-- Mods -->
    {#if modsLoading}
      <div class="flex items-center gap-2 text-xs text-base-content/50">
        <span class="loading loading-spinner loading-xs"></span>
        Loading mods ({server.mods_count})…
      </div>
    {:else if mods.length > 0}
      <div>
        <div class="text-xs font-semibold text-base-content/50 mb-1">
          Mods ({mods.length})
        </div>
        <div class="space-y-0.5 max-h-40 overflow-y-auto">
          {#each mods as mod}
            {@const installed = installedIds.has(mod.steam_workshop_id)}
            <div class="flex items-center gap-1.5 text-xs">
              <span class="{installed ? 'text-success' : 'text-error'} font-bold w-3 text-center flex-shrink-0">
                {installed ? '+' : '−'}
              </span>
              <span class="truncate text-base-content/80">{mod.name}</span>
              <button
                class="text-base-content/40 ml-auto font-mono flex-shrink-0 hover:text-primary transition-colors"
                onclick={(e) => { e.stopPropagation(); openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.steam_workshop_id}`); }}
                title="Open on Steam Workshop"
              >{mod.steam_workshop_id}</button>
            </div>
          {/each}
        </div>
      </div>
    {:else if server.mods_count > 0}
      <div class="text-xs text-base-content/40 italic">
        {server.mods_count} mod(s) — details unavailable
      </div>
    {:else}
      <div class="text-xs text-base-content/40 italic">No mods</div>
    {/if}

    <!-- A2S live info -->
    <div>
      <div class="flex items-center justify-between mb-1">
        <span class="text-xs font-semibold text-base-content/50">Live A2S Info</span>
        <button
          class="btn btn-ghost btn-xs"
          onclick={onQueryA2s}
          disabled={a2sLoading}
        >
          {#if a2sLoading}
            <span class="loading loading-spinner loading-xs"></span>
          {:else}
            Refresh
          {/if}
        </button>
      </div>

      {#if a2s}
        <div class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs mb-2">
          <div class="text-base-content/50">Live players</div>
          <div class="font-bold text-success">{a2s.players}/{a2s.max_players}</div>
          <div class="text-base-content/50">Game</div>
          <div>{a2s.game}</div>
        </div>

        {#if a2s.players_list.length > 0}
          <div class="text-xs text-base-content/50 mb-1">Online players</div>
          <div class="space-y-0.5 max-h-32 overflow-y-auto">
            {#each a2s.players_list as player}
              <div class="flex items-center gap-2 text-xs">
                <span class="text-base-content/80 truncate">{player.name}</span>
                <span class="text-base-content/40 ml-auto">score: {player.score}</span>
              </div>
            {/each}
          </div>
        {:else if a2s.players === 0}
          <p class="text-xs text-base-content/40 italic">No players online</p>
        {:else}
          <p class="text-xs text-base-content/40 italic">Player names not reported by server</p>
        {/if}
      {:else if a2sError}
        <div class="flex items-start gap-2 px-2.5 py-2 rounded-lg bg-error/10 border border-error/25 text-xs text-error">
          <Icon icon="ph:warning-circle" class="size-3.5 shrink-0 mt-0.5" />
          <span class="leading-snug break-all">{a2sError}</span>
        </div>
      {:else if !a2sLoading}
        <p class="text-xs text-base-content/40 italic">Click Refresh to query live data</p>
      {/if}
    </div>
  </div>
</div>
