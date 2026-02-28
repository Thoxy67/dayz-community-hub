<script lang="ts">
  import type { BattleMetricsDto } from '$lib/types';
  import { sparklinePath, countryCodeToFlag, countryCodeToName, haversineDistance } from '$lib/utils';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '@iconify/svelte';

  interface Props {
    bm: BattleMetricsDto | null;
    bmLoading: boolean;
    bmError: string;
    bmApiKey: string | null;
    /** User location [longitude, latitude] for distance calculation. */
    userLocation?: [number, number] | null;
    onRetry: () => void;
    /** SVG viewBox height for the sparkline (default 24). */
    sparklineHeight?: number;
  }

  let { bm, bmLoading, bmError, bmApiKey, userLocation = null, onRetry, sparklineHeight = 24 }: Props = $props();

  /** Calculate relative time from ISO date string */
  function getServerAge(isoDate: string): string {
    const created = new Date(isoDate);
    const now = new Date();
    const diffMs = now.getTime() - created.getTime();
    const days = Math.floor(diffMs / (1000 * 60 * 60 * 24));
    if (days < 1) return 'today';
    if (days === 1) return '1 day';
    if (days < 30) return `${days} days`;
    const months = Math.floor(days / 30);
    if (months === 1) return '1 month';
    if (months < 12) return `${months} months`;
    const years = Math.floor(days / 365);
    if (years === 1) return '1 year';
    return `${years} years`;
  }
</script>

