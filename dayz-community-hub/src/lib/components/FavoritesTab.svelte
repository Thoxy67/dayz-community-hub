<script lang="ts">
  import type { FavoriteDto, ServerDto, A2sDetailsDto, BattleMetricsDto } from '$lib/types';
  import {
    pingLabel,
    pingColor,
    pingDot,
    playerFill,
    playerBarColor,
    formatDuration,
    sortIcon as _sortIcon,
  } from '$lib/utils';
  import BattleMetricsPanel from './BattleMetricsPanel.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '@iconify/svelte';
  import { onMount } from 'svelte';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    favorites: FavoriteDto[];
    servers: ServerDto[];
    pingCache: Map<string, number>;
    /** BattleMetrics personal access token (null = not configured). */
    bmApiKey: string | null;
    /** User location [longitude, latitude] for distance calculation. */
    userLocation?: [number, number] | null;
    onConnect: (ip: string, port: number, name: string) => void;
    onRemove: (fav: FavoriteDto) => void;
    onGoToServers?: () => void;
    onPing: (ip: string, port: number) => void;
    /** D key — open selected server in Direct Connect tab with query. */
    onDirectConnect?: (ip: string, gamePort: number, queryPort?: number) => void;
  }

  let {
    favorites,
    servers,
    pingCache,
    bmApiKey,
    userLocation = null,
    onConnect,
    onRemove,
    onGoToServers,
    onPing,
    onDirectConnect,
  }: Props = $props();

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
    return _sortIcon(col, sortCol, sortAsc);
  }

  // Pre-built lookup map: both "ip:query_port" and "ip:game_port" → server.
  // Rebuilt only when `servers` changes (O(n) once) so per-row lookups are O(1).
  let serverByKey = $derived(
    (() => {
      const m = new Map<string, ServerDto>();
      for (const s of servers) {
        m.set(`${s.ip}:${s.query_port}`, s);
        m.set(`${s.ip}:${s.game_port}`, s);
      }
      return m;
    })(),
  );

  function findServer(fav: FavoriteDto): ServerDto | null {
    return serverByKey.get(`${fav.ip}:${fav.port}`) ?? null;
  }

  // Track which servers were just pinged for a brief green flash.
  let pingFlash = $state<Set<string>>(new Set());

  // A2S-refreshed player counts: "ip:queryPort" → live players
  let a2sPlayers = $state<Map<string, number>>(new Map());
  let a2sPlayersLoading = $state<Set<string>>(new Set());

  async function doRefreshPlayers(fav: FavoriteDto) {
    const sv = findServer(fav);
    const ip = fav.ip;
    const queryPort = sv ? sv.query_port : fav.port;
    const key = `${ip}:${queryPort}`;
    if (a2sPlayersLoading.has(key)) return;
    a2sPlayersLoading = new Set([...a2sPlayersLoading, key]);
    try {
      const res = await invoke<A2sDetailsDto>('query_a2s', { ip, port: queryPort });
      a2sPlayers = new Map([...a2sPlayers, [key, res.players]]);
    } catch {
      const fallback = sv ? sv.players : 0;
      a2sPlayers = new Map([...a2sPlayers, [key, fallback]]);
    } finally {
      a2sPlayersLoading.delete(key);
      a2sPlayersLoading = new Set(a2sPlayersLoading);
    }
  }

  function favA2sKey(fav: FavoriteDto): string {
    const sv = findServer(fav);
    return `${fav.ip}:${sv ? sv.query_port : fav.port}`;
  }

  function doPing(fav: FavoriteDto) {
    const sv = findServer(fav);
    const port = sv ? sv.query_port : fav.port;
    const key = `${fav.ip}:${port}`;
    onPing(fav.ip, port);
    pingFlash.add(key);
    pingFlash = new Set(pingFlash);
    setTimeout(() => {
      pingFlash.delete(key);
      pingFlash = new Set(pingFlash);
    }, 1000);
  }

  /** Best ping key for a favorite: prefer server's query_port, fall back to fav.port. */
  function pingKey(fav: FavoriteDto): string {
    const sv = findServer(fav);
    return sv ? `${fav.ip}:${sv.query_port}` : `${fav.ip}:${fav.port}`;
  }

  let sorted = $derived(
    (() => {
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
          default:
            return 0;
        }
      });
      return arr;
    })(),
  );

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
    bm = null;
    bmError = '';
    bmFetchedKey = '';
  }

  // ── BattleMetrics ──────────────────────────────────────────────────────────
  let bm = $state<BattleMetricsDto | null>(null);
  let bmLoading = $state(false);
  let bmError = $state('');
  let bmFetchedKey = '';
  let bmRetryTick = $state(0);
  let _bmDebounce: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    if (!detailFav) return;
    const sv = findServer(detailFav);
    const queryPort = sv ? sv.query_port : detailFav.port;
    const key = `${detailFav.ip}:${queryPort}`;
    bmRetryTick;
    const token = bmApiKey;
    if (!token || key === bmFetchedKey) return;

    clearTimeout(_bmDebounce);
    _bmDebounce = setTimeout(() => {
      if (!detailFav) return;
      const currentSv = findServer(detailFav);
      const currentPort = currentSv ? currentSv.query_port : detailFav.port;
      if (`${detailFav.ip}:${currentPort}` !== key) return;
      bmLoading = true;
      bmError = '';
      invoke<BattleMetricsDto>('fetch_battlemetrics_server', { ip: detailFav.ip, port: queryPort })
        .then((result) => {
          if (
            detailFav &&
            `${detailFav.ip}:${(findServer(detailFav) ?? { query_port: detailFav.port }).query_port}` === key
          ) {
            bm = result;
            bmError = '';
            bmFetchedKey = key;
          }
        })
        .catch((e: unknown) => {
          bm = null;
          bmError = String(e);
          bmFetchedKey = key;
        })
        .finally(() => {
          bmLoading = false;
        });
    }, 300);
    return () => clearTimeout(_bmDebounce);
  });

  let selectedIdx = $state(-1);

  function handleKeydown(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

    if (e.key === 'Escape' && detailFav) {
      closeDetail();
      e.preventDefault();
      return;
    }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const len = sorted.length;
      if (len === 0) return;
      selectedIdx = e.key === 'ArrowDown' ? Math.min(selectedIdx + 1, len - 1) : Math.max(selectedIdx - 1, 0);
    }
    if (e.key === 'Enter' && selectedIdx >= 0 && selectedIdx < sorted.length) {
      e.preventDefault();
      openDetail(sorted[selectedIdx]);
    }
    if ((e.key === 'f' || e.key === 'F') && !e.ctrlKey && selectedIdx >= 0 && selectedIdx < sorted.length) {
      // F — remove from favorites
      e.preventDefault();
      onRemove(sorted[selectedIdx]);
    }
    if ((e.key === 'i' || e.key === 'I') && !e.ctrlKey && selectedIdx >= 0 && selectedIdx < sorted.length) {
      // I — toggle info/detail panel
      e.preventDefault();
      if (detailFav?.ip === sorted[selectedIdx].ip && detailFav?.port === sorted[selectedIdx].port) {
        closeDetail();
      } else {
        openDetail(sorted[selectedIdx]);
      }
    }
    if ((e.key === 'p' || e.key === 'P') && !e.ctrlKey && selectedIdx >= 0 && selectedIdx < sorted.length) {
      // P — ping the selected server
      e.preventDefault();
      doPing(sorted[selectedIdx]);
    }
    if (
      (e.key === 'd' || e.key === 'D') &&
      !e.ctrlKey &&
      selectedIdx >= 0 &&
      selectedIdx < sorted.length &&
      onDirectConnect
    ) {
      // D — open in Direct Connect tab with prefilled address + auto-query
      e.preventDefault();
      const fav = sorted[selectedIdx];
      const sv = findServer(fav);
      onDirectConnect(fav.ip, sv ? sv.game_port : fav.port, sv ? sv.query_port : undefined);
    }
  }

  // ── Copy IP ───────────────────────────────────────────────────────────────
  let copiedKey = $state('');
  async function copyIp(e: MouseEvent, ip: string, port: number) {
    e.stopPropagation();
    const text = `${ip}:${port}`;
    await writeText(text);
    copiedKey = text;
    setTimeout(() => {
      if (copiedKey === text) copiedKey = '';
    }, 1500);
  }

  // ── Helpers ───────────────────────────────────────────────────────────────
  function timeIcon(time: string | undefined): string {
    if (!time) return 'ph:sun-horizon';
    const h = parseInt(time.split(':')[0], 10);
    if (isNaN(h)) return 'ph:sun-horizon';
    if (h >= 5 && h < 7) return 'ph:sun-horizon';
    if (h >= 7 && h < 19) return 'ph:sun';
    if (h >= 19 && h < 21) return 'ph:sun-horizon';
    return 'ph:moon';
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
</script>

<div class="flex h-full overflow-hidden" role="region" tabindex="-1">
  <!-- Table -->
  <div class="flex flex-col flex-1 overflow-hidden">
    {#if favorites.length === 0}
      <div class="flex flex-col items-center justify-center h-full gap-3 text-base-content/40">
        <Icon icon="ph:star" class="size-10 opacity-30" />
        <span class="text-sm">{m.fav_no_favorites()}</span>
        <button class="btn btn-sm btn-outline btn-primary gap-1.5" onclick={onGoToServers}>
          <Icon icon="mdi:server" class="size-3.5" />
          {m.fav_browse_servers()}
        </button>
      </div>
    {:else}
      <div class="overflow-auto flex-1">
        <table class="w-full text-xs" style="table-layout: fixed; border-collapse: collapse;">
          <thead class="sticky top-0 z-10">
            <tr
              class="bg-base-200/95 backdrop-blur-sm text-base-content/50 uppercase tracking-wider border-b border-base-300 select-none"
              style="font-size:10px;"
            >
              <th
                class="px-3 py-2 text-left cursor-pointer hover:text-base-content transition-colors"
                onclick={() => toggleSort('name')}
              >
                <span class="flex items-center gap-1">
                  {m.servers_col_server()}
                  <span class="normal-case font-normal text-base-content/35 ml-0.5">{favorites.length}</span>
                  <Icon icon={sortIcon('name')} class="size-2.5" />
                </span>
              </th>
              <th
                class="w-32 px-3 py-2 cursor-pointer hover:text-base-content transition-colors"
                onclick={() => toggleSort('players')}
              >
                <span class="flex items-center gap-1"
                  >{m.servers_col_players()} <Icon icon={sortIcon('players')} class="size-2.5" /></span
                >
              </th>
              <th
                class="w-20 px-3 py-2 cursor-pointer hover:text-base-content transition-colors"
                onclick={() => toggleSort('ping')}
              >
                <span class="flex items-center gap-1"
                  >{m.servers_col_ping()} <Icon icon={sortIcon('ping')} class="size-2.5" /></span
                >
              </th>
              <th class="w-28 px-3 py-2 text-left font-medium">{m.servers_col_map()}</th>
              <th class="w-16 px-3 py-2 font-medium text-left" title={m.servers_col_time_title()}
                >{m.servers_col_time()}</th
              >
              <th class="w-14 px-3 py-2 text-center font-medium">{m.servers_col_mods()}</th>
              <th class="w-8 px-2 py-2 text-center font-medium">{m.servers_col_os()}</th>
              <th class="w-40 px-3 py-2"></th>
            </tr>
          </thead>
          <tbody>
            {#each sorted as fav, fi}
              {@const server = findServer(fav)}
              {@const ping = pingCache.get(pingKey(fav))}
              {@const isSelected = detailFav?.ip === fav.ip && detailFav?.port === fav.port}
              {@const isFocused = fi === selectedIdx}
              {@const favKey2 = favA2sKey(fav)}
              {@const livePlayers = server ? (a2sPlayers.get(favKey2) ?? server.players) : 0}
              {@const loadingPlayers = a2sPlayersLoading.has(favKey2)}
              {@const pct = server && server.max_players > 0 ? Math.round((livePlayers / server.max_players) * 100) : 0}
              <tr
                class="group/row border-b border-base-300/40 transition-colors cursor-pointer
                       {isSelected
                  ? 'bg-primary/10 border-primary/20'
                  : isFocused
                    ? 'bg-base-200/80 outline outline-1 outline-primary/40'
                    : 'hover:bg-base-200/60'}"
                onclick={() => {
                  selectedIdx = fi;
                }}
                ondblclick={() => onConnect(fav.ip, fav.port, fav.name)}
              >
                <!-- Server name + IP -->
                <td class="px-3 py-2 max-w-0">
                  <div class="flex items-center gap-1.5 min-w-0">
                    <span class="truncate font-medium text-base-content/90">{fav.name}</span>
                    {#if !server}
                      <span class="shrink-0 text-warning" style="font-size:9px;" title={m.fav_server_offline_hint()}
                        >{m.fav_server_offline()}</span
                      >
                    {/if}
                  </div>
                  <div class="flex items-center gap-2 mt-0.5">
                    <button
                      class="font-mono flex items-center gap-1 group/ip
                             {copiedKey === `${fav.ip}:${fav.port}`
                        ? 'text-success'
                        : 'text-base-content/30 hover:text-base-content/60'}"
                      style="font-size:10px;"
                      onclick={(e) => copyIp(e, fav.ip, fav.port)}
                      title={m.servers_copy_ip({ address: `${fav.ip}:${fav.port}` })}
                    >
                      {fav.ip}:{fav.port}
                      <Icon
                        icon={copiedKey === `${fav.ip}:${fav.port}` ? 'ph:check' : 'ph:copy'}
                        class="size-2 opacity-0 group-hover/ip:opacity-100 transition-opacity {copiedKey ===
                        `${fav.ip}:${fav.port}`
                          ? 'opacity-100'
                          : ''}"
                      />
                    </button>
                    {#if server}
                      <span class="text-base-content/25" style="font-size:10px;">{server.version}</span>
                    {/if}
                  </div>
                </td>

                <!-- Players + bar — click to refresh via A2S -->
                <td class="px-3 py-2">
                  {#if server}
                    <div class="flex items-center gap-2">
                      <button
                        class="tabular-nums font-mono {playerFill(
                          livePlayers,
                          server.max_players,
                        )} w-14 shrink-0 cursor-pointer hover:opacity-70 transition-opacity text-left"
                        onclick={(e) => {
                          e.stopPropagation();
                          doRefreshPlayers(fav);
                        }}
                        title={m.servers_click_refresh_players()}
                      >
                        {#if loadingPlayers}
                          <span class="loading loading-spinner" style="width:10px;height:10px;"></span>
                        {:else}
                          {livePlayers}<span class="text-base-content/30">/{server.max_players}</span>
                        {/if}
                      </button>
                      <div class="flex-1 h-1 rounded-full bg-base-300 overflow-hidden">
                        <div
                          class="h-full rounded-full {playerBarColor(livePlayers, server.max_players)}"
                          style="width:{pct}%"
                        ></div>
                      </div>
                    </div>
                  {:else}
                    <span class="text-base-content/25 font-mono">—</span>
                  {/if}
                </td>

                <!-- Ping — click to re-ping -->
                <td class="px-3 py-2">
                  <button
                    class="flex items-center gap-1.5 cursor-pointer hover:opacity-70 transition-opacity {pingFlash.has(
                      pingKey(fav),
                    )
                      ? 'ping-flash'
                      : ''}"
                    onclick={(e) => {
                      e.stopPropagation();
                      doPing(fav);
                    }}
                    title={m.servers_click_ping()}
                  >
                    <span class="size-1.5 rounded-full shrink-0 {pingDot(ping)}"></span>
                    <span class="tabular-nums font-mono {pingColor(ping)}">
                      {pingLabel(ping)}
                    </span>
                  </button>
                </td>

                <!-- Map -->
                <td class="px-3 py-2 max-w-0">
                  <span class="truncate block text-amber-500/80">{server ? server.map : '—'}</span>
                </td>

                <!-- Time -->
                <td class="px-3 py-2">
                  <span class="flex items-center gap-1 text-base-content/60 tabular-nums font-mono">
                    <Icon icon={timeIcon(server?.time)} class="size-3 shrink-0" />
                    {server?.time || '—'}
                  </span>
                </td>

                <!-- Mods -->
                <td class="px-3 py-2 text-center">
                  {#if server && server.mods_count > 0}
                    <span class="inline-flex items-center gap-0.5 text-violet-400/90">
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
                      <span title={m.servers_os_windows()}><Icon icon="devicon:windows11" class="size-3.5" /></span>
                    {:else}
                      <span title={m.servers_os_linux()}><Icon icon="flat-color-icons:linux" class="size-3.5" /></span>
                    {/if}
                  {:else}
                    <span class="text-base-content/20">—</span>
                  {/if}
                </td>

                <!-- Actions — always visible -->
                <td class="px-2 py-2">
                  <div class="flex gap-1 items-center justify-end">
                    <!-- Info toggle -->
                    <button
                      class="size-6 rounded flex items-center justify-center transition-colors
                             {isSelected
                        ? 'bg-primary/15 text-primary hover:bg-primary/25'
                        : 'text-base-content/35 hover:bg-base-300 hover:text-base-content/80'}"
                      title={isSelected ? m.fav_close_details() : m.fav_live_details()}
                      onclick={() => (isSelected ? closeDetail() : openDetail(fav))}
                    >
                      <Icon icon="ph:info" class="size-3.5" />
                    </button>
                    <!-- Remove -->
                    <button
                      class="size-6 rounded flex items-center justify-center text-base-content/35 hover:bg-error/10 hover:text-error transition-colors"
                      title={m.servers_remove_favorite()}
                      onclick={() => onRemove(fav)}
                    >
                      <Icon icon="ph:trash" class="size-3.5" />
                    </button>
                    <!-- Connect -->
                    <button
                      class="btn btn-primary btn-xs h-6 min-h-0 px-2.5 text-xs font-medium"
                      title={m.servers_connect_title()}
                      onclick={() => onConnect(fav.ip, fav.port, fav.name)}
                    >
                      {m.servers_connect()}
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
        <button class="btn btn-ghost btn-xs p-0.5" onclick={closeDetail} title={m.fav_close()}>
          <Icon icon="ph:x" class="size-3.5" />
        </button>
      </div>

      <div class="flex-1 flex flex-col min-h-0">
        {#if a2sLoading}
          <div class="flex items-center justify-center py-8 gap-2 text-base-content/50">
            <span class="loading loading-spinner loading-sm"></span>
            <span class="text-xs">{m.fav_querying()}</span>
          </div>
        {:else if a2sError}
          <div
            class="m-3 flex items-start gap-2 px-2.5 py-2 rounded-lg bg-error/10 border border-error/25 text-xs text-error"
          >
            <Icon icon="ph:warning-circle" class="size-3.5 shrink-0 mt-0.5" />
            <span class="leading-snug break-all">{a2sError}</span>
          </div>
        {:else if a2s}
          <!-- Fixed top section: stats + players -->
          <div class="flex-shrink-0 p-3 space-y-3">
            <!-- Stats grid -->
            <div class="grid grid-cols-2 gap-x-3 gap-y-2 text-xs">
              <span class="flex items-center gap-1.5 text-base-content/50">
                <Icon icon="mdi:controller" class="size-3.5 shrink-0" />{m.detail_players()}
              </span>
              <span class="font-mono font-medium {playerFill(a2s.players, a2s.max_players)}">
                {a2s.players}/{a2s.max_players}
              </span>

              <span class="flex items-center gap-1.5 text-base-content/50">
                <Icon icon="mdi:map-outline" class="size-3.5 shrink-0" />{m.detail_map()}
              </span>
              <span class="text-amber-500/80">{a2s.map}</span>

              <span class="flex items-center gap-1.5 text-base-content/50">
                <Icon icon="mdi:tag-outline" class="size-3.5 shrink-0" />{m.detail_version()}
              </span>
              <span class="text-base-content/70">{a2s.version}</span>

              <span class="flex items-center gap-1.5 text-base-content/50">
                <Icon icon="mdi:gamepad-variant-outline" class="size-3.5 shrink-0" />{m.detail_a2s_game()}
              </span>
              <span class="text-base-content/70">{a2s.game}</span>

              <span class="flex items-center gap-1.5 text-base-content/50">
                <Icon icon="mdi:signal" class="size-3.5 shrink-0" />{m.detail_ping()}
              </span>
              <button
                class="font-mono cursor-pointer hover:opacity-70 transition-opacity {pingColor(
                  pingCache.get(pingKey(detailFav)),
                )}"
                onclick={() => detailFav && doPing(detailFav)}
                title={m.servers_click_ping()}
              >
                {pingLabel(pingCache.get(pingKey(detailFav)))}
              </button>
            </div>

            <!-- Online players -->
            {#if a2s.players_list.length > 0}
              <div>
                <div class="flex items-center gap-1.5 text-xs text-base-content/40 mb-1.5">
                  <Icon icon="mdi:account-multiple-outline" class="size-3.5" />
                  <span>{m.fav_online_count({ count: a2s.players_list.length })}</span>
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
              <p class="text-xs text-base-content/30 text-center py-1">{m.detail_a2s_no_players()}</p>
            {:else}
              <p class="text-xs text-base-content/30 text-center py-1">{m.detail_a2s_names_not_reported()}</p>
            {/if}
          </div>

          <!-- Mod list — fills all remaining height -->
          {#if a2s.mods.length > 0}
            <div class="flex flex-col flex-1 min-h-0 border-t border-base-300">
              <div class="flex items-center gap-1.5 text-xs text-base-content/40 px-3 py-2 flex-shrink-0">
                <Icon icon="mdi:puzzle-outline" class="size-3.5" />
                <span>{m.detail_mods_count({ count: a2s.mods.length })}</span>
              </div>
              <div class="flex-1 overflow-y-auto px-3 pb-2 space-y-1">
                {#each a2s.mods as mod}
                  <div class="flex items-center gap-1.5 text-xs">
                    <Icon icon="mdi:puzzle-outline" class="size-3 text-secondary shrink-0" />
                    <button
                      class="truncate text-base-content/80 hover:text-primary transition-colors text-left"
                      onclick={() =>
                        openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.steam_workshop_id}`)}
                      title="{m.detail_open_workshop()}: {mod.name}">{mod.name}</button
                    >
                    <button
                      class="ml-auto shrink-0 font-mono text-xs text-base-content/30 hover:text-primary transition-colors flex items-center gap-0.5"
                      onclick={() =>
                        openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.steam_workshop_id}`)}
                      title={m.detail_open_workshop()}
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

      <BattleMetricsPanel
        {bm}
        {bmLoading}
        {bmError}
        {bmApiKey}
        {userLocation}
        onRetry={() => {
          bmFetchedKey = '';
          bmRetryTick++;
        }}
      />

      <!-- Refresh A2S button -->
      <div class="px-3 py-2 border-t border-base-300 flex-shrink-0">
        <button
          class="btn btn-ghost btn-xs w-full gap-1.5"
          title={m.fav_refresh_a2s_title()}
          onclick={() => detailFav && openDetail(detailFav)}
          disabled={a2sLoading}
        >
          <Icon icon="ph:arrows-clockwise" class="size-3.5" />
          {m.fav_refresh_a2s()}
        </button>
      </div>
    </div>
  {/if}
</div>

<style></style>
