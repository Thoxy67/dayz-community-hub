<script lang="ts">
  import type { ServerDto, ServerFullDto, ModDto, A2sDetailsDto, InstalledModDto, BattleMetricsDto } from '$lib/types';
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import Icon from '@iconify/svelte';

  let copiedIp = $state(false);
  async function copyIp() {
    await writeText(`${server.ip}:${server.game_port}`);
    copiedIp = true;
    setTimeout(() => { copiedIp = false; }, 1500);
  }

  interface Props {
    server: ServerDto;
    a2s: A2sDetailsDto | null;
    a2sLoading: boolean;
    a2sError: string;
    installedMods: InstalledModDto[];
    pingMs: number | null;
    /** BattleMetrics personal access token from profile (null = not configured). */
    bmApiKey: string | null;
    onClose: () => void;
    onQueryA2s: () => void;
  }

  let { server, a2s, a2sLoading, a2sError, installedMods, pingMs, bmApiKey, onClose, onQueryA2s }: Props = $props();

  let installedIds = $derived(new Set(installedMods.map((m) => m.id)));

  // On-demand mod fetching
  let mods = $state<ModDto[]>([]);
  let modsLoading = $state(false);
  let modsFetchFailed = $state(false);
  // Plain (non-reactive) guard — must NOT be $state or it would become an
  // effect dependency and cause self-cancelling re-runs.
  let fetchedKey = '';
  let _detailDebounce: ReturnType<typeof setTimeout> | undefined;
  // Reactive counter used only to force a re-fetch on manual retry.
  let retryTick = $state(0);

  // Prefer mods from get_server_details; fall back to a2s.mods if the list returned nothing.
  let displayMods = $derived(mods.length > 0 ? mods : (a2s?.mods ?? []));

  // Fetch full mod list when the selected server changes.
  // A 200ms debounce prevents firing an IPC call for every row the user
  // passes through while scrolling/navigating the server list quickly.
  $effect(() => {
    const key = `${server.ip}:${server.query_port}`;
    const count = server.mods_count;

    // Read retryTick so a manual retry invalidates this effect.
    retryTick;
    // Already fetched for this server — nothing to do.
    if (key === fetchedKey) return;

    // Cancel any in-flight debounce for a previous server.
    clearTimeout(_detailDebounce);

    if (count === 0) {
      mods = [];
      modsFetchFailed = false;
      fetchedKey = key;
      return;
    }

    _detailDebounce = setTimeout(() => {
      // Guard: server may have changed during the debounce window.
      if (`${server.ip}:${server.query_port}` !== key) return;
      modsLoading = true;
      invoke<ServerFullDto>('get_server_details', { ip: server.ip, port: server.query_port })
        .then((full) => {
          if (`${server.ip}:${server.query_port}` === key) {
            mods = full.mods;
            modsFetchFailed = false;
            fetchedKey = key;
          }
        })
        .catch(() => {
          if (`${server.ip}:${server.query_port}` === key) {
            mods = [];
            modsFetchFailed = true;
            fetchedKey = key;
          }
        })
        .finally(() => { modsLoading = false; });
    }, 200);

    return () => clearTimeout(_detailDebounce);
  });

  // ── BattleMetrics ──────────────────────────────────────────────────────────
  let bm = $state<BattleMetricsDto | null>(null);
  let bmLoading = $state(false);
  let bmError = $state('');
  // Plain (non-reactive) guard — same pattern as fetchedKey above.
  let bmFetchedKey = '';
  let bmRetryTick = $state(0);

  $effect(() => {
    const key = `${server.ip}:${server.query_port}`;
    // Depend on bmRetryTick for manual retries; bmApiKey for token changes.
    bmRetryTick;
    const token = bmApiKey;

    if (!token || key === bmFetchedKey) return;

    bmLoading = true;
    bmError = '';
    invoke<BattleMetricsDto>('fetch_battlemetrics_server', { ip: server.ip, port: server.query_port })
      .then((result) => {
        if (`${server.ip}:${server.query_port}` === key) {
          bm = result;
          bmError = '';
          bmFetchedKey = key;
        }
      })
      .catch((e: unknown) => {
        if (`${server.ip}:${server.query_port}` === key) {
          bm = null;
          bmError = String(e);
          bmFetchedKey = key;
        }
      })
      .finally(() => { bmLoading = false; });
  });

  /** Build a tiny SVG sparkline from player history data. */
  function sparklinePath(history: [number, number][], w = 120, h = 28): string {
    if (history.length < 2) return '';
    // Sort ascending by timestamp
    const pts = [...history].sort((a, b) => a[0] - b[0]);
    const maxVal = Math.max(...pts.map((p) => p[1]), 1);
    const step = w / (pts.length - 1);
    return pts
      .map((p, i) => {
        const x = i * step;
        const y = h - (p[1] / maxVal) * h;
        return `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(' ');
  }

  function formatDuration(secs: number): string {
    const s = Math.floor(secs);
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m`;
    return `<1m`;
  }

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
      <button
        class="font-mono text-base-content hover:text-primary transition-colors flex items-center gap-1 group/ip text-left"
        title="Copy IP:port"
        onclick={copyIp}
      >
        {#if copiedIp}
          <span class="text-success">Copied!</span>
        {:else}
          {server.ip}:{server.game_port}
          <Icon icon="ph:copy" class="size-3 opacity-0 group-hover/ip:opacity-100 transition-opacity" />
        {/if}
      </button>

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
    {:else if displayMods.length > 0}
      <div>
        <div class="text-xs font-semibold text-base-content/50 mb-1">
          Mods ({displayMods.length})
        </div>
        <div class="space-y-0.5 max-h-40 overflow-y-auto">
          {#each displayMods as mod}
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
      <div class="flex items-center gap-2 text-xs text-base-content/40">
        {#if modsFetchFailed}
          <Icon icon="ph:warning-circle" class="size-3.5 text-warning/60 shrink-0" />
          <span class="italic">{server.mods_count} mod(s) — fetch failed</span>
          <button
            class="btn btn-ghost btn-xs h-5 min-h-0 px-1.5 ml-auto"
            onclick={() => { fetchedKey = ''; }}
            title="Retry"
          >
            <Icon icon="ph:arrows-clockwise" class="size-3" />
          </button>
        {:else}
          <span class="italic">{server.mods_count} mod(s) — not reported by server</span>
          {#if !a2s}
            <button
              class="btn btn-ghost btn-xs h-5 min-h-0 px-1.5 ml-auto gap-1"
              onclick={onQueryA2s}
              title="Query live server for mod list"
            >
              <Icon icon="ph:broadcast" class="size-3" />
              Query A2S
            </button>
          {/if}
        {/if}
      </div>
    {:else}
      <div class="text-xs text-base-content/40 italic">No mods</div>
    {/if}

    <!-- BattleMetrics -->
    <div>
      <div class="flex items-center justify-between mb-1">
        <span class="text-xs font-semibold text-base-content/50 flex items-center gap-1.5">
          <Icon icon="ph:chart-line-up" class="size-3.5" />
          BattleMetrics
        </span>
        {#if bmApiKey}
          <button
            class="btn btn-ghost btn-xs"
            onclick={() => { bmFetchedKey = ''; bmRetryTick++; }}
            disabled={bmLoading}
            title="Refresh BattleMetrics data"
          >
            {#if bmLoading}
              <span class="loading loading-spinner loading-xs"></span>
            {:else}
              Refresh
            {/if}
          </button>
        {/if}
      </div>

      {#if !bmApiKey}
        <p class="text-xs text-base-content/35 italic">
          Configure a BattleMetrics API token in settings to see server rankings &amp; history.
        </p>
      {:else if bmLoading}
        <div class="flex items-center gap-2 text-xs text-base-content/50">
          <span class="loading loading-spinner loading-xs"></span>
          Loading BattleMetrics data…
        </div>
      {:else if bm}
        <!-- Rank + status row -->
        <div class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs mb-2">
          {#if bm.rank !== null}
            <div class="text-base-content/50">Rank</div>
            <div class="font-mono font-bold text-primary">#{bm.rank}</div>
          {/if}
          <div class="text-base-content/50">Status</div>
          <div class="flex items-center gap-1.5">
            <span class="size-2 rounded-full flex-shrink-0 {bm.status === 'online' ? 'bg-success' : bm.status === 'offline' ? 'bg-error' : 'bg-base-content/30'}"></span>
            <span class="{bm.status === 'online' ? 'text-success' : bm.status === 'offline' ? 'text-error' : 'text-base-content/50'}">{bm.status}</span>
          </div>
          {#if bm.country}
            <div class="text-base-content/50">Country</div>
            <div class="font-mono">{bm.country}</div>
          {/if}
          {#if bm.uptime !== null}
            <div class="text-base-content/50">Uptime</div>
            <div class="{(bm.uptime ?? 0) >= 90 ? 'text-success' : (bm.uptime ?? 0) >= 70 ? 'text-warning' : 'text-error'}">{bm.uptime?.toFixed(1)}%</div>
          {/if}
        </div>

        <!-- Player history sparkline -->
        {#if bm.player_history.length >= 2}
          <div class="mb-2">
            <div class="text-xs text-base-content/40 mb-1">Player count (24 h)</div>
            <svg
              viewBox="0 0 120 28"
              class="w-full h-7 text-primary"
              preserveAspectRatio="none"
            >
              <path
                d={sparklinePath(bm.player_history)}
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </div>
        {/if}

        <!-- Link to BattleMetrics -->
        <button
          class="btn btn-ghost btn-xs gap-1 text-base-content/50 hover:text-primary"
          onclick={() => openUrl(`https://www.battlemetrics.com/servers/dayz/${bm?.id}`)}
        >
          <Icon icon="ph:arrow-square-out" class="size-3.5" />
          View on BattleMetrics
        </button>
      {:else if bmError}
        <div class="flex items-start gap-2 px-2.5 py-2 rounded-lg bg-error/10 border border-error/25 text-xs text-error">
          <Icon icon="ph:warning-circle" class="size-3.5 shrink-0 mt-0.5" />
          <span class="leading-snug break-all flex-1">{bmError}</span>
          <button
            class="btn btn-ghost btn-xs h-5 min-h-0 px-1.5 shrink-0"
            onclick={() => { bmFetchedKey = ''; bmRetryTick++; }}
            title="Retry"
          >
            <Icon icon="ph:arrows-clockwise" class="size-3" />
          </button>
        </div>
      {/if}
    </div>

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
                <span class="text-base-content/40 ml-auto tabular-nums">{formatDuration(player.duration)}</span>
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