<div class="px-3 py-2 border-t border-base-300 flex-shrink-0 space-y-2">
  <div class="flex items-center justify-between">
    <span class="text-xs font-semibold text-base-content/50 flex items-center gap-1.5">
      <Icon icon="ph:chart-line-up" class="size-3.5" />
      BattleMetrics
    </span>
    {#if bmApiKey}
      <button
        class="btn btn-ghost btn-xs h-5 min-h-0 px-1.5"
        onclick={onRetry}
        disabled={bmLoading}
        title="Refresh BattleMetrics"
      >
        {#if bmLoading}
          <span class="loading loading-spinner loading-xs"></span>
        {:else}
          <Icon icon="ph:arrows-clockwise" class="size-3" />
        {/if}
      </button>
    {/if}
  </div>

  {#if !bmApiKey}
    <p class="text-xs text-base-content/30 italic">Configure a BattleMetrics API token in settings.</p>
  {:else if bmLoading}
    <div class="flex items-center gap-1.5 text-xs text-base-content/40">
      <span class="loading loading-spinner loading-xs"></span>
      Loading…
    </div>
  {:else if bm}
    <!-- Server type badges -->
    <div class="flex flex-wrap gap-1 mb-1.5">
      {#if bm.official}
        <span class="badge badge-xs bg-amber-500/15 text-amber-500 border-amber-500/30 gap-1">
          <Icon icon="ph:seal-check-fill" class="size-2.5" />official
        </span>
      {/if}
      {#if bm.private}
        <span class="badge badge-xs bg-rose-500/15 text-rose-400 border-rose-500/30 gap-1">
          <Icon icon="ph:lock-fill" class="size-2.5" />private
        </span>
      {/if}
      {#if bm.third_person}
        <span class="badge badge-xs bg-sky-500/15 text-sky-400 border-sky-500/30 gap-1">
          <Icon icon="ph:eye" class="size-2.5" />3PP
        </span>
      {:else if bm.third_person === false}
        <span class="badge badge-xs bg-violet-500/15 text-violet-400 border-violet-500/30 gap-1">
          <Icon icon="ph:crosshair-simple" class="size-2.5" />1PP
        </span>
      {/if}
      {#if bm.modded}
        <span class="badge badge-xs bg-fuchsia-500/15 text-fuchsia-400 border-fuchsia-500/30 gap-1">
          <Icon icon="ph:puzzle-piece" class="size-2.5" />modded
        </span>
      {/if}
      {#if bm.query_status === 'valid'}
        <span class="badge badge-xs bg-emerald-500/15 text-emerald-400 border-emerald-500/30 gap-1">
          <Icon icon="ph:wifi-high" class="size-2.5" />online
        </span>
      {:else if bm.query_status}
        <span class="badge badge-xs bg-orange-500/15 text-orange-400 border-orange-500/30 gap-1">
          <Icon icon="ph:wifi-slash" class="size-2.5" />{bm.query_status}
        </span>
      {/if}
    </div>

    <div class="grid grid-cols-2 gap-x-3 gap-y-1 text-xs">
      {#if bm.rank !== null}
        <span class="text-base-content/50">Rank</span>
        <span class="font-mono font-bold text-primary">#{bm.rank}</span>
      {/if}
      <span class="text-base-content/50">Status</span>
      <span class="flex items-center gap-1.5">
        <span
          class="size-1.5 rounded-full flex-shrink-0 {bm.status === 'online'
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
      </span>
      {#if bm.country}
        <span class="text-base-content/50">Country</span>
        <span class="flex items-center gap-1.5">
          <span>{countryCodeToFlag(bm.country)}</span>
          <span>{countryCodeToName(bm.country)}</span>
        </span>
      {/if}
      {#if bm.location && userLocation}
        {@const dist = haversineDistance(userLocation[1], userLocation[0], bm.location[1], bm.location[0])}
        <span class="text-base-content/50">Distance</span>
        <span class="font-mono">{dist.toFixed(0)} km</span>
      {/if}
      {#if bm.uptime !== null}
        <span class="text-base-content/50">Uptime</span>
        <span class={(bm.uptime ?? 0) >= 90 ? 'text-success' : (bm.uptime ?? 0) >= 70 ? 'text-warning' : 'text-error'}
          >{bm.uptime?.toFixed(1)}%</span
        >
      {/if}
      {#if bm.created_at}
        <span class="text-base-content/50">First seen</span>
        <span class="text-base-content/70" title={bm.created_at}>{getServerAge(bm.created_at)} ago</span>
      {/if}
      {#if bm.server_steam_id}
        <span class="text-base-content/50">Steam ID</span>
        <button
          class="text-left font-mono text-base-content/60 hover:text-primary truncate"
          onclick={() => openUrl(`https://steamcommunity.com/profiles/${bm?.server_steam_id}`)}
          title="Open Steam profile"
        >
          {bm.server_steam_id.slice(-8)}…
        </button>
      {/if}
    </div>
    {#if bm.player_history.length >= 2}
      {@const counts = bm.player_history.map(([, c]) => c)}
      {@const minCount = Math.min(...counts)}
      {@const maxCount = Math.max(...counts)}
      {@const currentCount = counts[counts.length - 1]}
      {@const avgCount = Math.round(counts.reduce((a, b) => a + b, 0) / counts.length)}
      <div>
        <div class="flex items-center justify-between mb-1">
          <span class="text-xs text-base-content/35">Player count (24 h)</span>
          <div class="flex items-center gap-2 text-xs font-mono">
            <span class="text-base-content/30" title="Min">{minCount}</span>
            <span class="text-base-content/50">–</span>
            <span class="text-primary font-semibold" title="Current">{currentCount}</span>
            <span class="text-base-content/50">–</span>
            <span class="text-base-content/30" title="Max">{maxCount}</span>
          </div>
        </div>
        <div class="relative">
          <svg viewBox="0 0 120 {sparklineHeight}" class="w-full h-6 text-primary" preserveAspectRatio="none">
            <path
              d={sparklinePath(bm.player_history, 120, sparklineHeight)}
              fill="none"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </div>
        <div class="flex items-center justify-between text-xs text-base-content/25 mt-0.5">
          <span>24h ago</span>
          <span>avg: {avgCount}</span>
          <span>now</span>
        </div>
      </div>
    {/if}
    <button
      class="btn btn-ghost btn-xs gap-1 text-base-content/40 hover:text-primary w-full"
      title="Open this server's BattleMetrics page in browser"
      onclick={() => openUrl(`https://www.battlemetrics.com/servers/dayz/${bm?.id}`)}
    >
      <Icon icon="ph:arrow-square-out" class="size-3.5" />
      View on BattleMetrics
    </button>
  {:else if bmError}
    <div class="flex items-start gap-1.5 text-xs text-error">
      <Icon icon="ph:warning-circle" class="size-3.5 shrink-0 mt-0.5" />
      <span class="leading-snug break-all flex-1">{bmError}</span>
      <button class="btn btn-ghost btn-xs h-5 min-h-0 px-1 shrink-0" onclick={onRetry} title="Retry"
        ><Icon icon="ph:arrows-clockwise" class="size-3" /></button
      >
    </div>
  {/if}
</div>
