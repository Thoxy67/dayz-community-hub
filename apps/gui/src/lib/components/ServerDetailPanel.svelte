<script lang="ts">
  import type { ServerDto, ServerFullDto, ModDto, A2sDetailsDto, InstalledModDto, BattleMetricsDto } from '$lib/types';
  import {
    pingColor,
    playerFill,
    playerBarColor,
    formatDuration,
    sparklinePath,
    countryCodeToFlag,
    countryCodeToName,
    haversineDistance,
  } from '$lib/utils';
  import { createSimpleCopyState } from '$lib/utils/clipboard.svelte';
  import { serverData } from '$lib/services';
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '@iconify/svelte';
  import * as m from '$lib/paraglide/messages.js';

  /** Calculate relative time from ISO date string */
  function getServerAge(isoDate: string): string {
    const created = new Date(isoDate);
    const now = new Date();
    const diffMs = now.getTime() - created.getTime();
    const days = Math.floor(diffMs / (1000 * 60 * 60 * 24));
    if (days < 1) return m.detail_age_today();
    if (days === 1) return m.detail_age_day();
    if (days < 30) return m.detail_age_days({ count: days });
    const months = Math.floor(days / 30);
    if (months === 1) return m.detail_age_month();
    if (months < 12) return m.detail_age_months({ count: months });
    const years = Math.floor(days / 365);
    if (years === 1) return m.detail_age_year();
    return m.detail_age_years({ count: years });
  }

  const { copied: copiedIp, copy: copyToClipboard } = createSimpleCopyState();
  async function copyIp() {
    if (!server) return;
    await copyToClipboard(`${server.ip}:${server.game_port}`);
  }

  const { copied: copiedAll, copy: copyAllToClipboard } = createSimpleCopyState();
  async function copyAllDetails() {
    if (!server) return;
    const ping = pingMs !== null ? `${pingMs}ms` : 'N/A';
    const details = `${server.name} - ${server.ip}:${server.game_port} - ${server.map} - ${server.players}/${server.max_players} - ${ping}`;
    await copyAllToClipboard(details);
  }

  interface Props {
    server: ServerDto | null;
    a2s: A2sDetailsDto | null;
    a2sLoading: boolean;
    a2sError: string;
    installedMods: InstalledModDto[];
    pingMs: number | null;
    /** BattleMetrics personal access token from profile (null = not configured). */
    bmApiKey: string | null;
    /** User location [longitude, latitude] for distance calculation. */
    userLocation?: [number, number] | null;
    /** When true, scroll the mods section into view once it renders. */
    scrollToMods?: boolean;
    onClose: () => void;
    onQueryA2s: () => void;
  }

  let {
    server,
    a2s,
    a2sLoading,
    a2sError,
    installedMods,
    pingMs,
    bmApiKey,
    userLocation = null,
    scrollToMods = false,
    onClose,
    onQueryA2s,
  }: Props = $props();

  let modsHeading = $state<HTMLElement | null>(null);

  $effect(() => {
    if (scrollToMods && modsHeading) {
      // Small tick so the panel has fully rendered before scrolling.
      const id = setTimeout(() => modsHeading?.scrollIntoView({ behavior: 'smooth', block: 'start' }), 80);
      return () => clearTimeout(id);
    }
  });

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

  // A2S refresh debounce: prevent double-clicks from firing multiple queries
  let lastA2sRefreshTime = $state(0);
  const A2S_REFRESH_DEBOUNCE_MS = 1000;

  function handleA2sRefresh() {
    const now = Date.now();
    if (now - lastA2sRefreshTime < A2S_REFRESH_DEBOUNCE_MS) {
      // Too soon, ignore click
      return;
    }
    lastA2sRefreshTime = now;
    onQueryA2s();
  }

  // Prefer mods from get_server_details; fall back to a2s.mods if the list returned nothing.
  let displayMods = $derived(mods.length > 0 ? mods : (a2s?.mods ?? []));

  // A2S mods (from rules query) - shown when API mods are not available
  let a2sMods = $derived(a2s?.mods_from_a2s ?? []);

  // A2S rules (non-mod entries)
  let a2sRules = $derived(a2s?.rules ?? []);

  // Fetch full mod list when the selected server changes.
  // A 200ms debounce prevents firing an IPC call for every row the user
  // passes through while scrolling/navigating the server list quickly.
  $effect(() => {
    if (!server) {
      mods = [];
      modsFetchFailed = false;
      fetchedKey = '';
      clearTimeout(_detailDebounce);
      return;
    }

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
      if (!server || `${server.ip}:${server.query_port}` !== key) return;
      modsLoading = true;
      invoke<ServerFullDto>('get_server_details', { ip: server.ip, port: server.query_port })
        .then((full) => {
          if (server && `${server.ip}:${server.query_port}` === key) {
            mods = full.mods;
            modsFetchFailed = false;
            fetchedKey = key;
          }
        })
        .catch(() => {
          if (server && `${server.ip}:${server.query_port}` === key) {
            mods = [];
            modsFetchFailed = true;
            fetchedKey = key;
          }
        })
        .finally(() => {
          modsLoading = false;
        });
    }, 200);

    return () => clearTimeout(_detailDebounce);
  });

  // ── BattleMetrics (using serverData service) ───────────────────────────────
  let bm = $derived(server ? serverData.getBm(server.ip, server.game_port, server.query_port)?.data : null);
  let bmLoading = $derived(server ? serverData.isBmLoading(server.ip, server.game_port, server.query_port) : false);
  let bmError = $derived(server ? serverData.getBmError(server.ip, server.game_port, server.query_port) : null);
  let _bmDebounce: ReturnType<typeof setTimeout> | undefined;

  // Fetch BM when server changes (debounced)
  $effect(() => {
    if (!server || !bmApiKey) {
      clearTimeout(_bmDebounce);
      return;
    }

    const ip = server.ip;
    const port = server.game_port;
    const queryPort = server.query_port;
    const name = server.name;

    clearTimeout(_bmDebounce);
    _bmDebounce = setTimeout(() => {
      serverData.fetchBattleMetrics(ip, port, queryPort, name);
    }, 100);

    return () => clearTimeout(_bmDebounce);
  });

  function refreshBm() {
    if (!server) return;
    serverData.refreshBattleMetrics(server.ip, server.game_port, server.query_port, server.name);
  }

  // Player-history sparkline stats — computed once per history change instead
  // of inline on every render.  Uses reduce (not Math.min/max spread) so large
  // history arrays don't blow the call stack or re-scan multiple times.
  let playerHistoryStats = $derived.by(() => {
    const hist = bm?.player_history;
    if (!hist || hist.length < 2) return null;
    let min = Infinity;
    let max = -Infinity;
    let sum = 0;
    for (const [, c] of hist) {
      if (c < min) min = c;
      if (c > max) max = c;
      sum += c;
    }
    return {
      min,
      max,
      current: hist[hist.length - 1][1],
      avg: Math.round(sum / hist.length),
      path: sparklinePath(hist),
    };
  });
