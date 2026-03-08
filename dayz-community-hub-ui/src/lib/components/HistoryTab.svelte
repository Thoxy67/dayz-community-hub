<script lang="ts">
  import type { HistoryDto, ServerDto, A2sDetailsDto } from '$lib/types';
  import {
    pingLabel,
    pingColor,
    pingDot,
    playerFill,
    playerBarColor,
    formatDuration,
    sortIcon as _sortIcon,
    timeIcon,
    isTimeout,
  } from '$lib/utils';
  import { formatRelativeTime } from '$lib/utils/i18n';
  import { createCopyState } from '$lib/utils/clipboard.svelte';
  import { createServerLookup, findServer as findServerByKey } from '$lib/utils/server-lookup';
  import ServerDetailPanel from './ServerDetailPanel.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { app } from '$lib/state.svelte';
  import { serverData, pingService } from '$lib/services';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '@iconify/svelte';
  import { onMount } from 'svelte';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    history: HistoryDto[];
    servers: ServerDto[];
    pingCache: Map<string, number>;
    pingPending: Set<string>;
    pingTimeouts: Map<string, number>;
    a2sFailures: Set<string>;
    favorites: Set<string>; // "ip:port" keys
    /** BattleMetrics personal access token (null = not configured). */
    bmApiKey: string | null;
    /** User location [longitude, latitude] for distance calculation. */
    userLocation?: [number, number] | null;
    onConnect: (ip: string, port: number, name: string) => void;
    onAddFavorite: (h: HistoryDto) => void;
    onRemoveFavorite: (h: HistoryDto) => void;
    onRemove: (h: HistoryDto) => void;
    onClearAll: () => void;
    onGoToServers?: () => void;
    onPing: (ip: string, port: number) => void;
    /** D key — open selected server in Direct Connect tab with query. */
    onDirectConnect?: (ip: string, gamePort: number, queryPort?: number) => void;
  }

  let {
    history,
    servers,
    pingCache,
    pingPending,
    pingTimeouts,
    a2sFailures,
    favorites,
    bmApiKey,
    userLocation = null,
    onConnect,
    onAddFavorite,
    onRemoveFavorite,
    onRemove,
    onClearAll,
    onGoToServers,
    onPing,
    onDirectConnect,
  }: Props = $props();

  // ── Virtual scrolling ────────────────────────────────────────────────────
  const ROW_HEIGHT = 48;
  const OVERSCAN = 5; // Reduced for 4K screens with fewer visible rows
  let scrollContainer: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let containerHeight = $state(600);

  function handleScroll() {
    if (scrollContainer) scrollTop = scrollContainer.scrollTop;
  }

  $effect(() => {
    if (!scrollContainer) return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) containerHeight = entry.contentRect.height;
    });
    ro.observe(scrollContainer);
    containerHeight = scrollContainer.clientHeight;
    return () => ro.disconnect();
  });

  // Pre-built lookup map rebuilt only when `servers` changes (O(n) once).
  // Covers both query_port and game_port keys so per-row lookups are O(1).
  let serverByKey = $derived(createServerLookup(servers));

  function findServer(h: HistoryDto): ServerDto | null {
    return findServerByKey(serverByKey, h.ip, h.port);
  }

  /**
   * Check if a history entry matches a favorite.
   * Favorites are stored with the game port; history with the query port (game port - 1).
   * Also cross-check via the live server list which knows both ports.
   */
  function isFav(h: HistoryDto): boolean {
    const ip = h.ip;
    const p = h.port;
    // Direct match (same port stored)
    if (favorites.has(`${ip}:${p}`)) return true;
    // Game port is typically query port + 1
    if (favorites.has(`${ip}:${p + 1}`)) return true;
    // Resolve via server list — the server knows both game_port and query_port
    const sv = findServer(h);
    if (sv) {
      if (favorites.has(`${ip}:${sv.game_port}`)) return true;
      if (favorites.has(`${ip}:${sv.query_port}`)) return true;
    }
    return false;
  }

  /**
   * Return the port that was actually stored in favorites for this history entry,
   * so remove_favorite hits the right record.
   */
  function favPort(h: HistoryDto): number {
    const ip = h.ip;
    const p = h.port;
    if (favorites.has(`${ip}:${p}`)) return p;
    if (favorites.has(`${ip}:${p + 1}`)) return p + 1;
    const sv = findServer(h);
    if (sv) {
      if (favorites.has(`${ip}:${sv.game_port}`)) return sv.game_port;
      if (favorites.has(`${ip}:${sv.query_port}`)) return sv.query_port;
    }
    return p;
  }

  type SortCol = 'name' | 'players' | 'ping' | 'last';
  let sortCol = $state<SortCol>('last');
  let sortAsc = $state(false); // most recent first by default

  function toggleSort(col: SortCol) {
    if (sortCol === col) {
      sortAsc = !sortAsc;
    } else {
      sortCol = col;
      // text cols default ascending; numeric/time cols default descending
      sortAsc = col === 'name';
    }
  }

  function sortIcon(col: SortCol) {
    return _sortIcon(col, sortCol, sortAsc);
  }

  let sorted = $derived(
    (() => {
      const arr = history.slice();
      const dir = sortAsc ? 1 : -1;
      arr.sort((a, b) => {
        switch (sortCol) {
          case 'name':
            return dir * a.name.localeCompare(b.name);
          case 'players': {
            const sa = findServer(a);
            const sb = findServer(b);
            const pa = sa ? sa.players : -1;
            const pb = sb ? sb.players : -1;
            return dir * (pa - pb);
          }
          case 'ping': {
            const pa = pingCache.get(pingKey(a)) ?? Infinity;
            const pb = pingCache.get(pingKey(b)) ?? Infinity;
            return dir * (pa - pb);
          }
          case 'last':
            return dir * (a.ts - b.ts);
          default:
            return 0;
        }
      });
      return arr;
    })(),
  );

  // ── Virtual scrolling derived state ─────────────────────────────────────
  let totalHeight = $derived(sorted.length * ROW_HEIGHT);
  let startIndex = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN));
  let endIndex = $derived(Math.min(sorted.length, Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT) + OVERSCAN));
  let visibleHistory = $derived(sorted.slice(startIndex, endIndex));
  let offsetY = $derived(startIndex * ROW_HEIGHT);

  /** Canonical ping cache key: always ip:query_port, matching what start_pinging stores. */
  function pingKey(entry: HistoryDto): string {
    const sv = findServer(entry);
    return `${entry.ip}:${sv ? sv.query_port : entry.port}`;
  }

  async function doRefreshPlayers(entry: HistoryDto) {
    const sv = findServer(entry);
    const queryPort = sv ? sv.query_port : entry.port;
    await serverData.refreshPlayers(entry.ip, queryPort);
  }

  // Auto-ping history entries on mount/change
  // Stops re-pinging after 3 consecutive timeouts
  $effect(() => {
    const needsPing = history
      .filter((h) => {
        const key = pingKey(h);
        // Skip if pending
        if (pingPending.has(key)) return false;
        const cached = pingCache.get(key);
        // Skip if has valid (non-timeout) result
        if (cached !== undefined && !isTimeout(cached)) return false;
        // Skip if timed out too many times (configurable, 0 = disabled)
        const maxRetries = app.profile?.ping_max_retries ?? 3;
        const timeoutCount = pingTimeouts.get(key) ?? 0;
        if (maxRetries > 0 && timeoutCount >= maxRetries) return false;
        return true;
      })
      .map((h) => pingKey(h));
    if (needsPing.length > 0) {
      // Mark as pending
      for (const key of needsPing) {
        app.pingPending.add(key);
      }
      app.pingPending = new Set(app.pingPending);
      invoke('ping_servers', { targets: needsPing }).catch(() => {});
    }
  });

  function doPing(entry: HistoryDto) {
    const sv = findServer(entry);
    const port = sv ? sv.query_port : entry.port;
    const key = pingKey(entry);
    onPing(entry.ip, port);
    pingService.triggerFlash(key);
  }

  const { copiedKey, copy: copyToClipboard } = createCopyState();
  async function copyIp(e: MouseEvent, ip: string, port: number) {
    e.stopPropagation();
    await copyToClipboard(`${ip}:${port}`);
  }

  // ── A2S detail panel ─────────────────────────────────────────────────────
  let detailEntry = $state<HistoryDto | null>(null);
  let a2s = $state<A2sDetailsDto | null>(null);
  let a2sLoading = $state(false);
  let a2sError = $state('');

  async function openDetail(entry: HistoryDto) {
    detailEntry = entry;
    a2s = null;
    a2sError = '';
    a2sLoading = true;
    try {
      // Use query port from live server list if available, else history port directly.
      const sv = findServer(entry);
      const queryPort = sv ? sv.query_port : entry.port;
      // Use serverData service to properly track A2S failures
      const result = await serverData.refreshA2s(entry.ip, queryPort);
      if (result) {
        a2s = result;
      } else {
        a2sError = serverData.getA2sError(entry.ip, queryPort) ?? 'A2S query failed';
      }
    } catch (e) {
      a2sError = String(e);
    } finally {
      a2sLoading = false;
    }
  }

  function closeDetail() {
    detailEntry = null;
    a2s = null;
    a2sError = '';
  }

  // Auto-open detail for first entry when triggered from quick-connect banner
  $effect(() => {
    if (app.historyAutoOpenDetail && history.length > 0) {
      selectedIdx = 0;
      openDetail(history[0]);
      app.historyAutoOpenDetail = false;
    }
  });

  let selectedIdx = $state(-1);

  function scrollToIndex(idx: number) {
    if (!scrollContainer) return;
    const rowTop = idx * ROW_HEIGHT;
    const rowBot = rowTop + ROW_HEIGHT;
    const theadHeight = 32;
    if (rowTop < scrollContainer.scrollTop + theadHeight) {
      scrollContainer.scrollTop = rowTop - theadHeight;
    } else if (rowBot > scrollContainer.scrollTop + containerHeight) {
      scrollContainer.scrollTop = rowBot - containerHeight;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

    if (e.key === 'Escape' && detailEntry) {
      closeDetail();
      e.preventDefault();
      return;
    }
    const len = sorted.length;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (len === 0) return;
      selectedIdx = e.key === 'ArrowDown' ? Math.min(selectedIdx + 1, len - 1) : Math.max(selectedIdx - 1, 0);
      scrollToIndex(selectedIdx);
    }
    if (e.key === 'Enter' && selectedIdx >= 0 && selectedIdx < len) {
      e.preventDefault();
      const entry = sorted[selectedIdx];
      onConnect(entry.ip, entry.port, entry.name);
    }
    if ((e.key === 'f' || e.key === 'F') && !e.ctrlKey && selectedIdx >= 0 && selectedIdx < len) {
      // F — toggle favorite for the selected history entry
      e.preventDefault();
      const entry = sorted[selectedIdx];
      const sv = findServer(entry);
      const queryPort = sv ? sv.query_port : entry.port;
      const key = `${entry.ip}:${queryPort}`;
      if (favorites.has(key)) {
        onRemoveFavorite(entry);
      } else {
        onAddFavorite(entry);
      }
    }
    if ((e.key === 'i' || e.key === 'I') && !e.ctrlKey && selectedIdx >= 0 && selectedIdx < len) {
      // I — toggle info/detail panel
      e.preventDefault();
      const entry = sorted[selectedIdx];
      if (detailEntry?.ip === entry.ip && detailEntry?.port === entry.port) {
        closeDetail();
      } else {
        openDetail(entry);
      }
    }
    if ((e.key === 'p' || e.key === 'P') && !e.ctrlKey && selectedIdx >= 0 && selectedIdx < len) {
      // P — ping the selected server
      e.preventDefault();
      doPing(sorted[selectedIdx]);
    }
    if ((e.key === 'd' || e.key === 'D') && !e.ctrlKey && selectedIdx >= 0 && selectedIdx < len && onDirectConnect) {
      // D — open in Direct Connect tab with prefilled address + auto-query
      e.preventDefault();
      const entry = sorted[selectedIdx];
      const sv = findServer(entry);
      onDirectConnect(entry.ip, sv ? sv.game_port : entry.port, sv ? sv.query_port : undefined);
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
</script>

<div class="flex h-full overflow-hidden" role="region" tabindex="-1">
  <div class="flex flex-col flex-1 overflow-hidden">
    {#if history.length === 0}
      <div class="flex flex-col items-center justify-center h-full gap-3 text-base-content/40">
        <Icon icon="ph:clock-clockwise" class="size-10 opacity-30" />
        <span class="text-sm">{m.history_no_history()}</span>
        <button class="btn btn-sm btn-outline btn-primary gap-1.5" onclick={onGoToServers}>
          <Icon icon="mdi:server" class="size-3.5" />
          {m.fav_browse_servers()}
        </button>
      </div>
    {:else}
      <div class="overflow-auto flex-1" bind:this={scrollContainer} onscroll={handleScroll}>
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
                  <span class="normal-case font-normal text-base-content/35 ml-0.5">{history.length}</span>
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
              <th class="w-28 px-3 py-2 font-medium text-left">{m.servers_col_map()}</th>
              <th class="w-16 px-3 py-2 font-medium text-left" title={m.servers_col_time_title()}
                >{m.servers_col_time()}</th
              >
              <th
                class="w-36 px-3 py-2 cursor-pointer hover:text-base-content transition-colors text-left"
                onclick={() => toggleSort('last')}
              >
                <span class="flex items-center gap-1"
                  >{m.history_col_last_played()} <Icon icon={sortIcon('last')} class="size-2.5" /></span
                >
              </th>
              <th class="w-40 px-3 py-2"></th>
            </tr>
          </thead>
          <tbody>
            {#if offsetY > 0}
              <tr><td colspan="7" class="p-0 border-0" style="height:{offsetY}px"></td></tr>
            {/if}
            {#each visibleHistory as entry, vi}
              {@const ei = startIndex + vi}
              {@const server = findServer(entry)}
              {@const pk = pingKey(entry)}
              {@const ping = pingCache.get(pk)}
              {@const isPingPending = pingPending.has(pk)}
              {@const timeoutCount = pingTimeouts.get(pk) ?? 0}
              {@const hasA2sFailure = a2sFailures.has(pk)}
              {@const isFull = server ? server.players > 0 && server.players === server.max_players : false}
              {@const queryPort = server ? server.query_port : entry.port}
              {@const playerData = serverData.getPlayers(entry.ip, queryPort)}
              {@const livePlayers = playerData.players}
              {@const loadingPlayers = serverData.isA2sLoading(entry.ip, queryPort)}
              {@const pct = server && server.max_players > 0 ? Math.round((livePlayers / server.max_players) * 100) : 0}
              {@const isFocused = ei === selectedIdx}
              {@const isSelected = detailEntry?.ip === entry.ip && detailEntry?.port === entry.port}
              <tr
                class="group/row border-b border-base-300/40 transition-colors cursor-pointer
                     {isSelected
                  ? 'bg-primary/10 border-primary/20'
                  : isFocused
                    ? 'bg-base-200/80 outline outline-1 outline-primary/40'
                    : 'hover:bg-base-200/60'}"
                style="height:{ROW_HEIGHT}px"
                onclick={() => (selectedIdx = ei)}
                ondblclick={() => onConnect(entry.ip, entry.port, entry.name)}
              >
                <!-- Server name + IP -->
                <td class="px-3 py-2 max-w-0">
                  <div class="flex items-center gap-1.5 min-w-0">
                    <span class="truncate font-medium text-base-content/90">{entry.name}</span>
                    {#if !server}
                      <span class="shrink-0 text-warning" style="font-size:9px;" title={m.fav_server_offline_hint()}
                        >{m.fav_server_offline()}</span
                      >
                    {/if}
                  </div>
                  <div class="flex items-center gap-2 mt-0.5">
                    <button
                      class="font-mono flex items-center gap-1 group/ip
                           {copiedKey === `${entry.ip}:${entry.port}`
                        ? 'text-success'
                        : 'text-base-content/30 hover:text-base-content/60'}"
                      style="font-size:10px;"
                      onclick={(e) => copyIp(e, entry.ip, entry.port)}
                      title={m.servers_copy_ip({ address: `${entry.ip}:${entry.port}` })}
                    >
                      {entry.ip}:{entry.port}
                      <Icon
                        icon={copiedKey === `${entry.ip}:${entry.port}` ? 'ph:check' : 'ph:copy'}
                        class="size-2 opacity-0 group-hover/ip:opacity-100 transition-opacity {copiedKey ===
                        `${entry.ip}:${entry.port}`
                          ? 'opacity-100'
                          : ''}"
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
                    <button
                      class="flex items-center gap-2 w-full cursor-pointer hover:opacity-70 transition-opacity text-left"
                      onclick={(e) => {
                        e.stopPropagation();
                        doRefreshPlayers(entry);
                      }}
                      title={m.servers_click_refresh_players()}
                      disabled={loadingPlayers}
                    >
                      {#if loadingPlayers}
                        <span class="loading loading-spinner loading-xs text-primary shrink-0"></span>
                      {:else}
                        <span
                          class="tabular-nums font-mono {playerFill(livePlayers, server.max_players)} w-14 shrink-0"
                        >
                          {livePlayers}<span class="text-base-content/30">/{server.max_players}</span>
                        </span>
                      {/if}
                      <div class="flex-1 h-1 rounded-full bg-base-300 overflow-hidden">
                        <div
                          class="h-full rounded-full {playerBarColor(livePlayers, server.max_players)}"
                          style="width:{pct}%"
                        ></div>
                      </div>
                    </button>
                  {:else}
                    <span class="text-base-content/25 font-mono">—</span>
                  {/if}
                </td>

                <!-- Ping — click to re-ping -->
                <td class="px-3 py-2">
                  <button
                    class="flex items-center gap-1.5 cursor-pointer hover:opacity-70 transition-opacity {pingService.hasFlash(
                      pk,
                    )
                      ? 'ping-flash'
                      : ''}"
                    onclick={(e) => {
                      e.stopPropagation();
                      doPing(entry);
                    }}
                    title={m.servers_click_ping()}
                  >
                    {#if isPingPending}
                      <Icon icon="svg-spinners:pulse-3" class="size-4 text-base-content/50" />
                    {:else if ping === undefined}
                      <Icon icon="icon-park-outline:dot" class="size-3 text-base-content/30" />
                    {:else}
                      <span class="size-1.5 rounded-full shrink-0 {pingDot(ping)}"></span>
                      <span class="tabular-nums font-mono {pingColor(ping)}">
                        {pingLabel(ping)}
                      </span>
                      {#if isFull && hasA2sFailure}
                        <Icon
                          icon="ic:round-warning"
                          class="size-3.5 text-warning"
                          title={m.servers_player_count_unverified()}
                        />
                      {/if}
                    {/if}
                  </button>
                </td>

                <!-- Map -->
                <td class="px-3 py-2 max-w-0">
                  <span class="truncate block text-accent-map">{server ? server.map : '—'}</span>
                </td>

                <!-- Time -->
                <td class="px-3 py-2">
                  <span class="flex items-center gap-1 text-base-content/60 tabular-nums font-mono">
                    <Icon icon={timeIcon(server?.time)} class="size-3 shrink-0" />
                    {server?.time || '—'}
                  </span>
                </td>

                <!-- Last played -->
                <td class="px-3 py-2">
                  <span class="text-base-content/40 cursor-default" title={new Date(entry.ts * 1000).toLocaleString()}
                    >{formatRelativeTime(entry.ts)}</span
                  >
                </td>

                <!-- Actions — always visible -->
                <td class="px-2 py-2">
                  <div class="flex gap-1 items-center justify-end">
                    <!-- Info / A2S detail -->
                    <button
                      class="size-6 rounded flex items-center justify-center transition-colors
                           {isSelected
                        ? 'bg-primary/15 text-primary hover:bg-primary/25'
                        : 'text-base-content/35 hover:bg-base-300 hover:text-base-content/80'}"
                      title={isSelected ? m.fav_close_details() : m.fav_live_details()}
                      onclick={(e) => {
                        e.stopPropagation();
                        isSelected ? closeDetail() : openDetail(entry);
                      }}
                    >
                      <Icon icon="ph:info" class="size-3.5" />
                    </button>
                    <!-- Favorite toggle -->
                    <button
                      class="size-6 rounded flex items-center justify-center transition-colors
                            {isFav(entry)
                        ? 'text-warning hover:bg-error/10 hover:text-error'
                        : 'text-base-content/35 hover:bg-warning/10 hover:text-warning'}"
                      onclick={() =>
                        isFav(entry) ? onRemoveFavorite({ ...entry, port: favPort(entry) }) : onAddFavorite(entry)}
                      title={isFav(entry) ? m.servers_remove_favorite() : m.servers_add_favorite()}
                    >
                      <Icon icon={isFav(entry) ? 'ph:star-fill' : 'ph:star'} class="size-3.5" />
                    </button>
                    <!-- Remove -->
                    <button
                      class="size-6 rounded flex items-center justify-center text-base-content/35 hover:bg-error/10 hover:text-error transition-colors"
                      title={m.history_remove()}
                      onclick={() => onRemove(entry)}
                    >
                      <Icon icon="ph:trash" class="size-3.5" />
                    </button>
                    <!-- Connect -->
                    <button
                      class="btn btn-primary btn-xs gap-1.5"
                      title={m.servers_connect_title()}
                      onclick={() => onConnect(entry.ip, entry.port, entry.name)}
                    >
                      <Icon icon="ph:play-fill" class="size-3.5" />
                      {m.servers_connect()}
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
            {#if totalHeight - endIndex * ROW_HEIGHT > 0}
              <tr><td colspan="7" class="p-0 border-0" style="height:{totalHeight - endIndex * ROW_HEIGHT}px"></td></tr>
            {/if}
          </tbody>
        </table>
      </div>

      <div class="flex justify-end px-3 py-2 bg-base-200 border-t border-base-300 flex-shrink-0">
        <button class="btn btn-error btn-xs btn-outline" title={m.history_clear_all_title()} onclick={onClearAll}>
          {m.history_clear_all()}
        </button>
      </div>
    {/if}
  </div>
  <!-- end flex-col flex-1 -->

  <!-- A2S detail side panel -->
  {#if detailEntry}
    {@const server = findServer(detailEntry)}
    {@const key = server ? `${server.ip}:${server.query_port}` : `${detailEntry.ip}:${detailEntry.port}`}

    <div class="w-80 flex-shrink-0 flex flex-col overflow-hidden">
      <ServerDetailPanel
        {server}
        {a2s}
        {a2sLoading}
        {a2sError}
        installedMods={[]}
        pingMs={pingCache.get(key) ?? null}
        {bmApiKey}
        {userLocation}
        scrollToMods={false}
        onClose={closeDetail}
        onQueryA2s={() => detailEntry && openDetail(detailEntry)}
      />
    </div>
  {/if}
</div>

<style></style>
