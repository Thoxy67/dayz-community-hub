<script lang="ts">
  import type { A2sDetailsDto, ServerDto, ServerFullDto, InstalledModDto } from '$lib/types';
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '@iconify/svelte';

  interface Props {
    servers: ServerDto[];
    installedMods: InstalledModDto[];
    /** Set of "ip:port" strings for quick favorite lookup */
    favorites?: Set<string>;
    onConnect: (ip: string, port: number, password?: string) => void;
    onAddFavorite?: (name: string, ip: string, port: number) => void;
  }

  let { servers, installedMods, favorites = new Set(), onConnect, onAddFavorite }: Props = $props();

  let address = $state('');
  let port = $state('2302');
  let password = $state('');
  let showPassword = $state(false);

  // Query state
  let a2s = $state<A2sDetailsDto | null>(null);
  let a2sLoading = $state(false);
  let a2sError = $state('');

  // Resolved port info for display
  type PortKind = 'query' | 'game' | 'unknown';
  let resolvedQueryPort = $state<number | null>(null);
  let resolvedPortKind = $state<PortKind>('unknown');

  // Full server details (from list)
  let fullServer = $state<ServerFullDto | null>(null);
  let fullLoading = $state(false);

  // Installed mod ids for badge
  let installedIds = $derived(new Set(installedMods.map((m) => m.id)));

  // ── Helpers ────────────────────────────────────────────────────────────────

  /** Server found in the cached list by IP + port. */
  let foundServer = $derived((() => {
    const p = parseInt(port, 10);
    if (!address || isNaN(p)) return null;
    return (
      servers.find(
        (s) =>
          s.ip === address.trim() &&
          (s.query_port === p || s.game_port === p)
      ) ?? null
    );
  })());

  /** True if the current ip:port (or any related port) is already a favorite. */
  let isFavorite = $derived((() => {
    const ip = address.trim();
    const p = parseInt(port, 10);
    if (!ip || isNaN(p)) return false;
    // Check the typed port directly
    if (favorites.has(`${ip}:${p}`)) return true;
    // Also check game_port / query_port from the matched server
    if (!foundServer) return false;
    return favorites.has(`${ip}:${foundServer.game_port}`) || favorites.has(`${ip}:${foundServer.query_port}`);
  })());

  function parseAddress() {
    const raw = address.trim();
    const colon = raw.lastIndexOf(':');
    if (colon !== -1) {
      const after = raw.slice(colon + 1);
      if (/^\d+$/.test(after)) {
        address = raw.slice(0, colon);
        port = after;
      }
    }
  }

  function playerBarColor(players: number, max: number): string {
    if (players >= max) return 'bg-error';
    if (players > max / 2) return 'bg-warning';
    return 'bg-success';
  }

  function playerTextColor(players: number, max: number): string {
    if (players >= max) return 'text-error';
    if (players > max / 2) return 'text-warning';
    return 'text-success';
  }

  function formatDuration(secs: number): string {
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    if (h > 0) return `${h}h ${m}m`;
    return `${m}m`;
  }

  // ── Query ──────────────────────────────────────────────────────────────────

  /**
   * Try to query A2S at a specific port. Returns the result or throws.
   */
  async function tryQuery(ip: string, qport: number): Promise<A2sDetailsDto> {
    return invoke<A2sDetailsDto>('query_a2s', { ip, port: qport });
  }

  async function queryInfo() {
    parseAddress();
    const p = parseInt(port, 10);
    if (!address || isNaN(p)) return;

    const ip = address.trim();
    a2sLoading = true;
    a2sError = '';
    a2s = null;
    fullServer = null;
    resolvedQueryPort = null;
    resolvedPortKind = 'unknown';

    try {
      // 1. Server found in list — use authoritative query port, no guessing needed.
      const fs = foundServer;
      if (fs) {
        a2s = await tryQuery(ip, fs.query_port);
        resolvedQueryPort = fs.query_port;
        resolvedPortKind = p === fs.query_port ? 'query' : 'game';
      } else {
        // 2. Not in list — use the typed port directly as the query port.
        //    The backend will also attempt to match it against both game_port
        //    and query_port in the server list as a last resort.
        a2s = await tryQuery(ip, p);
        resolvedQueryPort = a2s.query_port;
        resolvedPortKind = p === a2s.query_port ? 'query' : 'unknown';
      }

      // Load full server details (mods) from the list using the resolved query port.
      fullLoading = true;
      try {
        fullServer = await invoke<ServerFullDto>('get_server_details', {
          ip,
          port: a2s.query_port,
        });
      } catch {
        fullServer = null;
      } finally {
        fullLoading = false;
      }
    } catch (e) {
      a2sError = String(e);
    } finally {
      a2sLoading = false;
    }
  }

  function connect() {
    parseAddress();
    const p = parseInt(port, 10);
    if (!address || isNaN(p)) return;
    onConnect(address.trim(), p, password || undefined);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') connect();
  }

  // Server name to show: prefer a2s (live) > fullServer > foundServer
  let displayName = $derived(
    a2s ? a2s.server_name : (foundServer?.name ?? '')
  );

  // Mods: prefer fullServer (has IDs) > a2s.mods
  let displayMods = $derived(
    (fullServer && fullServer.mods.length > 0)
      ? fullServer.mods
      : (a2s && a2s.mods.length > 0 ? a2s.mods : [])
  );

  let showCard = $derived(!!(a2s || foundServer));