</script>

<div class="flex flex-col h-full overflow-hidden border-l border-base-300 bg-base-100">
  <!-- Header -->
  <div class="flex items-center justify-between px-3 py-2 bg-base-200 border-b border-base-300 flex-shrink-0">
    <span class="font-semibold text-sm truncate text-base-content">{server?.name ?? 'Server Details'}</span>
    <div class="flex items-center gap-1">
      <button
        class="btn btn-ghost btn-xs gap-1"
        onclick={copyAllDetails}
        title={m.detail_copy_all()}
        disabled={!server}
      >
        {#if copiedAll}
          <Icon icon="ph:check" class="size-3.5 text-success" />
        {:else}
          <Icon icon="ph:copy" class="size-3.5" />
        {/if}
      </button>
      <button
        class="btn btn-ghost btn-xs"
        onclick={onClose}
        aria-label={m.fav_close_details()}
        title={m.fav_close_details()}
      >
        <Icon icon="ph:x" class="size-3.5" />
      </button>
    </div>
  </div>

  {#if !server}
    <!-- Fallback UI when server is not in master list -->
    <div class="flex-1 overflow-y-auto p-3 space-y-3 text-sm">
      <div
        class="flex items-start gap-2 px-2.5 py-2 rounded-lg bg-warning/10 border border-warning/25 text-xs text-warning"
      >
        <Icon icon="ph:warning-circle" class="size-3.5 shrink-0 mt-0.5" />
        <span class="leading-snug">Server not found in master server list</span>
      </div>

      <!-- Show A2S info if available -->
      {#if a2s}
        <div>
          <span class="text-xs font-semibold text-base-content/50">{m.detail_a2s_title()}</span>
          <div class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs mt-2">
            <div class="text-base-content/50">{m.detail_a2s_live_players()}</div>
            <div class="font-bold text-success">{a2s.players}/{a2s.max_players}</div>
            <div class="text-base-content/50">{m.detail_map()}</div>
            <div class="text-accent-map">{a2s.map}</div>
            <div class="text-base-content/50">{m.detail_version()}</div>
            <div class="text-base-content/70">{a2s.version}</div>
            <div class="text-base-content/50">{m.detail_a2s_game()}</div>
            <div class="text-base-content/70">{a2s.game}</div>
          </div>
        </div>

        {#if a2s.players_list.length > 0}
          <div>
            <div class="text-xs text-base-content/50 mb-1">{m.detail_a2s_online_players()}</div>
            <div class="space-y-0.5 max-h-32 overflow-y-auto">
              {#each a2s.players_list as player}
                <div class="flex items-center gap-2 text-xs">
                  <span class="text-base-content/80 truncate">{player.name}</span>
                  <span class="text-base-content/40 ml-auto tabular-nums">{formatDuration(player.duration)}</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}

        {#if a2s.mods && a2s.mods.length > 0}
          <div>
            <div class="text-xs font-semibold text-base-content/50 mb-1">
              {m.detail_mods_count({ count: a2s.mods.length })}
            </div>
            <div class="space-y-0.5 max-h-40 overflow-y-auto">
              {#each a2s.mods as mod}
                <div class="flex items-center gap-1.5 text-xs">
                  <span class="truncate text-base-content/80">{mod.name}</span>
                  <button
                    class="text-base-content/40 ml-auto font-mono flex-shrink-0 hover:text-primary transition-colors"
                    onclick={(e) => {
                      e.stopPropagation();
                      openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.steam_workshop_id}`);
                    }}
                    title={m.detail_open_workshop()}>{mod.steam_workshop_id}</button
                  >
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {:else}
        <p class="text-xs text-base-content/40 italic">{m.detail_a2s_click_refresh()}</p>
        <button class="btn btn-ghost btn-xs" onclick={handleA2sRefresh} disabled={a2sLoading}>
          {#if a2sLoading}
            <span class="loading loading-spinner loading-xs"></span>
          {:else}
            {m.servers_refresh()}
          {/if}
        </button>
      {/if}
    </div>
  {:else}
    <!-- Full server details when server is available -->
    <div class="flex-1 overflow-y-auto p-3 space-y-3 text-sm">
      <!-- Core info grid -->
      <div class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
        <div class="text-base-content/50">IP</div>
        <button
          class="font-mono text-base-content hover:text-primary transition-colors flex items-center gap-1 group/ip text-left"
          title={m.detail_copy_ip()}
          onclick={copyIp}
        >
          {#if copiedIp}
            <span class="text-success">{m.servers_copied()}</span>
          {:else}
            {server.ip}:{server.game_port}
            <Icon icon="ph:copy" class="size-3 opacity-0 group-hover/ip:opacity-100 transition-opacity" />
          {/if}
        </button>

        <div class="text-base-content/50">{m.detail_query_port()}</div>
        <div class="font-mono text-base-content">{server.query_port}</div>

        <div class="text-base-content/50">{m.detail_players()}</div>
        <div class="flex flex-col gap-0.5">
          <span class="font-bold {playerFill(server.players, server.max_players, '40')}">
            {server.players}/{server.max_players}
          </span>
          <span class="block h-1 w-full rounded-full bg-base-300 overflow-hidden">
            <span
              class="block h-full rounded-full transition-[width] duration-500 ease-out {playerBarColor(
                server.players,
                server.max_players,
              )}"
              style="width:{server.max_players > 0
                ? Math.min(100, Math.round((server.players / server.max_players) * 100))
                : 0}%"
            ></span>
          </span>
        </div>

        <div class="text-base-content/50">{m.detail_map()}</div>
        <div class="text-accent-map">{server.map}</div>

        <div class="text-base-content/50">{m.detail_version()}</div>
        <div class="text-base-content/70">{server.version}</div>

        <div class="text-base-content/50">{m.detail_time()}</div>
        <div class="text-base-content/70">{server.time}</div>

        <div class="text-base-content/50">{m.detail_ping()}</div>
        <div class="font-mono {pingColor(pingMs)}">
          {pingMs !== null ? `${pingMs} ms` : '—'}
        </div>

        <div class="text-base-content/50">{m.detail_platform()}</div>
        <div class={server.environment === 'w' ? 'text-info' : 'text-success'}>
          {server.environment === 'w' ? m.servers_os_windows() : m.servers_os_linux()}
        </div>

        <div class="text-base-content/50">{m.detail_first_person()}</div>
        <div>{server.first_person_only ? m.detail_yes() : m.detail_no()}</div>

        <div class="text-base-content/50">{m.detail_password()}</div>
        <div class={server.password ? 'text-error' : 'text-base-content/40'}>
          {server.password ? m.detail_yes() : m.detail_no()}
        </div>

        <div class="text-base-content/50">{m.detail_vac()}</div>
        <div>{server.vac ? m.detail_yes() : m.detail_no()}</div>

        {#if server.battl_eye !== null}
          <div class="text-base-content/50">{m.detail_battleye()}</div>
          <div>{server.battl_eye ? m.detail_yes() : m.detail_no()}</div>
        {/if}
      </div>

      <!-- Mods -->
      {#if modsLoading}
        <div class="flex items-center gap-2 text-xs text-base-content/50">
          <span class="loading loading-spinner loading-xs"></span>
          {m.detail_loading_mods({ count: server.mods_count })}
        </div>
      {:else if displayMods.length > 0}
        <div>
          <div bind:this={modsHeading} class="text-xs font-semibold text-base-content/50 mb-1">
            {m.detail_mods_count({ count: displayMods.length })}
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
                  onclick={(e) => {
                    e.stopPropagation();
                    openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.steam_workshop_id}`);
                  }}
                  title={m.detail_open_workshop()}>{mod.steam_workshop_id}</button
                >
              </div>
            {/each}
          </div>
        </div>

        <!-- A2S Mods (when API mods not available) -->
      {:else if a2sMods.length > 0}
        <div>
          <div class="text-xs font-semibold text-base-content/50 mb-1">
            Mods from A2S ({a2sMods.length})
            <span class="text-base-content/40 font-normal italic">- Workshop IDs not available</span>
          </div>
          <div class="space-y-0.5 max-h-40 overflow-y-auto">
            {#each a2sMods as modName}
              <div class="flex items-center gap-1.5 text-xs">
                <span class="text-base-content/40 font-bold w-3 text-center flex-shrink-0">•</span>
                <span class="truncate text-base-content/80">{modName}</span>
              </div>
            {/each}
          </div>
        </div>
      {:else if server.mods_count > 0}
        <div class="flex items-center gap-2 text-xs text-base-content/40">
          {#if modsFetchFailed}
            <Icon icon="ph:warning-circle" class="size-3.5 text-warning/60 shrink-0" />
            <span class="italic">{m.detail_mods_failed({ count: server.mods_count })}</span>
            <button
              class="btn btn-ghost btn-xs h-5 min-h-0 px-1.5 ml-auto"
              onclick={() => {
                fetchedKey = '';
              }}
              title={m.detail_retry()}
            >
              <Icon icon="ph:arrows-clockwise" class="size-3" />
            </button>
          {:else}
            <span class="italic">{m.detail_mods_not_reported({ count: server.mods_count })}</span>
            {#if !a2s}
              <button
                class="btn btn-ghost btn-xs h-5 min-h-0 px-1.5 ml-auto gap-1"
                onclick={handleA2sRefresh}
                title={m.detail_query_mods()}
              >
                <Icon icon="ph:broadcast" class="size-3" />
                {m.detail_query_a2s()}
              </button>
            {/if}
          {/if}
        </div>

        <!-- A2S Rules (server configuration) -->
        {#if a2sRules.length > 0}
          <div class="mt-3">
            <div class="text-xs font-semibold text-base-content/50 mb-1">
              Server Rules / Configuration ({a2sRules.length})
            </div>
            <div class="space-y-0.5 max-h-40 overflow-y-auto">
              {#each a2sRules as rule}
                <div class="flex items-start gap-2 text-xs">
                  <span class="font-mono text-base-content/60 min-w-[120px] flex-shrink-0">{rule.name}:</span>
                  <span class="text-base-content/80 break-all">{rule.value}</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      {:else}
        <div class="text-xs text-base-content/40 italic">{m.detail_no_mods()}</div>
      {/if}

      <!-- BattleMetrics -->
      <div>
        <div class="flex items-center justify-between mb-1">
          <span class="text-xs font-semibold text-base-content/50 flex items-center gap-1.5">
            <Icon icon="ph:chart-line-up" class="size-3.5" />
            {m.detail_bm_title()}
          </span>
          {#if bmApiKey}
            <button
              class="btn btn-ghost btn-xs"
              onclick={refreshBm}
              disabled={bmLoading}
              title={m.detail_bm_refresh_title()}
            >
              {#if bmLoading}
                <span class="loading loading-spinner loading-xs"></span>
              {:else}
                {m.servers_refresh()}
              {/if}
            </button>
          {/if}
        </div>

        {#if !bmApiKey}
          <p class="text-xs text-base-content/35 italic">
            {m.detail_bm_configure()}
          </p>
        {:else if bmLoading}
          <div class="flex items-center gap-2 text-xs text-base-content/50">
            <span class="loading loading-spinner loading-xs"></span>
            {m.detail_bm_loading()}
          </div>
        {:else if bm}
          <!-- Server type badges -->
          <div class="flex flex-wrap gap-1 mb-2">
            {#if bm.official}
              <span class="badge badge-xs badge-rank gap-1">
                <Icon icon="ph:seal-check-fill" class="size-2.5" />{m.detail_bm_official()}
              </span>
            {/if}
            {#if bm.private}
              <span class="badge badge-xs badge-status gap-1">
                <Icon icon="ph:lock-fill" class="size-2.5" />{m.detail_bm_private()}
              </span>
            {/if}
            {#if bm.third_person}
              <span class="badge badge-xs badge-country gap-1">
                <Icon icon="ph:eye" class="size-2.5" />{m.detail_bm_3pp()}
              </span>
            {:else if bm.third_person === false}
              <span class="badge badge-xs badge-players gap-1">
                <Icon icon="ph:crosshair-simple" class="size-2.5" />{m.detail_bm_1pp()}
              </span>
            {/if}
            {#if bm.modded}
              <span class="badge badge-xs badge-firstperson gap-1">
                <Icon icon="ph:puzzle-piece" class="size-2.5" />{m.detail_bm_modded()}
              </span>
            {/if}
            {#if bm.query_status === 'valid'}
              <span class="badge badge-xs badge-official gap-1">
                <Icon icon="ph:wifi-high" class="size-2.5" />{m.detail_bm_online()}
              </span>
            {:else if bm.query_status}
              <span class="badge badge-xs badge-custom gap-1">
                <Icon icon="ph:wifi-slash" class="size-2.5" />{bm.query_status}
              </span>
            {/if}
          </div>

          <!-- Server name from BattleMetrics -->
          <div class="text-xs text-base-content/70 mb-2 truncate" title={bm.name}>
            <span class="text-base-content/40">{m.detail_bm_name()}:</span>
            {bm.name}
          </div>

          <!-- Stats grid -->
          <div class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs mb-2">
            {#if bm.rank !== null}
              <div class="text-base-content/50">{m.detail_bm_rank()}</div>
              <div class="font-mono font-bold text-primary">#{bm.rank}</div>
            {/if}
            {#if bm.players !== null}
              <div class="text-base-content/50">{m.detail_bm_players()}</div>
              <div class="font-mono">{bm.players}/{bm.max_players ?? '?'}</div>
            {/if}
            <div class="text-base-content/50">{m.detail_bm_status()}</div>
            <div class="flex items-center gap-1.5">
              <span
                class="size-2 rounded-full flex-shrink-0 {bm.status === 'online'
                  ? 'bg-success'
                  : bm.status === 'offline'
                    ? 'bg-error'
                    : 'bg-base-content/30'}"
              ></span>
              <span
                class={bm.status === 'online'
                  ? 'text-success'
                  : bm.status === 'offline'
                    ? 'text-error'
                    : 'text-base-content/50'}>{bm.status}</span
              >
            </div>
            {#if bm.country}
              <div class="text-base-content/50">{m.detail_bm_country()}</div>
              <div class="flex items-center gap-1.5">
                <span>{countryCodeToFlag(bm.country)}</span>
                <span>{countryCodeToName(bm.country)}</span>
              </div>
            {/if}
            {#if bm.location && userLocation}
              {@const dist = haversineDistance(userLocation[1], userLocation[0], bm.location[1], bm.location[0])}
              <div class="text-base-content/50">{m.detail_bm_distance()}</div>
              <div class="font-mono">{dist.toFixed(0)} km</div>
            {/if}
            {#if bm.uptime !== null}
              <div class="text-base-content/50">{m.detail_bm_uptime()}</div>
              <div
                class={(bm.uptime ?? 0) >= 90 ? 'text-success' : (bm.uptime ?? 0) >= 70 ? 'text-warning' : 'text-error'}
              >
                {bm.uptime?.toFixed(1)}%
              </div>
            {/if}
            {#if bm.created_at}
              <div class="text-base-content/50">{m.detail_bm_first_seen()}</div>
              <div class="text-base-content/70" title={bm.created_at}>
                {m.detail_ago({ time: getServerAge(bm.created_at) })}
              </div>
            {/if}
            {#if bm.server_steam_id}
              <div class="text-base-content/50">{m.detail_bm_steam_id()}</div>
              <button
                class="text-left font-mono text-base-content/60 hover:text-primary truncate"
                onclick={() => openUrl(`https://steamcommunity.com/profiles/${bm?.server_steam_id}`)}
                title={m.detail_bm_open_steam()}
              >
                {bm.server_steam_id.slice(-8)}…
              </button>
            {/if}
          </div>

          <!-- Player history sparkline -->
          {#if playerHistoryStats}
            <div class="mb-2">
              <div class="flex items-center justify-between mb-1">
                <span class="text-xs text-base-content/40">{m.detail_bm_player_history()}</span>
                <div class="flex items-center gap-2 text-xs font-mono">
                  <span class="text-base-content/30" title={m.detail_bm_min()}>{playerHistoryStats.min}</span>
                  <span class="text-base-content/50">–</span>
                  <span class="text-primary font-semibold" title={m.detail_bm_current()}
                    >{playerHistoryStats.current}</span
                  >
                  <span class="text-base-content/50">–</span>
                  <span class="text-base-content/30" title={m.detail_bm_max()}>{playerHistoryStats.max}</span>
                </div>
              </div>
              <svg viewBox="0 0 120 28" class="w-full h-7 text-primary" preserveAspectRatio="none">
                <path
                  d={playerHistoryStats.path}
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.5"
                  stroke-linecap="round"
                  stroke-linejoin="round"
                />
              </svg>
              <div class="flex items-center justify-between text-xs text-base-content/25 mt-0.5">
                <span>{m.detail_bm_24h_ago()}</span>
                <span>{m.detail_bm_avg({ count: playerHistoryStats.avg })}</span>
                <span>{m.detail_bm_now()}</span>
              </div>
            </div>
          {/if}

          <!-- Link to BattleMetrics -->
          <button
            class="btn btn-ghost btn-xs gap-1 text-base-content/50 hover:text-primary"
            onclick={() => openUrl(`https://www.battlemetrics.com/servers/dayz/${bm?.id}`)}
          >
            <Icon icon="ph:arrow-square-out" class="size-3.5" />
            {m.detail_bm_view()}
          </button>
        {:else if bmError}
          <div
            class="flex items-start gap-2 px-2.5 py-2 rounded-lg bg-error/10 border border-error/25 text-xs text-error"
          >
            <Icon icon="ph:warning-circle" class="size-3.5 shrink-0 mt-0.5" />
            <span class="leading-snug break-all flex-1">{bmError}</span>
            <button
              class="btn btn-ghost btn-xs h-5 min-h-0 px-1.5 shrink-0"
              onclick={refreshBm}
              title={m.detail_retry()}
            >
              <Icon icon="ph:arrows-clockwise" class="size-3" />
            </button>
          </div>
        {/if}
      </div>

      <!-- A2S live info -->
      <div>
        <div class="flex items-center justify-between mb-1">
          <span class="text-xs font-semibold text-base-content/50">{m.detail_a2s_title()}</span>
          <button class="btn btn-ghost btn-xs" onclick={handleA2sRefresh} disabled={a2sLoading}>
            {#if a2sLoading}
              <span class="loading loading-spinner loading-xs"></span>
            {:else}
              {m.servers_refresh()}
            {/if}
          </button>
        </div>

        {#if a2s}
          <div class="grid grid-cols-2 gap-x-4 gap-y-1 text-xs mb-2">
            <div class="text-base-content/50">{m.detail_a2s_live_players()}</div>
            <div class="font-bold text-success">{a2s.players}/{a2s.max_players}</div>
            <div class="text-base-content/50">{m.detail_bots()}</div>
            <div
              class="font-bold flex items-center gap-1 {a2s.bots > 0 ? 'text-warning' : 'text-base-content/70'}"
              title={a2s.bots > 0 ? m.servers_bots({ count: a2s.bots }) : undefined}
            >
              {#if a2s.bots > 0}<Icon icon="mdi:robot" class="size-3.5 shrink-0" />{/if}{a2s.bots}
            </div>
            <div class="text-base-content/50">{m.detail_a2s_game()}</div>
            <div>{a2s.game}</div>
          </div>

          {#if a2s.players_list.length > 0}
            <div class="text-xs text-base-content/50 mb-1">{m.detail_a2s_online_players()}</div>
            <div class="space-y-0.5 max-h-32 overflow-y-auto">
              {#each a2s.players_list as player}
                <div class="flex items-center gap-2 text-xs">
                  <span class="text-base-content/80 truncate">{player.name}</span>
                  <span class="text-base-content/40 ml-auto tabular-nums">{formatDuration(player.duration)}</span>
                </div>
              {/each}
            </div>
          {:else if a2s.players === 0}
            <p class="text-xs text-base-content/40 italic">{m.detail_a2s_no_players()}</p>
          {:else}
            <p class="text-xs text-base-content/40 italic">{m.detail_a2s_names_not_reported()}</p>
          {/if}
        {:else if a2sError}
          <div
            class="flex items-start gap-2 px-2.5 py-2 rounded-lg bg-error/10 border border-error/25 text-xs text-error"
          >
            <Icon icon="ph:warning-circle" class="size-3.5 shrink-0 mt-0.5" />
            <span class="leading-snug break-all">{a2sError}</span>
          </div>
        {:else if !a2sLoading}
          <p class="text-xs text-base-content/40 italic">{m.detail_a2s_click_refresh()}</p>
        {/if}
      </div>
    </div>
  {/if}
</div>
