<script lang="ts">
  import type { ServerDto, InstalledModDto, A2sDetailsDto, ServersFilterState } from '$lib/types';
  import {
    pingLabel,
    pingColor,
    pingDot,
    playerFill,
    playerBarColor,
    sortIcon as _sortIcon,
    timeIcon,
  } from '$lib/utils';
  import { createCopyState } from '$lib/utils/clipboard.svelte';
  import ServerDetailPanel from './ServerDetailPanel.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Icon from '@iconify/svelte';
  import { onMount } from 'svelte';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    servers: ServerDto[];
    pingCache: Map<string, number>;
    installedMods: InstalledModDto[];
    favorites: Set<string>; // "ip:port" keys
    /** Set of excluded IPs (bare IPs, no port). */
    excludedIps: Set<string>;
    loading: boolean;
    filter: ServersFilterState;
    /** BattleMetrics personal access token (null = not configured). */
    bmApiKey: string | null;
    /** User location [longitude, latitude] for distance calculation. */
    userLocation?: [number, number] | null;
    onConnect: (server: ServerDto) => void;
    onAddFavorite: (server: ServerDto) => void;
    onRemoveFavorite: (server: ServerDto) => void;
    onRefresh: () => void;
    onPing: (ip: string, port: number) => void;
    onExcludeIp: (ip: string) => void;
    onUnexcludeIp: (ip: string) => void;
    onManageExcluded: () => void;
    /** D key — open selected server in Direct Connect tab with query. */
    onDirectConnect?: (ip: string, gamePort: number, queryPort?: number) => void;
  }

  let {
    servers,
    pingCache,
    installedMods,
    favorites,
    excludedIps,
    loading,
    filter = $bindable(),
    bmApiKey,
    userLocation = null,
    onConnect,
    onAddFavorite,
    onRemoveFavorite,
    onRefresh,
    onPing,
    onExcludeIp,
    onUnexcludeIp,
    onManageExcluded,
    onDirectConnect,
  }: Props = $props();

  // Deferred version of filter.searchQuery: updated 150ms after the user stops typing.
  let deferredQuery = $state(filter.searchQuery);
  let _searchDebounceTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const q = filter.searchQuery;
    clearTimeout(_searchDebounceTimer);
    _searchDebounceTimer = setTimeout(() => {
      deferredQuery = q;
    }, 150);
    return () => clearTimeout(_searchDebounceTimer);
  });

  type ModFilter = 'both' | 'mods-only' | 'no-mods';
  function cycleMods() {
    const v = filter.filterMods;
    filter.filterMods = v === 'both' ? 'mods-only' : v === 'mods-only' ? 'no-mods' : 'both';
  }

  function getModsLabel(f: ModFilter): string {
    if (f === 'both') return m.servers_filter_mods_all();
    if (f === 'mods-only') return m.servers_filter_mods_only();
    return m.servers_filter_mods_none();
  }
  function getModsTitle(f: ModFilter): string {
    if (f === 'both') return m.servers_filter_mods_title_all();
    if (f === 'mods-only') return m.servers_filter_mods_title_only();
    return m.servers_filter_mods_title_none();
  }

  type FPFilter = 'both' | 'fp-only' | 'no-fp';
  function cycleFP() {
    const v = filter.filterFirstPerson;
    filter.filterFirstPerson = v === 'both' ? 'fp-only' : v === 'fp-only' ? 'no-fp' : 'both';
  }

  function getFPLabel(f: FPFilter): string {
    if (f === 'both') return m.servers_filter_fp_all();
    if (f === 'fp-only') return m.servers_filter_fp_only();
    return m.servers_filter_fp_none();
  }
  function getFPTitle(f: FPFilter): string {
    if (f === 'both') return m.servers_filter_fp_title_all();
    if (f === 'fp-only') return m.servers_filter_fp_title_only();
    return m.servers_filter_fp_title_none();
  }

  type PwdFilter = 'both' | 'no-pwd' | 'pwd-only';
  function cyclePwd() {
    const v = filter.filterPassword;
    filter.filterPassword = v === 'both' ? 'no-pwd' : v === 'no-pwd' ? 'pwd-only' : 'both';
  }

  function getPwdLabel(f: PwdFilter): string {
    if (f === 'both') return m.servers_filter_pwd_all();
    if (f === 'no-pwd') return m.servers_filter_pwd_none();
    return m.servers_filter_pwd_only();
  }
  function getPwdTitle(f: PwdFilter): string {
    if (f === 'both') return m.servers_filter_pwd_title_all();
    if (f === 'no-pwd') return m.servers_filter_pwd_title_none();
    return m.servers_filter_pwd_title_only();
  }

  type BEFilter = 'both' | 'be-only' | 'no-be';
  function cycleBE() {
    const v = filter.filterBE;
    filter.filterBE = v === 'both' ? 'be-only' : v === 'be-only' ? 'no-be' : 'both';
  }

  function getBELabel(f: BEFilter): string {
    if (f === 'both') return m.servers_filter_be_all();
    if (f === 'be-only') return m.servers_filter_be_only();
    return m.servers_filter_be_none();
  }
  function getBETitle(f: BEFilter): string {
    if (f === 'both') return m.servers_filter_be_title_all();
    if (f === 'be-only') return m.servers_filter_be_title_only();
    return m.servers_filter_be_title_none();
  }

  let selectedIndex = $state(0);
  let showDetails = $state(false);
  let scrollToMods = $state(false);
  let a2s = $state<A2sDetailsDto | null>(null);
  let a2sLoading = $state(false);

  /** When true, servers whose IP is in excludedIps are hidden from the list. */
  let hideExcluded = $state(true);

  // ── Long-press on Excluded button → open manage modal ────────────────────
  let excludedLongPressTimer: ReturnType<typeof setTimeout> | null = null;

  function onExcludedPointerDown() {
    excludedLongPressTimer = setTimeout(() => {
      excludedLongPressTimer = null;
      onManageExcluded();
    }, 1500);
  }

  function onExcludedPointerUp() {
    if (excludedLongPressTimer !== null) {
      clearTimeout(excludedLongPressTimer);
      excludedLongPressTimer = null;
      // Short click: toggle visibility as before
      hideExcluded = !hideExcluded;
    }
  }

  function onExcludedPointerLeave() {
    if (excludedLongPressTimer !== null) {
      clearTimeout(excludedLongPressTimer);
      excludedLongPressTimer = null;
    }
  }

  // ── Sorting ──────────────────────────────────────────────────────────────
  type SortCol = 'ping' | 'players' | 'name' | 'map' | 'mods' | 'none';
  // sortCol/sortAsc are read from filter but toggling writes back into filter directly.

  function toggleSort(col: SortCol) {
    if (filter.sortCol === col) {
      filter.sortAsc = !filter.sortAsc;
    } else {
      filter.sortCol = col;
      filter.sortAsc = col === 'name' || col === 'map';
    }
    selectedIndex = 0;
    if (scrollContainer) scrollContainer.scrollTo({ top: 0, behavior: 'smooth' });
    scrollTop = 0;
  }

  function sortIcon(col: SortCol) {
    return _sortIcon(col, filter.sortCol, filter.sortAsc);
  }

  // ── Virtual scrolling state ──────────────────────────────────────────────
  const ROW_HEIGHT = 48;
  const OVERSCAN = 10;
  let scrollContainer: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let containerHeight = $state(600);

  function pingKey(s: ServerDto) {
    return `${s.ip}:${s.query_port}`;
  }

  // Track which servers were just pinged for a brief green flash.
  let pingFlash = $state<Set<string>>(new Set());

  // A2S-refreshed player counts: "ip:queryPort" → live players
  let a2sPlayers = $state<Map<string, number>>(new Map());
  let a2sPlayersLoading = $state<Set<string>>(new Set());

  async function doRefreshPlayers(server: ServerDto) {
    const key = `${server.ip}:${server.query_port}`;
    if (a2sPlayersLoading.has(key)) return;
    a2sPlayersLoading = new Set([...a2sPlayersLoading, key]);
    try {
      const res = await invoke<A2sDetailsDto>('query_a2s', { ip: server.ip, port: server.query_port });
      a2sPlayers = new Map([...a2sPlayers, [key, res.players]]);
    } catch {
      a2sPlayers = new Map([...a2sPlayers, [key, server.players]]);
    } finally {
      a2sPlayersLoading.delete(key);
      a2sPlayersLoading = new Set(a2sPlayersLoading);
    }
  }

  function doPing(server: ServerDto) {
    const key = pingKey(server);
    onPing(server.ip, server.query_port);
    pingFlash.add(key);
    // Force Svelte to see the mutation
    pingFlash = new Set(pingFlash);
    setTimeout(() => {
      pingFlash.delete(key);
      pingFlash = new Set(pingFlash);
    }, 1000);
  }

  // Sorted list of unique map names — rebuilt only when servers changes,
  // not on every render (was previously computed inline in the {#each}).
  let uniqueMaps = $derived([...new Set(servers.map((s) => s.map))].sort());

  let filtered = $derived(
    (() => {
      let list = servers;
      const q = deferredQuery.trim().toLowerCase();
      if (q) {
        list = list.filter(
          (s) => s.name.toLowerCase().includes(q) || s.ip.includes(q) || s.map.toLowerCase().includes(q),
        );
      }
      if (filter.filterFirstPerson === 'fp-only') list = list.filter((s) => s.first_person_only);
      if (filter.filterFirstPerson === 'no-fp') list = list.filter((s) => !s.first_person_only);
      if (filter.filterPassword === 'no-pwd') list = list.filter((s) => !s.password);
      if (filter.filterPassword === 'pwd-only') list = list.filter((s) => s.password);
      if (filter.filterBE === 'be-only') list = list.filter((s) => !!s.battl_eye);
      if (filter.filterBE === 'no-be') list = list.filter((s) => !s.battl_eye);
      if (filter.filterMods === 'mods-only') list = list.filter((s) => s.mods_count > 0);
      if (filter.filterMods === 'no-mods') list = list.filter((s) => s.mods_count === 0);
      if (filter.filterMap) list = list.filter((s) => s.map === filter.filterMap);
      if (hideExcluded && excludedIps.size > 0) list = list.filter((s) => !excludedIps.has(s.ip));
      return list;
    })(),
  );

  // When sorting by ping we need to read pingCache, which updates on every
  // ping result.  For all other sort columns pingCache is irrelevant, so we
  // deliberately avoid reading it here — keeping those sorts cheap.
  let sortedNoPing = $derived(
    (() => {
      if (filter.sortCol === 'none' || filter.sortCol === 'ping') return filtered;
      const arr = filtered.slice();
      const dir = filter.sortAsc ? 1 : -1;
      arr.sort((a, b) => {
        switch (filter.sortCol) {
          case 'players':
            return dir * (a.players - b.players);
          case 'name':
            return dir * a.name.localeCompare(b.name);
          case 'map':
            return dir * a.map.localeCompare(b.map);
          case 'mods':
            return dir * (a.mods_count - b.mods_count);
          default:
            return 0;
        }
      });
      return arr;
    })(),
  );

  // Separate derived that reads pingCache — only invalidated by ping updates
  // when the user has explicitly chosen to sort by ping.
  let sorted = $derived(
    (() => {
      if (filter.sortCol !== 'ping') return sortedNoPing;
      const arr = sortedNoPing.length === filtered.length ? filtered.slice() : sortedNoPing.slice();
      const dir = filter.sortAsc ? 1 : -1;
      arr.sort((a, b) => {
        const pa = pingCache.get(pingKey(a)) ?? Infinity;
        const pb = pingCache.get(pingKey(b)) ?? Infinity;
        return dir * (pa - pb);
      });
      return arr;
    })(),
  );

  let totalHeight = $derived(sorted.length * ROW_HEIGHT);
  let startIndex = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN));
  let endIndex = $derived(Math.min(sorted.length, Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT) + OVERSCAN));
  let visibleServers = $derived(sorted.slice(startIndex, endIndex));
  let offsetY = $derived(startIndex * ROW_HEIGHT);

  let selected = $derived(sorted[selectedIndex] ?? null);

  function favKey(s: ServerDto) {
    return `${s.ip}:${s.query_port}`;
  }

  const { copiedKey, copy: copyToClipboard } = createCopyState();
  async function copyIp(e: MouseEvent, server: ServerDto) {
    e.stopPropagation(); // don't select the row
    await copyToClipboard(`${server.ip}:${server.game_port}`);
  }

  function selectRow(index: number) {
    selectedIndex = index;
    a2s = null;
    a2sError = '';
  }

  let a2sError = $state('');

  async function handleQueryA2s() {
    if (!selected) return;
    a2sLoading = true;
    a2sError = '';
    try {
      a2s = await invoke<A2sDetailsDto>('query_a2s', { ip: selected.ip, port: selected.query_port });
    } catch (e) {
      a2s = null;
      a2sError = String(e);
    } finally {
      a2sLoading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;

    if (e.key === 'ArrowDown') {
      selectedIndex = Math.min(selectedIndex + 1, sorted.length - 1);
      scrollToIndex(selectedIndex);
      e.preventDefault();
    } else if (e.key === 'ArrowUp') {
      selectedIndex = Math.max(selectedIndex - 1, 0);
      scrollToIndex(selectedIndex);
      e.preventDefault();
    } else if (e.key === 'Enter' && selected) {
      onConnect(selected);
    } else if (e.key === 'Escape') {
      if (showDetails) {
        showDetails = false;
        a2s = null;
        a2sError = '';
      } else {
        selectedIndex = -1;
      }
      e.preventDefault();
    } else if ((e.key === 'f' || e.key === 'F') && selected && !e.ctrlKey) {
      // F — toggle favorite for the selected server
      e.preventDefault();
      const key = `${selected.ip}:${selected.query_port}`;
      if (favorites.has(key)) {
        onRemoveFavorite(selected);
      } else {
        onAddFavorite(selected);
      }
    } else if ((e.key === 'i' || e.key === 'I') && selected && !e.ctrlKey) {
      // I — toggle info/detail panel for the selected server
      scrollToMods = false;
      showDetails = !showDetails;
      if (showDetails) handleQueryA2s();
    } else if ((e.key === 'p' || e.key === 'P') && selected && !e.ctrlKey) {
      // P — ping the selected server
      e.preventDefault();
      doPing(selected);
    } else if ((e.key === 'd' || e.key === 'D') && selected && !e.ctrlKey && onDirectConnect) {
      // D — open in Direct Connect tab with prefilled address + auto-query
      e.preventDefault();
      onDirectConnect(selected.ip, selected.game_port, selected.query_port);
    }
  }

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

  function handleScroll() {
    if (scrollContainer) scrollTop = scrollContainer.scrollTop;
  }

  $effect(() => {
    // Re-run whenever the debounced search text or any flag filter changes
    deferredQuery;
    filter.filterFirstPerson;
    filter.filterPassword;
    filter.filterBE;
    filter.filterMods;
    filter.filterMap;
    hideExcluded;
    excludedIps;
    selectedIndex = 0;
    a2s = null;
    if (scrollContainer) scrollContainer.scrollTo({ top: 0, behavior: 'smooth' });
    scrollTop = 0;
  });

  $effect(() => {
    if (!scrollContainer) return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) containerHeight = entry.contentRect.height;
    });
    ro.observe(scrollContainer);
    containerHeight = scrollContainer.clientHeight;
    return () => ro.disconnect();
  });

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
</script>

