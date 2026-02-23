<script lang="ts">
  import type { HistoryDto, ServerDto } from '$lib/types';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import Icon from '@iconify/svelte';

  interface Props {
    history: HistoryDto[];
    servers: ServerDto[];
    pingCache: Map<string, number>;
    onConnect: (ip: string, port: number, name: string) => void;
    onAddFavorite: (h: HistoryDto) => void;
    onRemove: (h: HistoryDto) => void;
    onClearAll: () => void;
  }

  let { history, servers, pingCache, onConnect, onAddFavorite, onRemove, onClearAll }: Props = $props();

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

  function findServer(h: HistoryDto): ServerDto | null {
    return (
      servers.find(
        (s) => s.ip === h.ip && (s.query_port === h.port || s.game_port === h.port)
      ) ?? null
    );
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
</script>

<div class="flex flex-col h-full overflow-hidden">
  {#if history.length === 0}
    <div class="flex items-center justify-center h-full text-base-content/40">
      No connection history yet
    </div>
  {:else}
    <div class="overflow-auto flex-1">
      <table class="w-full text-xs" style="table-layout: fixed; border-collapse: collapse;">
        <thead class="sticky top-0 z-10">
          <tr class="bg-base-200/95 backdrop-blur-sm text-base-content/50 uppercase tracking-wider border-b border-base-300 select-none" style="font-size:10px;">
            <th class="px-3 py-2 text-left cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('name')}>
              <span class="flex items-center gap-1">Server <Icon icon={sortIcon('name')} class="size-2.5" /></span>
            </th>
            <th class="w-32 px-3 py-2 cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('players')}>
              <span class="flex items-center gap-1">Players <Icon icon={sortIcon('players')} class="size-2.5" /></span>
            </th>
            <th class="w-20 px-3 py-2 cursor-pointer hover:text-base-content transition-colors" onclick={() => toggleSort('ping')}>
              <span class="flex items-center gap-1">Ping <Icon icon={sortIcon('ping')} class="size-2.5" /></span>
            </th>
            <th class="w-24 px-3 py-2 font-medium text-left">Last played</th>
            <th class="w-40 px-3 py-2"></th>
          </tr>
        </thead>
        <tbody>
          {#each sorted as entry}
            {@const server = findServer(entry)}
            {@const ping = pingCache.get(`${entry.ip}:${entry.port}`)}
            {@const pct = server && server.max_players > 0 ? Math.round((server.players / server.max_players) * 100) : 0}
            <tr class="group/row border-b border-base-300/40 transition-colors hover:bg-base-200/60">
              <!-- Server name + IP -->
              <td class="px-3 py-2 max-w-0">
                <div class="flex items-center gap-1.5 min-w-0">
                  <span class="truncate font-medium text-base-content/90">{entry.name}</span>
                  {#if !server}
                    <span class="shrink-0 text-warning" style="font-size:9px;" title="Not in current server list">OFFLINE</span>
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
                    <span class="text-base-content/25" style="font-size:10px;">{server.map} · {server.version}</span>
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

              <!-- Last played -->
              <td class="px-3 py-2">
                <span class="text-base-content/40">{entry.relative_time}</span>
              </td>

              <!-- Actions — always visible -->
              <td class="px-2 py-2">
                <div class="flex gap-1 items-center justify-end">
                  <!-- Add to favorites -->
                  <span title="Add to favorites">
                    <button
                      class="size-6 rounded flex items-center justify-center text-base-content/35 hover:bg-warning/10 hover:text-warning transition-colors"
                      onclick={() => onAddFavorite(entry)}
                    >
                      <Icon icon="ph:star" class="size-3.5" />
                    </button>
                  </span>
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
