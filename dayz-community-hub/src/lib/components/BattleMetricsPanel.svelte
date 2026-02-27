<script lang="ts">
  import type { BattleMetricsDto } from '$lib/types';
  import { sparklinePath } from '$lib/utils';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '@iconify/svelte';

  interface Props {
    bm: BattleMetricsDto | null;
    bmLoading: boolean;
    bmError: string;
    bmApiKey: string | null;
    onRetry: () => void;
    /** SVG viewBox height for the sparkline (default 24). */
    sparklineHeight?: number;
  }

  let {
    bm,
    bmLoading,
    bmError,
    bmApiKey,
    onRetry,
    sparklineHeight = 24,
  }: Props = $props();
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
    <div class="grid grid-cols-2 gap-x-3 gap-y-1 text-xs">
      {#if bm.rank !== null}
        <span class="text-base-content/50">Rank</span>
        <span class="font-mono font-bold text-primary">#{bm.rank}</span>
      {/if}
      <span class="text-base-content/50">Status</span>
      <span class="flex items-center gap-1.5">
        <span class="size-1.5 rounded-full flex-shrink-0 {bm.status === 'online' ? 'bg-success' : bm.status === 'offline' ? 'bg-error' : 'bg-base-content/30'}"></span>
        <span class="{bm.status === 'online' ? 'text-success' : bm.status === 'offline' ? 'text-error' : 'text-base-content/50'}">{bm.status}</span>
      </span>
      {#if bm.country}
        <span class="text-base-content/50">Country</span>
        <span class="font-mono">{bm.country}</span>
      {/if}
      {#if bm.uptime !== null}
        <span class="text-base-content/50">Uptime</span>
        <span class="{(bm.uptime ?? 0) >= 90 ? 'text-success' : (bm.uptime ?? 0) >= 70 ? 'text-warning' : 'text-error'}">{bm.uptime?.toFixed(1)}%</span>
      {/if}
    </div>
    {#if bm.player_history.length >= 2}
      <div>
        <div class="text-xs text-base-content/35 mb-1">Player count (24 h)</div>
        <svg viewBox="0 0 120 {sparklineHeight}" class="w-full h-6 text-primary" preserveAspectRatio="none">
          <path d={sparklinePath(bm.player_history, 120, sparklineHeight)} fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
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
      <button
        class="btn btn-ghost btn-xs h-5 min-h-0 px-1 shrink-0"
        onclick={onRetry}
        title="Retry"
      ><Icon icon="ph:arrows-clockwise" class="size-3" /></button>
    </div>
  {/if}
</div>
