<script lang="ts">
  import type { HistoryDto, ServerDto } from '$lib/types';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import Icon from '@iconify/svelte';

  interface Props {
    history: HistoryDto[];
    servers: ServerDto[];
    pingCache: Map<string, number>;
    favorites: Set<string>; // "ip:port" keys
    onConnect: (ip: string, port: number, name: string) => void;
    onAddFavorite: (h: HistoryDto) => void;
    onRemoveFavorite: (h: HistoryDto) => void;
    onRemove: (h: HistoryDto) => void;
    onClearAll: () => void;
    onGoToServers?: () => void;
  }

  let { history, servers, pingCache, favorites, onConnect, onAddFavorite, onRemoveFavorite, onRemove, onClearAll, onGoToServers }: Props = $props();

  // Pre-built lookup map rebuilt only when `servers` changes (O(n) once).
  // Covers both query_port and game_port keys so per-row lookups are O(1).
  let serverByKey = $derived((() => {
    const m = new Map<string, ServerDto>();
    for (const s of servers) {
      m.set(`${s.ip}:${s.query_port}`, s);
      m.set(`${s.ip}:${s.game_port}`, s);
    }
    return m;
  })());

  function findServer(h: HistoryDto): ServerDto | null {
    return serverByKey.get(`${h.ip}:${h.port}`) ?? null;
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
    if (sortCol !== col) return 'ph:arrows-down-up';
    return sortAsc ? 'ph:arrow-up' : 'ph:arrow-down';
  }

  let sorted = $derived((() => {
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
          const pa = pingCache.get(`${a.ip}:${a.port}`) ?? Infinity;
          const pb = pingCache.get(`${b.ip}:${b.port}`) ?? Infinity;
          return dir * (pa - pb);
        }
        case 'last':
          return dir * (a.ts - b.ts);
        default: return 0;
      }
    });
    return arr;
  })());

  function pingColor(ms: number | undefined): string {
    if (ms === undefined) return 'text-base-content/30';
    if (ms < 50) return 'text-success';
    if (ms < 100) return 'text-warning';
    return 'text-error';
  }

  /**
   * Compute a human-readable relative time string from a Unix timestamp (seconds).
   * Computed in the frontend so it never goes stale when the app is left open.
   */
  function relativeTime(ts: number): string {
    const secs = Math.floor(Date.now() / 1000) - ts;
    if (secs < 60) return 'just now';
    const mins = Math.floor(secs / 60);
    if (mins < 60) return `${mins} minute${mins === 1 ? '' : 's'} ago`;
    const hours = Math.floor(mins / 60);
    if (hours < 24) return `${hours} hour${hours === 1 ? '' : 's'} ago`;
    const days = Math.floor(hours / 24);
    return `${days} day${days === 1 ? '' : 's'} ago`;
  }

  let copiedKey = $state('');
  async function copyIp(e: MouseEvent, ip: string, port: number) {
    e.stopPropagation();
    const text = `${ip}:${port}`;
    await writeText(text);
    copiedKey = text;
    setTimeout(() => { if (copiedKey === text) copiedKey = ''; }, 1500);
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

  let selectedIdx = $state(-1);

  function handleKeydown(e: KeyboardEvent) {
    const len = sorted.length;
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (len === 0) return;
      selectedIdx = e.key === 'ArrowDown'
        ? Math.min(selectedIdx + 1, len - 1)
        : Math.max(selectedIdx - 1, 0);
    }
    if (e.key === 'Enter' && selectedIdx >= 0 && selectedIdx < len) {
      e.preventDefault();
      const entry = sorted[selectedIdx];
      onConnect(entry.ip, entry.port, entry.name);
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="flex flex-col h-full overflow-hidden" role="region" onkeydown={handleKeydown} tabindex="-1">
  {#if history.length === 0}
    <div class="flex flex-col items-center justify-center h-full gap-3 text-base-content/40">
      <Icon icon="ph:clock-clockwise" class="size-10 opacity-30" />
      <span class="text-sm">No connection history yet</span>
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
                <span class="normal-case font-normal text-base-content/35 ml-0.5">{history.length}</span>
                <Icon icon={sortIcon('name')} class="size-2.5" />
              </span>
            </th>
            <th class="w-32 px-3 py-2 cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('players')}>
              <span class="flex items-center gap-1">Players <Icon icon={sortIcon('players')} class="size-2.5" /></span>
            </th>
            <th class="w-20 px-3 py-2 cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('ping')}>
              <span class="flex items-center gap-1">Ping <Icon icon={sortIcon('ping')} class="size-2.5" /></span>
            </th>
            <th class="w-28 px-3 py-2 font-medium text-left">Map</th>
            <th class="w-16 px-3 py-2 font-medium text-left" title="In-game server time">Time</th>
            <th class="w-24 px-3 py-2 cursor-pointer hover:text-base-content transition-colors text-left" onclick={() => toggleSort('last')}>
              <span class="flex items-center gap-1">Last played <Icon icon={sortIcon('last')} class="size-2.5" /></span>
            </th>
            <th class="w-40 px-3 py-2"></th>
          </tr>
        </thead>
        <tbody>
          {#each sorted as entry, ei}
            {@const server = findServer(entry)}
            {@const ping = pingCache.get(`${entry.ip}:${entry.port}`)}
            {@const pct = server && server.max_players > 0 ? Math.round((server.players / server.max_players) * 100) : 0}
            {@const isFocused = ei === selectedIdx}
            <tr
              class="group/row border-b border-base-300/40 transition-colors cursor-pointer
                     {isFocused ? 'bg-base-200/80 outline outline-1 outline-primary/40' : 'hover:bg-base-200/60'}"
              onclick={() => selectedIdx = ei}
            >
              <!-- Server name + IP -->
              <td class="px-3 py-2 max-w-0">
                <div class="flex items-center gap-1.5 min-w-0">
                  <span class="truncate font-medium text-base-content/90">{entry.name}</span>
                  {#if !server}
                    <span class="shrink-0 text-warning" style="font-size:9px;" title="Server not found in the current server list — it may be offline, or try refreshing the server list">OFFLINE</span>
                  {/if}
                </div>
                <div class="flex items-center gap-2 mt-0.5">
                  <button
                    class="font-mono flex items-center gap-1 group/ip
                           {copiedKey === `${entry.ip}:${entry.port}` ? 'text-success' : 'text-base-content/30 hover:text-base-content/60'}"
                    style="font-size:10px;"
                    onclick={(e) => copyIp(e, entry.ip, entry.port)}
                    title="Copy {entry.ip}:{entry.port} to clipboard"
                  >
                    {entry.ip}:{entry.port}
                    <Icon
                      icon={copiedKey === `${entry.ip}:${entry.port}` ? 'ph:check' : 'ph:copy'}
                      class="size-2 opacity-0 group-hover/ip:opacity-100 transition-opacity {copiedKey === `${entry.ip}:${entry.port}` ? 'opacity-100' : ''}"
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

              <!-- Last played -->
              <td class="px-3 py-2">
                <span
                  class="text-base-content/40 cursor-default"
                  title={new Date(entry.ts * 1000).toLocaleString()}
                >{relativeTime(entry.ts)}</span>
              </td>

              <!-- Actions — always visible -->
              <td class="px-2 py-2">
                <div class="flex gap-1 items-center justify-end">
                 <!-- Favorite toggle -->
                   <button
                     class="size-6 rounded flex items-center justify-center transition-colors
                            {isFav(entry) ? 'text-warning hover:bg-error/10 hover:text-error' : 'text-base-content/35 hover:bg-warning/10 hover:text-warning'}"
                     onclick={() => isFav(entry)
                       ? onRemoveFavorite({ ...entry, port: favPort(entry) })
                       : onAddFavorite(entry)}
                     title={isFav(entry) ? 'Remove from favorites' : 'Add to favorites'}
                   >
                     <Icon icon={isFav(entry) ? 'ph:star-fill' : 'ph:star'} class="size-3.5" />
                   </button>
                  <!-- Remove -->
                  <span title="Remove from history">
                    <button
                      class="size-6 rounded flex items-center justify-center text-base-content/35 hover:bg-error/10 hover:text-error transition-colors"
                      onclick={() => onRemove(entry)}
                    >
                      <Icon icon="ph:trash" class="size-3.5" />
                    </button>
                  </span>
                  <!-- Connect -->
                  <button
                    class="btn btn-primary btn-xs h-6 min-h-0 px-2.5 text-xs font-medium"
                    onclick={() => onConnect(entry.ip, entry.port, entry.name)}
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

    <div class="flex justify-end px-3 py-2 bg-base-200 border-t border-base-300 flex-shrink-0">
      <button class="btn btn-error btn-xs btn-outline" onclick={onClearAll}>
        Clear all history
      </button>
    </div>
  {/if}
</div>