<div class="flex flex-col h-full" role="grid" tabindex="-1">
  <!-- Toolbar -->
  <div class="flex items-center gap-3 px-3 py-2 bg-base-200 border-b border-base-300 flex-shrink-0">
    <!-- Search input -->
    <label class="input input-sm input-bordered flex items-center gap-2 flex-1 min-w-0">
      <Icon icon="ph:magnifying-glass" class="size-3.5 text-base-content/40 shrink-0" />
      <input
        type="text"
        placeholder={m.servers_search_placeholder()}
        value={filter.searchQuery}
        oninput={(e) => {
          filter.searchQuery = (e.target as HTMLInputElement).value;
        }}
        class="grow bg-transparent outline-none text-sm min-w-0"
      />
      {#if filter.searchQuery}
        <button
          class="btn btn-ghost btn-xs p-0 min-h-0 h-auto shrink-0"
          title={m.servers_clear_search()}
          onclick={() => {
            filter.searchQuery = '';
          }}
        >
          <Icon icon="ph:x" class="size-3" />
        </button>
      {/if}
    </label>

    <!-- Flag filters — grouped pill bar -->
    <div
      class="flex items-center rounded-lg border border-base-300 bg-base-100/50 overflow-hidden divide-x divide-base-300 shrink-0 h-7"
    >
      <!-- 1P -->
      <button
        class="flex items-center gap-1 px-2.5 h-full text-xs font-semibold transition-colors"
        class:bg-warning={filter.filterFirstPerson === 'fp-only'}
        class:text-warning-content={filter.filterFirstPerson === 'fp-only'}
        class:bg-error={filter.filterFirstPerson === 'no-fp'}
        class:text-error-content={filter.filterFirstPerson === 'no-fp'}
        class:opacity-50={filter.filterFirstPerson === 'both'}
        class:hover:opacity-100={filter.filterFirstPerson === 'both'}
        onclick={cycleFP}
        title={getFPTitle(filter.filterFirstPerson as FPFilter)}
      >
        {getFPLabel(filter.filterFirstPerson as FPFilter)}
      </button>
      <!-- Password -->
      <button
        class="flex items-center gap-1.5 px-2.5 h-full text-xs font-medium transition-colors"
        class:bg-error={filter.filterPassword === 'pwd-only'}
        class:text-error-content={filter.filterPassword === 'pwd-only'}
        class:bg-warning={filter.filterPassword === 'no-pwd'}
        class:text-warning-content={filter.filterPassword === 'no-pwd'}
        class:text-base-content={filter.filterPassword === 'both'}
        class:opacity-50={filter.filterPassword === 'both'}
        class:hover:opacity-100={filter.filterPassword === 'both'}
        onclick={cyclePwd}
        title={getPwdTitle(filter.filterPassword as PwdFilter)}
      >
        <Icon icon="mdi:lock" class="size-3 shrink-0" />
        {getPwdLabel(filter.filterPassword as PwdFilter)}
      </button>
      <!-- BattlEye -->
      <button
        class="flex items-center gap-1.5 px-2.5 h-full text-xs font-medium transition-colors"
        class:bg-primary={filter.filterBE === 'be-only'}
        class:text-primary-content={filter.filterBE === 'be-only'}
        class:bg-error={filter.filterBE === 'no-be'}
        class:text-error-content={filter.filterBE === 'no-be'}
        class:opacity-50={filter.filterBE === 'both'}
        class:hover:opacity-100={filter.filterBE === 'both'}
        onclick={cycleBE}
        title={getBETitle(filter.filterBE as BEFilter)}
      >
        <img src="/battleeye.png" alt="BE" class="h-3.5 w-auto rounded-sm shrink-0" />
        {getBELabel(filter.filterBE as BEFilter)}
      </button>
      <!-- Mods -->
      <button
        class="flex items-center gap-1.5 px-2.5 h-full text-xs font-medium transition-colors"
        class:bg-feat-mods={filter.filterMods === 'mods-only'}
        class:text-white={filter.filterMods === 'mods-only'}
        class:bg-error={filter.filterMods === 'no-mods'}
        class:text-error-content={filter.filterMods === 'no-mods'}
        class:opacity-50={filter.filterMods === 'both'}
        class:hover:opacity-100={filter.filterMods === 'both'}
        onclick={cycleMods}
        title={getModsTitle(filter.filterMods as ModFilter)}
      >
        <Icon icon="mdi:puzzle-outline" class="size-3 shrink-0" />
        {getModsLabel(filter.filterMods as ModFilter)}
      </button>
    </div>

    <!-- Map dropdown -->
    <div class="relative shrink-0">
      <select
        class="select select-sm select-bordered h-7 min-h-0 py-0 pr-7 pl-2.5 text-xs rounded-lg appearance-none"
        class:text-base-content={!filter.filterMap}
        class:opacity-50={!filter.filterMap}
        class:text-accent-highlight={!!filter.filterMap}
        class:opacity-100={!!filter.filterMap}
        value={filter.filterMap}
        onchange={(e) => {
          filter.filterMap = (e.target as HTMLSelectElement).value;
        }}
      >
        <option value="">{m.servers_all_maps()}</option>
        {#each uniqueMaps as map}
          <option value={map}>{map}</option>
        {/each}
      </select>
      {#if filter.filterMap}
        <button
          class="absolute right-6 top-1/2 -translate-y-1/2 text-base-content/40 hover:text-base-content transition-colors"
          onclick={() => (filter.filterMap = '')}
          title={m.servers_clear_map_filter()}
        >
          <Icon icon="ph:x" class="size-2.5" />
        </button>
      {/if}
    </div>

    <!-- Server count -->
    <span class="text-xs tabular-nums text-base-content/40 shrink-0">
      {filtered.length}<span class="text-base-content/25">/{servers.length}</span>
    </span>

    <!-- Divider -->
    <div class="w-px h-5 bg-base-300 shrink-0"></div>

    <!-- Utility buttons -->
    <div class="flex items-center gap-1">
      {#if excludedIps.size > 0}
        <button
          class="btn btn-ghost btn-xs gap-1.5 transition-colors select-none"
          class:text-error={hideExcluded}
          class:text-base-content={!hideExcluded}
          onpointerdown={onExcludedPointerDown}
          onpointerup={onExcludedPointerUp}
          onpointerleave={onExcludedPointerLeave}
          title={hideExcluded ? m.servers_excluded_reveal() : m.servers_excluded_hide()}
        >
          <Icon icon={hideExcluded ? 'ph:eye-slash' : 'ph:eye'} class="size-3.5" />
          {hideExcluded ? m.servers_excluded_count({ count: excludedIps.size }) : m.servers_show_all()}
        </button>
      {/if}
      <button
        class="btn btn-ghost btn-xs gap-1.5"
        class:btn-active={showDetails}
        onclick={() => {
          scrollToMods = false;
          showDetails = !showDetails;
        }}
        title={m.servers_toggle_details()}
      >
        <Icon icon="ph:sidebar-simple" class="size-3.5" />
        {m.servers_details()}
      </button>
      <button
        class="btn btn-ghost btn-xs gap-1.5"
        onclick={onRefresh}
        disabled={loading}
        title={m.servers_refresh_title()}
      >
        {#if loading}
          <span class="loading loading-spinner loading-xs"></span>
        {:else}
          <Icon icon="ph:arrows-clockwise" class="size-3.5" />
        {/if}
        {m.servers_refresh()}
      </button>
    </div>
  </div>

  <!-- Main content: table + optional details panel -->
  <div class="flex flex-1 overflow-hidden">
    <!-- Server table with virtual scrolling (wrapper for jump-to-top button) -->
    <div class="relative flex-1 flex flex-col overflow-hidden">
      <div class="flex-1 overflow-auto" bind:this={scrollContainer} onscroll={handleScroll}>
        {#if loading && servers.length === 0}
          <div class="flex items-center justify-center h-full gap-2 text-base-content/50">
            <span class="loading loading-spinner loading-md"></span>
            <span>{m.servers_loading()}</span>
          </div>
        {:else if sorted.length === 0}
          <div class="flex items-center justify-center h-full text-base-content/40">{m.servers_no_match()}</div>
        {:else}
          <table class="w-full text-xs" style="table-layout: fixed; border-collapse: collapse;">
            <thead class="sticky top-0 z-10">
              <tr
                class="bg-base-200/95 backdrop-blur-sm text-base-content/50 uppercase tracking-wider border-b border-base-300 select-none"
                style="font-size:10px;"
              >
                <th class="w-8 px-2 py-2 text-right font-medium">#</th>
                <th class="w-6 px-1 py-2"></th>
                <th
                  class="w-20 px-3 py-2 cursor-pointer hover:text-base-content transition-colors"
                  onclick={() => toggleSort('ping')}
                >
                  <span class="flex items-center gap-1"
                    >{m.servers_col_ping()} <Icon icon={sortIcon('ping')} class="size-2.5" /></span
                  >
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
                  class="px-3 py-2 cursor-pointer hover:text-base-content transition-colors text-left"
                  onclick={() => toggleSort('name')}
                >
                  <span class="flex items-center gap-1"
                    >{m.servers_col_server()} <Icon icon={sortIcon('name')} class="size-2.5" /></span
                  >
                </th>
                <th
                  class="w-28 px-3 py-2 cursor-pointer hover:text-base-content transition-colors"
                  onclick={() => toggleSort('map')}
                >
                  <span class="flex items-center gap-1"
                    >{m.servers_col_map()} <Icon icon={sortIcon('map')} class="size-2.5" /></span
                  >
                </th>
                <th class="w-16 px-3 py-2 font-medium text-left" title={m.servers_col_time_title()}
                  >{m.servers_col_time()}</th
                >
                <th
                  class="w-14 px-3 py-2 cursor-pointer hover:text-base-content transition-colors"
                  onclick={() => toggleSort('mods')}
                >
                  <span class="flex items-center gap-1"
                    >{m.servers_col_mods()} <Icon icon={sortIcon('mods')} class="size-2.5" /></span
                  >
                </th>
                <th class="w-10 px-2 py-2 text-center">{m.servers_col_os()}</th>
              </tr>
            </thead>
            <tbody>
              {#if offsetY > 0}
                <tr><td colspan="9" class="p-0 border-0" style="height:{offsetY}px"></td></tr>
              {/if}
              {#each visibleServers as server, vi}
                {@const i = startIndex + vi}
                {@const ping = pingCache.get(pingKey(server))}
                {@const isFav = favorites.has(favKey(server))}
                {@const isSel = i === selectedIndex}
                {@const isExcluded = excludedIps.has(server.ip)}
                {@const pk = `${server.ip}:${server.query_port}`}
                {@const livePlayers = a2sPlayers.get(pk) ?? server.players}
                {@const loadingPlayers = a2sPlayersLoading.has(pk)}
                {@const pct = server.max_players > 0 ? Math.round((livePlayers / server.max_players) * 100) : 0}
                <tr
                  class="group/row border-b border-base-300/40 cursor-pointer transition-colors
                       {isSel ? 'bg-primary/10 border-primary/20' : 'hover:bg-base-200/60'}"
                  style="height:{ROW_HEIGHT}px"
                  onclick={() => selectRow(i)}
                  ondblclick={() => onConnect(server)}
                >
                  <!-- # -->
                  <td class="px-2 text-right tabular-nums text-base-content/25 font-mono" style="font-size:10px;"
                    >{i + 1}</td
                  >

                  <!-- Fav star + exclude button -->
                  <td class="px-1 text-center">
                    <div class="flex items-center gap-0.5">
                      <button
                        class="size-5 flex items-center justify-center rounded transition-colors hover:bg-warning/15"
                        onclick={(e) => {
                          e.stopPropagation();
                          isFav ? onRemoveFavorite(server) : onAddFavorite(server);
                        }}
                        title={isFav ? m.servers_remove_favorite() : m.servers_add_favorite()}
                      >
                        <Icon
                          icon={isFav ? 'ph:star-fill' : 'ph:star'}
                          class="size-3 {isFav
                            ? 'text-warning'
                            : 'text-base-content/20 group-hover/row:text-base-content/40'}"
                        />
                      </button>
                      <button
                        class="size-5 flex items-center justify-center rounded transition-colors
                              {isExcluded
                          ? 'opacity-100 hover:bg-error/15'
                          : 'opacity-0 group-hover/row:opacity-100 hover:bg-error/15'}"
                        onclick={(e) => {
                          e.stopPropagation();
                          isExcluded ? onUnexcludeIp(server.ip) : onExcludeIp(server.ip);
                        }}
                        title={isExcluded
                          ? m.servers_ip_excluded_click({ ip: server.ip })
                          : m.servers_exclude_ip({ ip: server.ip })}
                      >
                        <Icon
                          icon={isExcluded ? 'ph:prohibit-fill' : 'ph:prohibit'}
                          class="size-3 {isExcluded ? 'text-error' : 'text-base-content/30'}"
                        />
                      </button>
                    </div>
                  </td>

                  <!-- Ping: dot + ms — click to re-ping -->
                  <td class="px-3">
                    <button
                      class="flex items-center gap-1.5 cursor-pointer hover:opacity-70 transition-opacity {pingFlash.has(
                        pingKey(server),
                      )
                        ? 'ping-flash'
                        : ''}"
                      onclick={(e) => {
                        e.stopPropagation();
                        doPing(server);
                      }}
                      title={m.servers_click_ping()}
                    >
                      <span class="size-1.5 rounded-full shrink-0 {pingDot(ping)}"></span>
                      <span class="tabular-nums font-mono {pingColor(ping)}">
                        {pingLabel(ping)}
                      </span>
                    </button>
                  </td>

                  <!-- Players: fraction + mini bar — click to refresh via A2S -->
                  <td class="px-3">
                    <div class="flex items-center gap-2">
                      <button
                        class="tabular-nums font-mono {playerFill(
                          livePlayers,
                          server.max_players,
                        )} w-14 shrink-0 cursor-pointer hover:opacity-70 transition-opacity text-left"
                        onclick={(e) => {
                          e.stopPropagation();
                          doRefreshPlayers(server);
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
                          class="h-full rounded-full transition-all {playerBarColor(livePlayers, server.max_players)}"
                          style="width:{pct}%"
                        ></div>
                      </div>
                    </div>
                  </td>

                  <!-- Name + IP -->
                  <td class="px-3 max-w-0">
                    <div class="flex items-center gap-1.5 min-w-0">
                      <span class="truncate font-medium text-base-content/90">{server.name}</span>
                      {#if server.password}
                        <span title={m.servers_password_protected()}
                          ><Icon icon="mdi:lock" class="size-3 text-error shrink-0" /></span
                        >
                      {/if}
                      {#if server.first_person_only}
                        <span
                          class="text-warning shrink-0 font-bold"
                          style="font-size:9px;"
                          title={m.servers_first_person()}>1P</span
                        >
                      {/if}
                      {#if server.battl_eye}
                        <span title={m.servers_battleye()}
                          ><img src="/battleeye.png" alt="BE" class="h-3 w-auto shrink-0 rounded-sm" /></span
                        >
                      {/if}
                    </div>
                    <div class="flex items-center gap-2 mt-0.5">
                      <button
                        class="font-mono flex items-center gap-1 group/ip
                             {copiedKey === `${server.ip}:${server.game_port}`
                          ? 'text-success'
                          : 'text-base-content/30 hover:text-base-content/60'}"
                        style="font-size:10px;"
                        onclick={(e) => copyIp(e, server)}
                        title={m.servers_copy_ip({ address: `${server.ip}:${server.game_port}` })}
                      >
                        {server.ip}:{server.game_port}
                        <Icon
                          icon={copiedKey === `${server.ip}:${server.game_port}` ? 'ph:check' : 'ph:copy'}
                          class="size-2 opacity-0 group-hover/ip:opacity-100 transition-opacity {copiedKey ===
                          `${server.ip}:${server.game_port}`
                            ? 'opacity-100'
                            : ''}"
                        />
                      </button>
                      <span class="text-base-content/25" style="font-size:10px;">{server.version}</span>
                    </div>
                  </td>

                  <!-- Map -->
                  <td class="px-3 max-w-0">
                    <span class="truncate text-accent-map block">{server.map}</span>
                  </td>

                  <!-- Time -->
                  <td class="px-3">
                    <span class="flex items-center gap-1 text-base-content/60 tabular-nums font-mono">
                      <Icon icon={timeIcon(server.time)} class="size-3 shrink-0" />
                      {server.time || '—'}
                    </span>
                  </td>

                  <!-- Mods — click to open detail panel scrolled to mods -->
                  <td class="px-3 text-center">
                    {#if server.mods_count > 0}
                      <button
                        class="inline-flex items-center gap-0.5 text-accent-mods hover:opacity-80 transition-colors cursor-pointer"
                        onclick={(e) => {
                          e.stopPropagation();
                          selectedIndex = i;
                          scrollToMods = true;
                          showDetails = true;
                          handleQueryA2s();
                        }}
                        title={m.servers_show_mods()}
                      >
                        <Icon icon="mdi:puzzle-outline" class="size-3 shrink-0" />
                        {server.mods_count}
                      </button>
                    {:else}
                      <span class="text-base-content/20">—</span>
                    {/if}
                  </td>

                  <!-- OS -->
                  <td class="px-2 text-center">
                    {#if server.environment === 'w'}
                      <span title={m.servers_os_windows()}><Icon icon="devicon:windows11" class="size-3.5" /></span>
                    {:else}
                      <span title={m.servers_os_linux()}><Icon icon="flat-color-icons:linux" class="size-3.5" /></span>
                    {/if}
                  </td>
                </tr>
              {/each}
              {#if totalHeight - endIndex * ROW_HEIGHT > 0}
                <tr
                  ><td colspan="9" class="p-0 border-0" style="height:{totalHeight - endIndex * ROW_HEIGHT}px"></td></tr
                >
              {/if}
            </tbody>
          </table>
        {/if}
      </div>

      <!-- Jump-to-top button — appears when scrolled > 300px -->
      {#if scrollTop > 300}
        <button
          class="absolute bottom-4 left-4 z-20 btn btn-sm btn-circle btn-neutral shadow-lg opacity-80 hover:opacity-100 transition-opacity"
          title={m.servers_jump_top()}
          onclick={() => {
            if (scrollContainer) scrollContainer.scrollTo({ top: 0, behavior: 'smooth' });
            scrollTop = 0;
          }}
        >
          <Icon icon="ph:arrow-up" class="size-4" />
        </button>
      {/if}
    </div>
    <!-- end relative wrapper -->

    <!-- Details panel -->
    {#if showDetails && selected}
      <div class="w-80 flex-shrink-0 flex flex-col overflow-hidden">
        <ServerDetailPanel
          server={selected}
          {a2s}
          {a2sLoading}
          {a2sError}
          {installedMods}
          pingMs={pingCache.get(pingKey(selected)) ?? null}
          {bmApiKey}
          {userLocation}
          {scrollToMods}
          onClose={() => {
            showDetails = false;
            scrollToMods = false;
          }}
          onQueryA2s={handleQueryA2s}
        />
      </div>
    {/if}
  </div>

  <!-- Action bar / footer -->
  {#if selected}
    {@const selPing = pingCache.get(pingKey(selected))}
    {@const selPct = selected.max_players > 0 ? Math.round((selected.players / selected.max_players) * 100) : 0}
    <div class="flex items-center gap-0 border-t border-base-300 bg-base-200 flex-shrink-0 min-h-0">
      <!-- Server info block -->
      <div class="flex flex-col justify-center px-3 py-2 flex-1 min-w-0 gap-0.5">
        <!-- Name row -->
        <div class="flex items-center gap-2 min-w-0">
          <span class="font-semibold text-sm text-base-content/90 truncate leading-tight">{selected.name}</span>
          <!-- Flags -->
          <div class="flex items-center gap-1.5 shrink-0">
            {#if selected.password}
              <span title={m.servers_password_protected()}><Icon icon="mdi:lock" class="size-3 text-error" /></span>
            {/if}
            {#if selected.first_person_only}
              <span class="text-warning font-bold leading-none" style="font-size:9px;" title={m.servers_first_person()}
                >1P</span
              >
            {/if}
            {#if selected.battl_eye}
              <span title={m.servers_battleye()}
                ><img src="/battleeye.png" alt="BE" class="h-3 w-auto rounded-sm" /></span
              >
            {/if}
          </div>
        </div>
        <!-- Meta row -->
        <div class="flex items-center gap-3 text-xs text-base-content/40">
          <!-- Ping — click to re-ping -->
          <button
            class="flex items-center gap-1 tabular-nums font-mono cursor-pointer hover:opacity-70 transition-opacity {pingColor(
              selPing,
            )}"
            onclick={() => selected && doPing(selected)}
            title={m.servers_click_ping()}
          >
            <span class="size-1.5 rounded-full shrink-0 {pingDot(selPing)}"></span>
            {pingLabel(selPing)}
          </button>
          <!-- Players -->
          <span class="flex items-center gap-1.5">
            <Icon icon="ph:users" class="size-3 shrink-0" />
            <span class="tabular-nums font-mono {playerFill(selected.players, selected.max_players)}">
              {selected.players}<span class="text-base-content/25">/{selected.max_players}</span>
            </span>
            <div class="w-16 h-1 rounded-full bg-base-300 overflow-hidden">
              <div
                class="h-full rounded-full {playerBarColor(selected.players, selected.max_players)}"
                style="width:{selPct}%"
              ></div>
            </div>
          </span>
          <!-- Map -->
          <span class="flex items-center gap-1">
            <Icon icon="ph:map-trifold" class="size-3 shrink-0" />
            <span class="text-accent-map">{selected.map}</span>
          </span>
          <!-- Mods -->
          {#if selected.mods_count > 0}
            <span class="flex items-center gap-1">
              <Icon icon="mdi:puzzle-outline" class="size-3 shrink-0" />
              <span class="text-accent-mods"
                >{selected.mods_count === 1
                  ? m.servers_mod_one({ count: selected.mods_count })
                  : m.servers_mod_other({ count: selected.mods_count })}</span
              >
            </span>
          {/if}
          <!-- IP (click to copy) -->
          <button
            class="font-mono text-base-content/30 hover:text-base-content/60 transition-colors cursor-pointer"
            title={m.servers_copy_ip_port()}
            onclick={(e) => copyIp(e, selected!)}
          >
            {#if copiedKey === `${selected.ip}:${selected.game_port}`}
              <span class="text-success text-xs">{m.servers_copied()}</span>
            {:else}
              {selected.ip}:{selected.game_port}
            {/if}
          </button>
        </div>
      </div>

      <!-- Divider -->
      <div class="w-px self-stretch bg-base-300 my-2 shrink-0"></div>

      <!-- Actions -->
      <div class="flex items-center gap-1 px-2 shrink-0">
        <button
          class="btn btn-ghost btn-sm gap-1.5"
          class:text-warning={selected && favorites.has(favKey(selected))}
          onclick={() => {
            if (!selected) return;
            favorites.has(favKey(selected)) ? onRemoveFavorite(selected) : onAddFavorite(selected);
          }}
          title={selected && favorites.has(favKey(selected)) ? m.servers_remove_favorite() : m.servers_add_favorite()}
        >
          <Icon icon={selected && favorites.has(favKey(selected)) ? 'ph:star-fill' : 'ph:star'} class="size-3.5" />
          {selected && favorites.has(favKey(selected)) ? m.servers_unfavorite() : m.servers_favorite()}
        </button>
        <button
          class="btn btn-ghost btn-sm gap-1.5"
          onclick={() => {
            if (!selected) return;
            scrollToMods = false;
            showDetails = true;
            handleQueryA2s();
          }}
          title={m.servers_query_a2s()}
        >
          <Icon icon="ph:info" class="size-3.5" />
          {m.servers_info()}
        </button>
        <button
          class="btn btn-primary btn-sm gap-1.5 ml-1"
          title={m.servers_connect_title()}
          onclick={() => selected && onConnect(selected)}
        >
          <Icon icon="ph:play" class="size-3.5" />
          {m.servers_connect()}
        </button>
      </div>
    </div>
  {/if}
</div>

<style></style>
