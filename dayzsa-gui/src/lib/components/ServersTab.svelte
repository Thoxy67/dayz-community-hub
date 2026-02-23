<script lang="ts">
  import type { ServerDto, InstalledModDto, A2sDetailsDto } from '$lib/types';
  import ServerDetailPanel from './ServerDetailPanel.svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import Icon from '@iconify/svelte';

  interface Props {
    servers: ServerDto[];
    pingCache: Map<string, number>;
    installedMods: InstalledModDto[];
    favorites: Set<string>; // "ip:port" keys
    loading: boolean;
    onConnect: (server: ServerDto) => void;
    onAddFavorite: (server: ServerDto) => void;
    onRefresh: () => void;
  }

  let {
    servers,
    pingCache,
    installedMods,
    favorites,
    loading,
    onConnect,
    onAddFavorite,
    onRefresh,
  }: Props = $props();

  let searchQuery = $state('');
  let filterMap = $state('');            // '' = all maps
  type ModFilter = 'both' | 'mods-only' | 'no-mods';
  let filterMods = $state<ModFilter>('both');

  function cycleMods() {
    if (filterMods === 'both')      filterMods = 'mods-only';
    else if (filterMods === 'mods-only') filterMods = 'no-mods';
    else                            filterMods = 'both';
  }

  const modsLabel: Record<ModFilter, string> = {
    'both':      'Mods: all',
    'mods-only': 'Mods only',
    'no-mods':   'No mods',
  };
  const modsTitle: Record<ModFilter, string> = {
    'both':      'Click to show only modded servers',
    'mods-only': 'Click to hide modded servers',
    'no-mods':   'Click to show all servers',
  };

  type FPFilter = 'both' | 'fp-only' | 'no-fp';
  let filterFirstPerson = $state<FPFilter>('both');

  function cycleFP() {
    if (filterFirstPerson === 'both')    filterFirstPerson = 'fp-only';
    else if (filterFirstPerson === 'fp-only') filterFirstPerson = 'no-fp';
    else                                 filterFirstPerson = 'both';
  }

  const fpLabel: Record<FPFilter, string> = {
    'both':    '1P: all',
    'fp-only': '1P only',
    'no-fp':   'No 1P',
  };
  const fpTitle: Record<FPFilter, string> = {
    'both':    'Click to show only first-person servers',
    'fp-only': 'Click to hide first-person servers',
    'no-fp':   'Click to show all servers',
  };
  type PwdFilter = 'both' | 'no-pwd' | 'pwd-only';
  let filterPassword = $state<PwdFilter>('both');
  type BEFilter = 'both' | 'be-only' | 'no-be';
  let filterBE = $state<BEFilter>('both');

  function cycleBE() {
    if (filterBE === 'both')    filterBE = 'be-only';
    else if (filterBE === 'be-only') filterBE = 'no-be';
    else                        filterBE = 'both';
  }

  const beLabel: Record<BEFilter, string> = {
    'both':    'BE: all',
    'be-only': 'BE only',
    'no-be':   'No BE',
  };
  const beTitle: Record<BEFilter, string> = {
    'both':    'Click to show only BattlEye servers',
    'be-only': 'Click to hide BattlEye servers',
    'no-be':   'Click to show all servers',
  };

  function cyclePwd() {
    if (filterPassword === 'both')     filterPassword = 'no-pwd';
    else if (filterPassword === 'no-pwd') filterPassword = 'pwd-only';
    else                               filterPassword = 'both';
  }

  const pwdLabel: Record<PwdFilter, string> = {
    'both':     'Pwd: all',
    'no-pwd':   'No pwd',
    'pwd-only': 'Pwd only',
  };
  const pwdTitle: Record<PwdFilter, string> = {
    'both':     'Click to hide password servers',
    'no-pwd':   'Click to show only password servers',
    'pwd-only': 'Click to show all servers',
  };
  let selectedIndex = $state(0);
  let showDetails = $state(false);
  let a2s = $state<A2sDetailsDto | null>(null);
  let a2sLoading = $state(false);

  // ── Sorting ──────────────────────────────────────────────────────────────
  type SortCol = 'ping' | 'players' | 'name' | 'map' | 'mods' | 'none';
  let sortCol = $state<SortCol>('none');
  let sortAsc = $state(true);

  function toggleSort(col: SortCol) {
    if (sortCol === col) {
      sortAsc = !sortAsc;
    } else {
      sortCol = col;
      sortAsc = col === 'name' || col === 'map'; // text cols default asc, numeric cols default desc
    }
    selectedIndex = 0;
    if (scrollContainer) scrollContainer.scrollTop = 0;
    scrollTop = 0;
  }

  function sortIcon(col: SortCol) {
    if (sortCol !== col) return 'ph:arrows-down-up';
    return sortAsc ? 'ph:arrow-up' : 'ph:arrow-down';
  }

  // ── Virtual scrolling state ──────────────────────────────────────────────
  const ROW_HEIGHT = 48;
  const OVERSCAN = 10;
  let scrollContainer: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let containerHeight = $state(600);

  function pingKey(s: ServerDto) { return `${s.ip}:${s.query_port}`; }

  let filtered = $derived((() => {
    let list = servers;
    const q = searchQuery.trim().toLowerCase();
    if (q) {
      list = list.filter(
        (s) =>
          s.name.toLowerCase().includes(q) ||
          s.ip.includes(q) ||
          s.map.toLowerCase().includes(q)
      );
    }
    if (filterFirstPerson === 'fp-only') list = list.filter((s) => s.first_person_only);
    if (filterFirstPerson === 'no-fp')  list = list.filter((s) => !s.first_person_only);
    if (filterPassword === 'no-pwd')    list = list.filter((s) => !s.password);
    if (filterPassword === 'pwd-only')  list = list.filter((s) => s.password);
    if (filterBE === 'be-only')         list = list.filter((s) => !!s.battl_eye);
    if (filterBE === 'no-be')           list = list.filter((s) => !s.battl_eye);
    if (filterMods === 'mods-only')     list = list.filter((s) => s.mods_count > 0);
    if (filterMods === 'no-mods')       list = list.filter((s) => s.mods_count === 0);
    if (filterMap)                      list = list.filter((s) => s.map === filterMap);
    return list;
  })());

  let sorted = $derived((() => {
    if (sortCol === 'none') return filtered;
    const arr = filtered.slice();
    const dir = sortAsc ? 1 : -1;
    arr.sort((a, b) => {
      switch (sortCol) {
        case 'ping': {
          const pa = pingCache.get(pingKey(a)) ?? Infinity;
          const pb = pingCache.get(pingKey(b)) ?? Infinity;
          return dir * (pa - pb);
        }
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
  })());

  let totalHeight = $derived(sorted.length * ROW_HEIGHT);
  let startIndex = $derived(Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN));
  let endIndex = $derived(
    Math.min(sorted.length, Math.ceil((scrollTop + containerHeight) / ROW_HEIGHT) + OVERSCAN)
  );
  let visibleServers = $derived(sorted.slice(startIndex, endIndex));
  let offsetY = $derived(startIndex * ROW_HEIGHT);

  let selected = $derived(sorted[selectedIndex] ?? null);

  function pingColor(ms: number | undefined): string {
    if (ms === undefined) return 'text-base-content/30';
    if (ms < 50) return 'text-success';
    if (ms < 100) return 'text-warning';
    return 'text-error';
  }

  function pingDot(ms: number | undefined): string {
    if (ms === undefined) return 'bg-base-content/20';
    if (ms < 50) return 'bg-success';
    if (ms < 100) return 'bg-warning';
    return 'bg-error';
  }

  function playerFill(players: number, max: number): string {
    if (players === 0) return 'text-base-content/30';
    if (players >= max) return 'text-error';
    if (players > max / 2) return 'text-warning';
    return 'text-success';
  }

  function playerBarColor(players: number, max: number): string {
    if (players === 0) return 'bg-base-content/20';
    if (players >= max) return 'bg-error';
    if (players > max / 2) return 'bg-warning';
    return 'bg-success';
  }

  function favKey(s: ServerDto) { return `${s.ip}:${s.query_port}`; }

  let copiedKey = $state('');
  async function copyIp(e: MouseEvent, server: ServerDto) {
    e.stopPropagation(); // don't select the row
    const text = `${server.ip}:${server.game_port}`;
    await writeText(text);
    copiedKey = text;
    setTimeout(() => { if (copiedKey === text) copiedKey = ''; }, 1500);
  }

  function selectRow(index: number) {
    selectedIndex = index;
    a2s = null;
    a2sError = '';
  }

  let a2sError = $state('');

  async function handleQueryA2s() {
    if (!selected) return;
    const { invoke } = await import('@tauri-apps/api/core');
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
    // Re-run whenever search text or any flag filter changes
    searchQuery; filterFirstPerson; filterPassword; filterBE; filterMods; filterMap;
    selectedIndex = 0;
    a2s = null;
    if (scrollContainer) scrollContainer.scrollTop = 0;
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
</script>

<div class="flex flex-col h-full" onkeydown={handleKeydown} role="grid" tabindex="-1">
  <!-- Toolbar -->
  <div class="flex items-center gap-3 px-3 py-2 bg-base-200 border-b border-base-300 flex-shrink-0">

    <!-- Search input -->
    <label class="input input-sm input-bordered flex items-center gap-2 w-56 shrink-0">
      <Icon icon="ph:magnifying-glass" class="size-3.5 text-base-content/40 shrink-0" />
      <input
        type="text"
        placeholder="Search name, IP, map…"
        bind:value={searchQuery}
        class="grow bg-transparent outline-none text-sm min-w-0"
      />
      {#if searchQuery}
        <button class="btn btn-ghost btn-xs p-0 min-h-0 h-auto shrink-0" onclick={() => (searchQuery = '')}>
          <Icon icon="ph:x" class="size-3" />
        </button>
      {/if}
    </label>

    <!-- Divider -->
    <div class="w-px h-5 bg-base-300 shrink-0"></div>

    <!-- Flag filters — grouped pill bar -->
    <div class="flex items-center rounded-lg border border-base-300 bg-base-100/50 overflow-hidden divide-x divide-base-300 shrink-0 h-7">
      <!-- 1P -->
      <button
        class="flex items-center gap-1 px-2.5 h-full text-xs font-semibold transition-colors"
        class:bg-warning={filterFirstPerson === 'fp-only'}
        class:text-warning-content={filterFirstPerson === 'fp-only'}
        class:bg-error={filterFirstPerson === 'no-fp'}
        class:text-error-content={filterFirstPerson === 'no-fp'}
        class:opacity-50={filterFirstPerson === 'both'}
        class:hover:opacity-100={filterFirstPerson === 'both'}
        onclick={cycleFP}
        title={fpTitle[filterFirstPerson]}
      >
        {fpLabel[filterFirstPerson]}
      </button>
      <!-- Password -->
      <button
        class="flex items-center gap-1.5 px-2.5 h-full text-xs font-medium transition-colors"
        class:bg-error={filterPassword === 'pwd-only'}
        class:text-error-content={filterPassword === 'pwd-only'}
        class:bg-warning={filterPassword === 'no-pwd'}
        class:text-warning-content={filterPassword === 'no-pwd'}
        class:text-base-content={filterPassword === 'both'}
        class:opacity-50={filterPassword === 'both'}
        class:hover:opacity-100={filterPassword === 'both'}
        onclick={cyclePwd}
        title={pwdTitle[filterPassword]}
      >
        <Icon icon="mdi:lock" class="size-3 shrink-0" />
        {pwdLabel[filterPassword]}
      </button>
      <!-- BattlEye -->
      <button
        class="flex items-center gap-1.5 px-2.5 h-full text-xs font-medium transition-colors"
        class:bg-primary={filterBE === 'be-only'}
        class:text-primary-content={filterBE === 'be-only'}
        class:bg-error={filterBE === 'no-be'}
        class:text-error-content={filterBE === 'no-be'}
        class:opacity-50={filterBE === 'both'}
        class:hover:opacity-100={filterBE === 'both'}
        onclick={cycleBE}
        title={beTitle[filterBE]}
      >
        <img src="/battleeye.png" alt="BE" class="h-3.5 w-auto rounded-sm shrink-0" />
        {beLabel[filterBE]}
      </button>
      <!-- Mods -->
      <button
        class="flex items-center gap-1.5 px-2.5 h-full text-xs font-medium transition-colors"
        class:bg-violet-500={filterMods === 'mods-only'}
        class:text-white={filterMods === 'mods-only'}
        class:bg-error={filterMods === 'no-mods'}
        class:text-error-content={filterMods === 'no-mods'}
        class:opacity-50={filterMods === 'both'}
        class:hover:opacity-100={filterMods === 'both'}
        onclick={cycleMods}
        title={modsTitle[filterMods]}
      >
        <Icon icon="mdi:puzzle-outline" class="size-3 shrink-0" />
        {modsLabel[filterMods]}
      </button>
    </div>

    <!-- Map dropdown -->
    <div class="relative shrink-0">
      <select
        class="select select-sm select-bordered h-7 min-h-0 py-0 pr-7 pl-2.5 text-xs rounded-lg appearance-none"
        class:text-base-content={!filterMap}
        class:opacity-50={!filterMap}
        class:text-sky-400={!!filterMap}
        class:opacity-100={!!filterMap}
        bind:value={filterMap}
      >
        <option value="">All maps</option>
        {#each [...new Set(servers.map((s) => s.map))].sort() as map}
          <option value={map}>{map}</option>
        {/each}
      </select>
      {#if filterMap}
        <button
          class="absolute right-6 top-1/2 -translate-y-1/2 text-base-content/40 hover:text-base-content transition-colors"
          onclick={() => (filterMap = '')}
          title="Clear map filter"
        >
          <Icon icon="ph:x" class="size-2.5" />
        </button>
      {/if}
    </div>

    <!-- Server count -->
    <span class="text-xs tabular-nums text-base-content/40 shrink-0">
      {filtered.length}<span class="text-base-content/25">/{servers.length}</span>
    </span>

    <!-- Spacer -->
    <div class="flex-1"></div>

    <!-- Divider -->
    <div class="w-px h-5 bg-base-300 shrink-0"></div>

    <!-- Utility buttons -->
    <div class="flex items-center gap-1">
      <button
        class="btn btn-ghost btn-xs gap-1.5"
        class:btn-active={showDetails}
        onclick={() => (showDetails = !showDetails)}
        title="Toggle details panel"
      >
        <Icon icon="ph:sidebar-simple" class="size-3.5" />
        Details
      </button>
      <button
        class="btn btn-ghost btn-xs gap-1.5"
        onclick={onRefresh}
        disabled={loading}
        title="Refresh server list"
      >
        {#if loading}
          <span class="loading loading-spinner loading-xs"></span>
        {:else}
          <Icon icon="ph:arrows-clockwise" class="size-3.5" />
        {/if}
        Refresh
      </button>
    </div>
  </div>

  <!-- Main content: table + optional details panel -->
  <div class="flex flex-1 overflow-hidden">
    <!-- Server table with virtual scrolling -->
    <div
      class="flex-1 overflow-auto"
      bind:this={scrollContainer}
      onscroll={handleScroll}
    >
      {#if loading && servers.length === 0}
        <div class="flex items-center justify-center h-full gap-2 text-base-content/50">
          <span class="loading loading-spinner loading-md"></span>
          <span>Loading servers…</span>
        </div>
      {:else if sorted.length === 0}
        <div class="flex items-center justify-center h-full text-base-content/40">
          No servers match your search
        </div>
      {:else}
        <table class="w-full text-xs" style="table-layout: fixed; border-collapse: collapse;">
          <thead class="sticky top-0 z-10">
            <tr class="bg-base-200/95 backdrop-blur-sm text-base-content/50 uppercase tracking-wider border-b border-base-300 select-none" style="font-size:10px;">
              <th class="w-8 px-2 py-2 text-right font-medium">#</th>
              <th class="w-6 px-1 py-2"></th>
              <th class="w-20 px-3 py-2 cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('ping')}>
                <span class="flex items-center gap-1">Ping <Icon icon={sortIcon('ping')} class="size-2.5" /></span>
              </th>
              <th class="w-32 px-3 py-2 cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('players')}>
                <span class="flex items-center gap-1">Players <Icon icon={sortIcon('players')} class="size-2.5" /></span>
              </th>
              <th class="px-3 py-2 cursor-pointer hover:text-base-content transition-colors text-left" onclick={() => toggleSort('name')}>
                <span class="flex items-center gap-1">Server <Icon icon={sortIcon('name')} class="size-2.5" /></span>
              </th>
              <th class="w-28 px-3 py-2 cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('map')}>
                <span class="flex items-center gap-1">Map <Icon icon={sortIcon('map')} class="size-2.5" /></span>
              </th>
              <th class="w-14 px-3 py-2 cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('mods')}>
                <span class="flex items-center gap-1">Mods <Icon icon={sortIcon('mods')} class="size-2.5" /></span>
              </th>
              <th class="w-10 px-2 py-2 text-center">OS</th>
            </tr>
          </thead>
          <tbody>
            {#if offsetY > 0}
              <tr><td colspan="8" class="p-0 border-0" style="height:{offsetY}px"></td></tr>
            {/if}
            {#each visibleServers as server, vi}
              {@const i = startIndex + vi}
              {@const ping = pingCache.get(pingKey(server))}
              {@const isFav = favorites.has(favKey(server))}
              {@const isSel = i === selectedIndex}
              {@const pct = server.max_players > 0 ? Math.round((server.players / server.max_players) * 100) : 0}
              <tr
                class="group/row border-b border-base-300/40 cursor-pointer transition-colors
                       {isSel ? 'bg-primary/10 border-primary/20' : 'hover:bg-base-200/60'}"
                style="height:{ROW_HEIGHT}px"
                onclick={() => selectRow(i)}
                ondblclick={() => onConnect(server)}
              >
                <!-- # -->
                <td class="px-2 text-right tabular-nums text-base-content/25 font-mono" style="font-size:10px;">{i + 1}</td>

                <!-- Fav star -->
                <td class="px-1 text-center">
                  {#if isFav}
                    <Icon icon="ph:star-fill" class="size-3 text-warning" />
                  {/if}
                </td>

                <!-- Ping: dot + ms -->
                <td class="px-3">
                  <div class="flex items-center gap-1.5">
                    <span class="size-1.5 rounded-full shrink-0 {pingDot(ping)}"></span>
                    <span class="tabular-nums font-mono {pingColor(ping)}">
                      {ping !== undefined ? `${ping}ms` : '—'}
                    </span>
                  </div>
                </td>

                <!-- Players: fraction + mini bar -->
                <td class="px-3">
                  <div class="flex items-center gap-2">
                    <span class="tabular-nums font-mono {playerFill(server.players, server.max_players)} w-14 shrink-0">
                      {server.players}<span class="text-base-content/30">/{server.max_players}</span>
                    </span>
                    <div class="flex-1 h-1 rounded-full bg-base-300 overflow-hidden">
                      <div
                        class="h-full rounded-full transition-all {playerBarColor(server.players, server.max_players)}"
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
                      <span title="Password protected"><Icon icon="mdi:lock" class="size-3 text-error shrink-0" /></span>
                    {/if}
                    {#if server.first_person_only}
                      <span class="text-warning shrink-0 font-bold" style="font-size:9px;" title="First person only">1P</span>
                    {/if}
                    {#if server.battl_eye}
                      <span title="BattlEye"><img src="/battleeye.png" alt="BE" class="h-3 w-auto shrink-0 rounded-sm" /></span>
                    {/if}
                  </div>
                  <div class="flex items-center gap-2 mt-0.5">
                    <button
                      class="font-mono flex items-center gap-1 group/ip
                             {copiedKey === `${server.ip}:${server.game_port}` ? 'text-success' : 'text-base-content/30 hover:text-base-content/60'}"
                      style="font-size:10px;"
                      onclick={(e) => copyIp(e, server)}
                      title="Copy {server.ip}:{server.game_port} to clipboard"
                    >
                      {server.ip}:{server.game_port}
                      <Icon
                        icon={copiedKey === `${server.ip}:${server.game_port}` ? 'ph:check' : 'ph:copy'}
                        class="size-2 opacity-0 group-hover/ip:opacity-100 transition-opacity {copiedKey === `${server.ip}:${server.game_port}` ? 'opacity-100' : ''}"
                      />
                    </button>
                    <span class="text-base-content/25" style="font-size:10px;">{server.version}</span>
                  </div>
                </td>

                <!-- Map -->
                <td class="px-3 max-w-0">
                  <span class="truncate text-sky-500/80 block">{server.map}</span>
                </td>

                <!-- Mods -->
                <td class="px-3 text-center">
                  {#if server.mods_count > 0}
                    <span class="inline-flex items-center gap-0.5 text-violet-400/80">
                      <Icon icon="mdi:puzzle-outline" class="size-3 shrink-0" />
                      {server.mods_count}
                    </span>
                  {:else}
                    <span class="text-base-content/20">—</span>
                  {/if}
                </td>

                <!-- OS -->
                <td class="px-2 text-center">
                  {#if server.environment === 'w'}
                    <span title="Windows"><Icon icon="gg:windows" class="size-3.5 text-sky-400/70" /></span>
                  {:else}
                    <span title="Linux"><Icon icon="simple-icons:linux" class="size-3.5 text-emerald-400/70" /></span>
                  {/if}
                </td>
              </tr>
            {/each}
            {#if totalHeight - (endIndex * ROW_HEIGHT) > 0}
              <tr><td colspan="8" class="p-0 border-0" style="height:{totalHeight - (endIndex * ROW_HEIGHT)}px"></td></tr>
            {/if}
          </tbody>
        </table>
      {/if}
    </div>

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
          onClose={() => (showDetails = false)}
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
              <span title="Password protected"><Icon icon="mdi:lock" class="size-3 text-error" /></span>
            {/if}
            {#if selected.first_person_only}
              <span class="text-warning font-bold leading-none" style="font-size:9px;" title="First person only">1P</span>
            {/if}
            {#if selected.battl_eye}
              <span title="BattlEye"><img src="/battleeye.png" alt="BE" class="h-3 w-auto rounded-sm" /></span>
            {/if}
          </div>
        </div>
        <!-- Meta row -->
        <div class="flex items-center gap-3 text-xs text-base-content/40">
          <!-- Ping -->
          <span class="flex items-center gap-1 tabular-nums font-mono {pingColor(selPing)}">
            <span class="size-1.5 rounded-full shrink-0 {pingDot(selPing)}"></span>
            {selPing !== undefined ? `${selPing}ms` : '—'}
          </span>
          <!-- Players -->
          <span class="flex items-center gap-1.5">
            <Icon icon="ph:users" class="size-3 shrink-0" />
            <span class="tabular-nums font-mono {playerFill(selected.players, selected.max_players)}">
              {selected.players}<span class="text-base-content/25">/{selected.max_players}</span>
            </span>
            <div class="w-16 h-1 rounded-full bg-base-300 overflow-hidden">
              <div class="h-full rounded-full {playerBarColor(selected.players, selected.max_players)}" style="width:{selPct}%"></div>
            </div>
          </span>
          <!-- Map -->
          <span class="flex items-center gap-1">
            <Icon icon="ph:map-trifold" class="size-3 shrink-0" />
            <span class="text-sky-500/70">{selected.map}</span>
          </span>
          <!-- Mods -->
          {#if selected.mods_count > 0}
            <span class="flex items-center gap-1">
              <Icon icon="mdi:puzzle-outline" class="size-3 shrink-0" />
              <span class="text-violet-400/70">{selected.mods_count} mod{selected.mods_count !== 1 ? 's' : ''}</span>
            </span>
          {/if}
          <!-- IP -->
          <span class="font-mono text-base-content/30">{selected.ip}:{selected.game_port}</span>
        </div>
      </div>

      <!-- Divider -->
      <div class="w-px self-stretch bg-base-300 my-2 shrink-0"></div>

      <!-- Actions -->
      <div class="flex items-center gap-1 px-2 shrink-0">
        <button
          class="btn btn-ghost btn-sm gap-1.5"
          onclick={() => selected && onAddFavorite(selected)}
          title="Add to favorites"
        >
          <Icon icon="ph:star" class="size-3.5" />
          Favorite
        </button>
        <button
          class="btn btn-ghost btn-sm gap-1.5"
          onclick={() => selected && (showDetails = true, handleQueryA2s())}
          title="Query live A2S info"
        >
          <Icon icon="ph:info" class="size-3.5" />
          Info
        </button>
        <button
          class="btn btn-primary btn-sm gap-1.5 ml-1"
          onclick={() => selected && onConnect(selected)}
        >
          <Icon icon="ph:play" class="size-3.5" />
          Connect
        </button>
      </div>
    </div>
  {/if}
</div>
