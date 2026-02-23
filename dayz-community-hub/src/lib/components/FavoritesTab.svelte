<script lang="ts">
  import type { FavoriteDto, ServerDto, A2sDetailsDto } from '$lib/types';
  import { invoke } from '@tauri-apps/api/core';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '@iconify/svelte';

  interface Props {
    favorites: FavoriteDto[];
    servers: ServerDto[];
    pingCache: Map<string, number>;
    onConnect: (ip: string, port: number, name: string) => void;
    onRemove: (fav: FavoriteDto) => void;
    onGoToServers?: () => void;
  }

  let { favorites, servers, pingCache, onConnect, onRemove, onGoToServers }: Props = $props();

  // ── Sorting ──────────────────────────────────────────────────────────────
  type SortCol = 'name' | 'players' | 'ping';
  let sortCol = $state<SortCol>('name');
  let sortAsc = $state(true);

  function toggleSort(col: SortCol) {
    if (sortCol === col) {
      sortAsc = !sortAsc;
    } else {
      sortCol = col;
      sortAsc = col === 'name';
    }
  }

  function sortIcon(col: SortCol) {
    if (sortCol !== col) return 'ph:arrows-down-up';
    return sortAsc ? 'ph:arrow-up' : 'ph:arrow-down';
  }

  // Pre-built lookup map: both "ip:query_port" and "ip:game_port" → server.
  // Rebuilt only when `servers` changes (O(n) once) so per-row lookups are O(1).
  let serverByKey = $derived((() => {
    const m = new Map<string, ServerDto>();
    for (const s of servers) {
      m.set(`${s.ip}:${s.query_port}`, s);
      m.set(`${s.ip}:${s.game_port}`, s);
    }
    return m;
  })());

  function findServer(fav: FavoriteDto): ServerDto | null {
    return serverByKey.get(`${fav.ip}:${fav.port}`) ?? null;
  }

  /** Best ping key for a favorite: prefer server's query_port, fall back to fav.port. */
  function pingKey(fav: FavoriteDto): string {
    const sv = findServer(fav);
    return sv ? `${fav.ip}:${sv.query_port}` : `${fav.ip}:${fav.port}`;
  }

  let sorted = $derived((() => {
    const arr = favorites.slice();
    const dir = sortAsc ? 1 : -1;
    arr.sort((a, b) => {
      switch (sortCol) {
        case 'name':
          return dir * a.name.localeCompare(b.name);
        case 'players': {
          const pa = findServer(a)?.players ?? -1;
          const pb = findServer(b)?.players ?? -1;
          return dir * (pa - pb);
        }
        case 'ping': {
          const pa = pingCache.get(pingKey(a)) ?? Infinity;
          const pb = pingCache.get(pingKey(b)) ?? Infinity;
          return dir * (pa - pb);
        }
        default: return 0;
      }
    });
    return arr;
  })());

  // ── A2S detail panel ─────────────────────────────────────────────────────
  let detailFav = $state<FavoriteDto | null>(null);
  let a2s = $state<A2sDetailsDto | null>(null);
  let a2sLoading = $state(false);
  let a2sError = $state('');

  async function openDetail(fav: FavoriteDto) {
    detailFav = fav;
    a2s = null;
    a2sError = '';
    a2sLoading = true;
    try {
      // Prefer the authoritative query_port from the live server list.
      // Favorites store the game port; query port = game port - 1 (DayZ convention).
      const sv = findServer(fav);
      const queryPort = sv ? sv.query_port : fav.port;
      a2s = await invoke<A2sDetailsDto>('query_a2s', { ip: fav.ip, port: queryPort });
    } catch (e) {
      a2sError = String(e);
    } finally {
      a2sLoading = false;
    }
  }

  function closeDetail() {
    detailFav = null;
    a2s = null;
    a2sError = '';
  }

  let selectedIdx = $state(-1);

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && detailFav) {
      closeDetail();
      e.preventDefault();
      return;
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const len = sorted.length;
      if (len === 0) return;
      selectedIdx = e.key === 'ArrowDown'
        ? Math.min(selectedIdx + 1, len - 1)
        : Math.max(selectedIdx - 1, 0);
    }
    if (e.key === 'Enter' && selectedIdx >= 0 && selectedIdx < sorted.length) {
      e.preventDefault();
      openDetail(sorted[selectedIdx]);
    }
  }

  // ── Copy IP ───────────────────────────────────────────────────────────────
  let copiedKey = $state('');
  async function copyIp(e: MouseEvent, ip: string, port: number) {
    e.stopPropagation();
    const text = `${ip}:${port}`;
    await writeText(text);
    copiedKey = text;
    setTimeout(() => { if (copiedKey === text) copiedKey = ''; }, 1500);
  }

  // ── Helpers ───────────────────────────────────────────────────────────────
  function pingColor(ms: number | undefined): string {
    if (ms === undefined) return 'text-base-content/30';
    if (ms < 50) return 'text-success';
    if (ms < 100) return 'text-warning';
    return 'text-error';
  }

  function playerFill(players: number, max: number): string {
    if (players === 0) return 'text-base-content/30';
    if (players >= max) return 'text-error';
    if (players > max / 2) return 'text-warning';
    return 'text-success';
  }

  function pingDot(ms: number | undefined): string {
    if (ms === undefined) return 'bg-base-content/20';
    if (ms < 50) return 'bg-success';
    if (ms < 100) return 'bg-warning';
    return 'bg-error';
  }

  function playerBarColor(players: number, max: number): string {
    if (players === 0) return 'bg-base-content/20';
    if (players >= max) return 'bg-error';
    if (players > max / 2) return 'bg-warning';
    return 'bg-success';
  }

  function formatDuration(secs: number): string {
    const s = Math.floor(secs);
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m`;
    return `<1m`;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="flex h-full overflow-hidden" role="region" onkeydown={handleKeydown}>
  <!-- Table -->
  <div class="flex flex-col flex-1 overflow-hidden">
    {#if favorites.length === 0}
      <div class="flex flex-col items-center justify-center h-full gap-3 text-base-content/40">
        <Icon icon="ph:star" class="size-10 opacity-30" />
        <span class="text-sm">No favorites yet</span>
        <button
          class="btn btn-sm btn-outline btn-primary gap-1.5"
          onclick={onGoToServers}
        >
          <Icon icon="mdi:server" class="size-3.5" />
          Browse Servers
        </button>
      </div>
    {:else}
      <div class="overflow-auto flex-1">
        <table class="w-full text-xs" style="table-layout: fixed; border-collapse: collapse;">
          <thead class="sticky top-0 z-10">
            <tr class="bg-base-200/95 backdrop-blur-sm text-base-content/50 uppercase tracking-wider border-b border-base-300 select-none" style="font-size:10px;">
              <th class="px-3 py-2 text-left cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('name')}>
                <span class="flex items-center gap-1">
                  Server
                  <span class="normal-case font-normal text-base-content/35 ml-0.5">{favorites.length}</span>
                  <Icon icon={sortIcon('name')} class="size-2.5" />
                </span>
              </th>
              <th class="w-32 px-3 py-2 cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('players')}>
                <span class="flex items-center gap-1">Players <Icon icon={sortIcon('players')} class="size-2.5" /></span>
              </th>
              <th class="w-20 px-3 py-2 cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('ping')}>
                <span class="flex items-center gap-1">Ping <Icon icon={sortIcon('ping')} class="size-2.5" /></span>
              </th>
              <th class="w-28 px-3 py-2 text-left font-medium">Map</th>
              <th class="w-16 px-3 py-2 font-medium text-left" title="In-game server time">Time</th>
              <th class="w-14 px-3 py-2 text-center font-medium">Mods</th>
              <th class="w-8 px-2 py-2 text-center font-medium">OS</th>
              <th class="w-40 px-3 py-2"></th>
            </tr>
          </thead>
          <tbody>
            {#each sorted as fav, fi}
              {@const server = findServer(fav)}
              {@const ping = pingCache.get(pingKey(fav))}
              {@const isSelected = detailFav?.ip === fav.ip && detailFav?.port === fav.port}
              {@const isFocused = fi === selectedIdx}
              {@const pct = server && server.max_players > 0 ? Math.round((server.players / server.max_players) * 100) : 0}
              <tr
                class="group/row border-b border-base-300/40 transition-colors cursor-pointer
                       {isSelected ? 'bg-primary/10 border-primary/20' : isFocused ? 'bg-base-200/80 outline outline-1 outline-primary/40' : 'hover:bg-base-200/60'}"
                onclick={() => { selectedIdx = fi; openDetail(fav); }}
              >
                <!-- Server name + IP -->
                <td class="px-3 py-2 max-w-0">
                  <div class="flex items-center gap-1.5 min-w-0">
                    <span class="truncate font-medium text-base-content/90">{fav.name}</span>
                    {#if !server}
                      <span class="shrink-0 text-warning" style="font-size:9px;" title="Server not found in the current server list — it may be offline, or try refreshing the server list">OFFLINE</span>
                    {/if}
                  </div>
                  <div class="flex items-center gap-2 mt-0.5">
                    <button
                      class="font-mono flex items-center gap-1 group/ip
                             {copiedKey === `${fav.ip}:${fav.port}` ? 'text-success' : 'text-base-content/30 hover:text-base-content/60'}"
                      style="font-size:10px;"
                      onclick={(e) => copyIp(e, fav.ip, fav.port)}
                      title="Copy {fav.ip}:{fav.port} to clipboard"
                    >
                      {fav.ip}:{fav.port}
                      <Icon
                        icon={copiedKey === `${fav.ip}:${fav.port}` ? 'ph:check' : 'ph:copy'}
                        class="size-2 opacity-0 group-hover/ip:opacity-100 transition-opacity {copiedKey === `${fav.ip}:${fav.port}` ? 'opacity-100' : ''}"
                      />
                    </button>
                    {#if server}
                      <span class="text-base-content/25" style="font-size:10px;">{server.version}</span>
                    {/if}
                  </div>
                </td>

                <!-- Players + bar -->
                <td class="px-3 py-2">
                  {#if server}
                    <div class="flex items-center gap-2">
                      <span class="tabular-nums font-mono {playerFill(server.players, server.max_players)} w-14 shrink-0">
                        {server.players}<span class="text-base-content/30">/{server.max_players}</span>
                      </span>
                      <div class="flex-1 h-1 rounded-full bg-base-300 overflow-hidden">
                        <div class="h-full rounded-full {playerBarColor(server.players, server.max_players)}" style="width:{pct}%"></div>
                      </div>
                    </div>
                  {:else}
                    <span class="text-base-content/25 font-mono">—</span>
                  {/if}
                </td>

                <!-- Ping -->
                <td class="px-3 py-2">
                  <div class="flex items-center gap-1.5">
                    <span class="size-1.5 rounded-full shrink-0 {pingDot(ping)}"></span>
                    <span class="tabular-nums font-mono {pingColor(ping)}">
                      {ping !== undefined ? `${ping}ms` : '—'}
                    </span>
                  </div>
                </td>

                <!-- Map -->
                <td class="px-3 py-2 max-w-0">
                  <span class="truncate block text-teal-400/80">{server ? server.map : '—'}</span>
                </td>

                <!-- Time -->
                <td class="px-3 py-2">
                  <span class="text-base-content/60 tabular-nums font-mono">{server?.time || '—'}</span>
                </td>

                <!-- Mods -->
                <td class="px-3 py-2 text-center">
                  {#if server && server.mods_count > 0}
                    <span class="inline-flex items-center gap-0.5 text-fuchsia-400/90">
                      <Icon icon="mdi:puzzle-outline" class="size-3 shrink-0" />
                      {server.mods_count}
                    </span>
                  {:else}
                    <span class="text-base-content/20">—</span>
                  {/if}
                </td>

                <!-- OS -->
                <td class="px-2 py-2 text-center">
                  {#if server}
                    {#if server.environment === 'w'}
                      <span title="Windows"><Icon icon="gg:windows" class="size-3.5 text-sky-400/70" /></span>
                    {:else}
                      <span title="Linux"><Icon icon="simple-icons:linux" class="size-3.5 text-rose-400/80" /></span>
                    {/if}
                  {:else}
                    <span class="text-base-content/20">—</span>
                  {/if}
                </td>

                <!-- Actions — always visible -->
                <td class="px-2 py-2">
                  <div class="flex gap-1 items-center justify-end">
                    <!-- Info toggle -->
                    <span title={isSelected ? 'Close details' : 'Live server details'}>
                      <button
                        class="size-6 rounded flex items-center justify-center transition-colors
                               {isSelected
                                 ? 'bg-primary/15 text-primary hover:bg-primary/25'
                                 : 'text-base-content/35 hover:bg-base-300 hover:text-base-content/80'}"
                        onclick={() => isSelected ? closeDetail() : openDetail(fav)}
                      >
                        <Icon icon="ph:info" class="size-3.5" />
                      </button>
                    </span>
                    <!-- Remove -->
                    <span title="Remove from favorites">
                      <button
                        class="size-6 rounded flex items-center justify-center text-base-content/35 hover:bg-error/10 hover:text-error transition-colors"
                        onclick={() => onRemove(fav)}
                      >
                        <Icon icon="ph:trash" class="size-3.5" />
                      </button>
                    </span>
                    <!-- Connect -->
                    <button
                      class="btn btn-primary btn-xs h-6 min-h-0 px-2.5 text-xs font-medium"
                      onclick={() => onConnect(fav.ip, fav.port, fav.name)}
                    >
                      Connect
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>

  <!-- A2S detail side panel -->
  {#if detailFav}
    <div class="w-72 flex-shrink-0 border-l border-base-300 flex flex-col overflow-hidden">
      <!-- Panel header -->
      <div class="flex items-center gap-2 px-3 py-2 bg-base-200 border-b border-base-300 flex-shrink-0">
        <Icon icon="mdi:server" class="size-4 text-primary shrink-0" />
        <span class="text-xs font-semibold truncate flex-1">{detailFav.name}</span>
        <button class="btn btn-ghost btn-xs p-0.5" onclick={closeDetail} title="Close">
          <Icon icon="ph:x" class="size-3.5" />
        </button>
      </div>

      <div class="flex-1 flex flex-col min-h-0">
        {#if a2sLoading}
          <div class="flex items-center justify-center py-8 gap-2 text-base-content/50">
            <span class="loading loading-spinner loading-sm"></span>
            <span class="text-xs">Querying…</span>
          </div>
        {:else if a2sError}
          <div class="m-3 flex items-start gap-2 px-2.5 py-2 rounded-lg bg-error/10 border border-error/25 text-xs text-error">
            <Icon icon="ph:warning-circle" class="size-3.5 shrink-0 mt-0.5" />
            <span class="leading-snug break-all">{a2sError}</span>
          </div>
        {:else if a2s}
          <!-- Fixed top section: stats + players -->
          <div class="flex-shrink-0 p-3 space-y-3">
            <!-- Stats grid -->
            <div class="grid grid-cols-2 gap-x-3 gap-y-2 text-xs">
              <span class="flex items-center gap-1.5 text-base-content/50">
                <Icon icon="mdi:controller" class="size-3.5 shrink-0" />Players
              </span>
              <span class="font-mono font-medium {playerFill(a2s.players, a2s.max_players)}">
                {a2s.players}/{a2s.max_players}
              </span>

              <span class="flex items-center gap-1.5 text-base-content/50">
                <Icon icon="mdi:map-outline" class="size-3.5 shrink-0" />Map
              </span>
              <span class="text-teal-400">{a2s.map}</span>

              <span class="flex items-center gap-1.5 text-base-content/50">
                <Icon icon="mdi:tag-outline" class="size-3.5 shrink-0" />Version
              </span>
              <span class="text-base-content/70">{a2s.version}</span>

              <span class="flex items-center gap-1.5 text-base-content/50">
                <Icon icon="mdi:gamepad-variant-outline" class="size-3.5 shrink-0" />Game
              </span>
              <span class="text-base-content/70">{a2s.game}</span>

              <span class="flex items-center gap-1.5 text-base-content/50">
                <Icon icon="mdi:signal" class="size-3.5 shrink-0" />Ping
              </span>
              <span class="font-mono {pingColor(pingCache.get(pingKey(detailFav)))}">
                {pingCache.get(pingKey(detailFav)) !== undefined ? `${pingCache.get(pingKey(detailFav))}ms` : '—'}
              </span>
            </div>

            <!-- Online players -->
            {#if a2s.players_list.length > 0}
              <div>
                <div class="flex items-center gap-1.5 text-xs text-base-content/40 mb-1.5">
                  <Icon icon="mdi:account-multiple-outline" class="size-3.5" />
                  <span>Online ({a2s.players_list.length})</span>
                </div>
                <div class="space-y-1 max-h-36 overflow-y-auto">
                  {#each a2s.players_list as pl}
                    <div class="flex justify-between text-xs">
                      <div class="flex items-center gap-1.5 text-base-content/80">
                        <Icon icon="mdi:account-outline" class="size-3 text-base-content/30" />
                        <span>{pl.name || '—'}</span>
                      </div>
                      <span class="text-base-content/30 tabular-nums">{formatDuration(pl.duration)}</span>
                    </div>
                  {/each}
                </div>
              </div>
            {:else if a2s.players === 0}
              <p class="text-xs text-base-content/30 text-center py-1">No players online</p>
            {:else}
              <p class="text-xs text-base-content/30 text-center py-1">Player names not reported by server</p>
            {/if}
          </div>

          <!-- Mod list — fills all remaining height -->
          {#if a2s.mods.length > 0}
            <div class="flex flex-col flex-1 min-h-0 border-t border-base-300">
              <div class="flex items-center gap-1.5 text-xs text-base-content/40 px-3 py-2 flex-shrink-0">
                <Icon icon="mdi:puzzle-outline" class="size-3.5" />
                <span>Mods ({a2s.mods.length})</span>
              </div>
              <div class="flex-1 overflow-y-auto px-3 pb-2 space-y-1">
                {#each a2s.mods as mod}
                  <div class="flex items-center gap-1.5 text-xs">
                    <Icon icon="mdi:puzzle-outline" class="size-3 text-secondary shrink-0" />
                    <button
                      class="truncate text-base-content/80 hover:text-primary transition-colors text-left"
                      onclick={() => openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.steam_workshop_id}`)}
                      title="Open on Steam Workshop: {mod.name}"
                    >{mod.name}</button>
                    <button
                      class="ml-auto shrink-0 font-mono text-xs text-base-content/30 hover:text-primary transition-colors flex items-center gap-0.5"
                      onclick={() => openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.steam_workshop_id}`)}
                      title="Open on Steam Workshop"
                    >
                      {mod.steam_workshop_id}
                      <Icon icon="mdi:steam" class="size-3" />
                    </button>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        {/if}
      </div>

      <!-- Refresh button -->
      <div class="px-3 py-2 border-t border-base-300 flex-shrink-0">
        <button
          class="btn btn-ghost btn-xs w-full gap-1.5"
          onclick={() => detailFav && openDetail(detailFav)}
          disabled={a2sLoading}
        >
          <Icon icon="ph:arrows-clockwise" class="size-3.5" />
          Refresh
        </button>
      </div>
    </div>
  {/if}
</div>