</script>

<div class="flex flex-col h-full overflow-auto">
  <div class="max-w-xl mx-auto w-full p-6 space-y-5">

    <!-- Header -->
    <div class="flex items-center gap-2">
      <Icon icon="ph:plugs-connected" class="size-5 text-primary" />
      <h2 class="text-base font-semibold">Direct Connect</h2>
    </div>

    <!-- Form card -->
    <div class="rounded-xl border border-base-300 bg-base-100 overflow-hidden">
      <div class="px-4 py-3 bg-base-200 border-b border-base-300 flex items-center gap-2">
        <Icon icon="ph:network" class="size-4 text-base-content/50" />
        <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wide">Connection</span>
      </div>

      <div class="p-4 space-y-4">
        <!-- Address + Port -->
        <div class="flex gap-3">
          <div class="form-control flex-1">
            <label class="label py-0 pb-1.5" for="dc-address">
              <span class="label-text text-xs text-base-content/60 flex items-center gap-1.5">
                <Icon icon="ph:globe" class="size-3.5" />
                IP / Hostname
              </span>
            </label>
            <input
              id="dc-address"
              type="text"
              class="input input-bordered input-sm font-mono"
              placeholder="e.g. 192.168.1.1"
              bind:value={address}
              onblur={parseAddress}
              onkeydown={handleKeydown}
            />
          </div>
          <div class="form-control w-28">
            <label class="label py-0 pb-1.5" for="dc-port">
              <span class="label-text text-xs text-base-content/60 flex items-center gap-1.5">
                <Icon icon="ph:plugs" class="size-3.5" />
                {#if foundServer && parseInt(port, 10) === foundServer?.game_port}
                  Port <span class="text-amber-400 ml-1">(game)</span>
                {:else if foundServer && parseInt(port, 10) === foundServer?.query_port}
                  Port <span class="text-sky-400 ml-1">(query)</span>
                {:else}
                  Port
                {/if}
              </span>
            </label>
            <input
              id="dc-port"
              type="number"
              class="input input-bordered input-sm font-mono"
              placeholder="2302"
              bind:value={port}
              onkeydown={handleKeydown}
            />
          </div>
        </div>

        <!-- Password -->
        <div class="form-control">
          <label class="label py-0 pb-1.5" for="dc-password">
            <span class="label-text text-xs text-base-content/60 flex items-center gap-1.5">
              <Icon icon="ph:lock-simple" class="size-3.5" />
              Password
              <span class="text-base-content/30 ml-1">optional</span>
            </span>
          </label>
          <div class="flex gap-2">
            <div class="relative flex-1">
              <input
                id="dc-password"
                type={showPassword ? 'text' : 'password'}
                class="input input-bordered input-sm w-full pr-9"
                placeholder="Leave blank if none"
                bind:value={password}
                onkeydown={handleKeydown}
              />
              <button
                class="absolute right-2 top-1/2 -translate-y-1/2 text-base-content/40 hover:text-base-content transition-colors"
                onclick={() => (showPassword = !showPassword)}
                type="button"
                title={showPassword ? 'Hide' : 'Show'}
              >
                <Icon icon={showPassword ? 'ph:eye-slash' : 'ph:eye'} class="size-4" />
              </button>
            </div>
          </div>
        </div>

        <!-- Actions -->
        <div class="flex gap-2 pt-1">
          <button
            class="btn btn-ghost btn-sm gap-1.5"
            onclick={queryInfo}
            disabled={!address || a2sLoading}
          >
            {#if a2sLoading}
              <span class="loading loading-spinner loading-xs"></span>
              Querying…
            {:else}
              <Icon icon="ph:magnifying-glass" class="size-4" />
              Query Server
            {/if}
          </button>
          <button
            class="btn btn-primary btn-sm flex-1 gap-1.5"
            onclick={connect}
            disabled={!address}
          >
            <Icon icon="ph:rocket-launch" class="size-4" />
            Connect
          </button>
        </div>
      </div>
    </div>

    <!-- Error -->
    {#if a2sError}
      <div class="flex items-start gap-2.5 px-3.5 py-3 rounded-xl bg-error/10 border border-error/25 text-sm text-error">
        <Icon icon="ph:warning-circle" class="size-4 shrink-0 mt-0.5" />
        <div>
          <p class="font-medium text-sm">Query failed</p>
          <p class="text-xs mt-0.5 opacity-80 break-all">{a2sError}</p>
        </div>
      </div>
    {/if}

    <!-- Rich server info card -->
    {#if showCard}
      {@const fs = foundServer}
      {@const players = a2s?.players ?? fs?.players ?? 0}
      {@const maxPlayers = a2s?.max_players ?? fs?.max_players ?? 0}
      {@const map = a2s?.map ?? fs?.map ?? ''}
      {@const version = a2s?.version ?? fs?.version ?? ''}
      {@const mods = displayMods}
      {@const name = displayName}

      <div class="rounded-xl border border-base-300 bg-base-100 overflow-hidden">

        <!-- Card header: name + source badge -->
        <div class="px-4 py-3 bg-base-200 border-b border-base-300 flex items-start gap-3">
          <Icon icon="ph:server" class="size-4 text-primary shrink-0 mt-0.5" />
          <div class="flex-1 min-w-0">
            <p class="font-semibold text-sm text-base-content leading-snug">{name}</p>
            <div class="flex items-center gap-2 mt-1 flex-wrap">
              {#if fs}
                <span class="badge badge-success badge-xs gap-1">
                  <Icon icon="ph:check-circle" class="size-2.5" />
                  In server list
                </span>
              {:else if a2s}
                <span class="badge badge-warning badge-xs gap-1">
                  <Icon icon="ph:question" class="size-2.5" />
                  Not in list
                </span>
              {/if}
              {#if resolvedQueryPort !== null}
                {#if resolvedPortKind === 'query'}
                  <span class="badge badge-xs gap-1 bg-sky-500/15 text-sky-400 border-sky-500/20" title="You entered the query (A2S) port">
                    <Icon icon="ph:plugs" class="size-2.5" />
                    Query port
                  </span>
                {:else if resolvedPortKind === 'game'}
                  <span class="badge badge-xs gap-1 bg-amber-500/15 text-amber-400 border-amber-500/20" title="You entered the game port — query port resolved from server list">
                    <Icon icon="ph:game-controller" class="size-2.5" />
                    Game port → Q:{resolvedQueryPort}
                  </span>
                {/if}
              {/if}
              {#if fs?.password}
                <span class="badge badge-error badge-xs gap-1">
                  <Icon icon="ph:lock-simple" class="size-2.5" />
                  Password
                </span>
              {/if}
              {#if fs?.battl_eye}
                <span class="badge badge-xs gap-1 bg-blue-500/15 text-blue-400 border-blue-500/20">
                  <Icon icon="ph:shield-check" class="size-2.5" />
                  BattlEye
                </span>
              {/if}
              {#if fs?.first_person_only}
                <span class="badge badge-xs gap-1 bg-violet-500/15 text-violet-400 border-violet-500/20">
                  <Icon icon="ph:eye" class="size-2.5" />
                  1PP
                </span>
              {/if}
              {#if fs?.vac}
                <span class="badge badge-xs gap-1 bg-base-content/10 text-base-content/50 border-base-content/20">
                  VAC
                </span>
              {/if}
            </div>
          </div>
          {#if isFavorite}
            <span class="btn btn-ghost btn-xs btn-square shrink-0 cursor-default" title="Already in favorites">
              <Icon icon="ph:star-fill" class="size-4 text-warning" />
            </span>
          {:else if onAddFavorite && name}
            <button
              class="btn btn-ghost btn-xs btn-square shrink-0"
              onclick={() => onAddFavorite!(name, address.trim(), parseInt(port, 10))}
              title="Add to favorites"
            >
              <Icon icon="ph:star" class="size-4 text-warning/60" />
            </button>
          {/if}
        </div>

        <div class="p-4 space-y-4">

          <!-- Player bar -->
          <div>
            <div class="flex items-center justify-between mb-1.5">
              <span class="text-xs text-base-content/50 flex items-center gap-1.5">
                <Icon icon="ph:users" class="size-3.5" />
                Players
              </span>
              <span class="text-sm font-bold {playerTextColor(players, maxPlayers)} tabular-nums">
                {players}<span class="text-base-content/30 font-normal">/{maxPlayers}</span>
              </span>
            </div>
            <div class="w-full h-2 rounded-full bg-base-300 overflow-hidden">
              <div
                class="h-full rounded-full transition-all {playerBarColor(players, maxPlayers)}"
                style="width: {maxPlayers > 0 ? Math.min(100, (players / maxPlayers) * 100) : 0}%"
              ></div>
            </div>
          </div>

          <!-- Info grid -->
          <div class="grid grid-cols-2 gap-x-6 gap-y-2 text-xs">
            <div class="text-base-content/50 flex items-center gap-1.5">
              <Icon icon="ph:map-trifold" class="size-3.5" />
              Map
            </div>
                  <span class="text-teal-400 font-medium">{map || '—'}</span>

            {#if fs}
              <div class="text-base-content/50 flex items-center gap-1.5">
                <Icon icon="ph:globe-hemisphere-west" class="size-3.5" />
                IP
              </div>
              <span class="font-mono text-base-content">{fs.ip}:{fs.game_port}</span>

              <div class="text-base-content/50 flex items-center gap-1.5">
                <Icon icon="ph:plugs" class="size-3.5" />
                Query port
              </div>
              <span class="font-mono text-base-content">{fs.query_port}</span>

              <div class="text-base-content/50 flex items-center gap-1.5">
                <Icon icon="ph:clock" class="size-3.5" />
                Server time
              </div>
              <span class="text-base-content">{fs.time || '—'}</span>

              <div class="text-base-content/50 flex items-center gap-1.5">
                <Icon icon="ph:monitor" class="size-3.5" />
                Platform
              </div>
              <span class="{fs.environment === 'w' ? 'text-info' : 'text-success'}">
                {fs.environment === 'w' ? 'Windows' : 'Linux'}
              </span>
            {:else if a2s}
              <div class="text-base-content/50 flex items-center gap-1.5">
                <Icon icon="ph:plugs" class="size-3.5" />
                Query port
              </div>
              <span class="font-mono text-base-content">{a2s.query_port}</span>
            {/if}

            {#if version}
              <div class="text-base-content/50 flex items-center gap-1.5">
                <Icon icon="ph:tag" class="size-3.5" />
                Version
              </div>
              <span class="text-base-content/70">{version}</span>
            {/if}
          </div>

          <!-- Mods -->
          {#if fullLoading}
            <div class="flex items-center gap-2 text-xs text-base-content/50">
              <span class="loading loading-spinner loading-xs"></span>
              Loading mods…
            </div>
          {:else if mods.length > 0}
            <div>
              <div class="flex items-center gap-1.5 mb-2">
                <Icon icon="ph:puzzle-piece" class="size-3.5 text-base-content/50" />
                <span class="text-xs font-semibold text-base-content/60 uppercase tracking-wide">
                  Mods ({mods.length})
                </span>
              </div>
              <div class="space-y-1 max-h-44 overflow-y-auto pr-1">
                {#each mods as mod}
                  {@const installed = installedIds.has(mod.steam_workshop_id)}
                  <div class="flex items-center gap-2 text-xs group">
                    <span class="flex-shrink-0 w-4 text-center font-bold {installed ? 'text-success' : 'text-error/70'}">
                      {installed ? '✓' : '−'}
                    </span>
                    <span class="truncate flex-1 text-base-content/80">{mod.name}</span>
                    <button
                      class="font-mono text-base-content/30 hover:text-primary transition-colors flex-shrink-0 opacity-0 group-hover:opacity-100"
                      onclick={(e) => { e.stopPropagation(); openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.steam_workshop_id}`); }}
                      title="Open on Steam Workshop"
                    >
                      {mod.steam_workshop_id}
                    </button>
                  </div>
                {/each}
              </div>
            </div>
          {:else if (fs?.mods_count ?? 0) > 0}
            <div class="text-xs text-base-content/40 italic flex items-center gap-1.5">
              <Icon icon="ph:puzzle-piece" class="size-3.5" />
              {fs!.mods_count} mod(s) — IDs not available
            </div>
          {:else}
            <div class="text-xs text-base-content/40 italic flex items-center gap-1.5">
              <Icon icon="ph:puzzle-piece" class="size-3.5" />
              No mods
            </div>
          {/if}

          <!-- Live players from A2S -->
          {#if a2s}
            <div>
              <div class="flex items-center gap-1.5 mb-2">
                <Icon icon="ph:users-three" class="size-3.5 text-base-content/50" />
                <span class="text-xs font-semibold text-base-content/60 uppercase tracking-wide">
                  Online Players
                </span>
                <span class="ml-auto text-xs text-base-content/40">
                  {a2s.players_list.length > 0 ? `${a2s.players_list.length} shown` : 'live count only'}
                </span>
              </div>
              {#if a2s.players_list.length > 0}
                <div class="space-y-1 max-h-40 overflow-y-auto pr-1">
                  {#each a2s.players_list as pl}
                    <div class="flex items-center gap-2 text-xs">
                      <Icon icon="ph:user" class="size-3 text-base-content/30 flex-shrink-0" />
                      <span class="truncate flex-1 text-base-content/80">{pl.name || '—'}</span>
                      <span class="text-base-content/40 tabular-nums flex-shrink-0">
                        {formatDuration(pl.duration)}
                      </span>
                    </div>
                  {/each}
                </div>
              {:else if a2s.players === 0}
                <p class="text-xs text-base-content/40 italic">No players online</p>
              {:else}
                <p class="text-xs text-base-content/40 italic">
                  {a2s.players} player{a2s.players !== 1 ? 's' : ''} online — names not reported by server
                </p>
              {/if}
            </div>
          {/if}

          <!-- Connect action at bottom of card -->
          <div class="flex gap-2 pt-1 border-t border-base-200">
            <button
              class="btn btn-ghost btn-sm gap-1.5"
              onclick={queryInfo}
              disabled={a2sLoading}
            >
              {#if a2sLoading}
                <span class="loading loading-spinner loading-xs"></span>
              {:else}
                <Icon icon="ph:arrows-clockwise" class="size-4" />
              {/if}
              Refresh
            </button>
            <button
              class="btn btn-primary btn-sm flex-1 gap-1.5"
              onclick={connect}
            >
              <Icon icon="ph:rocket-launch" class="size-4" />
              Connect
            </button>
          </div>
        </div>
      </div>
    {/if}

  </div>
</div>
