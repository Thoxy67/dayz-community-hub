<script lang="ts">
  import Icon from '@iconify/svelte';

  interface Props {
    missions: string[];
    loading: boolean;
    status: string;
    statusKind: 'info' | 'success' | 'error' | 'warning';
    onRefresh: () => void;
    onUpdate: () => void;
    onLaunch: (mission: string) => void;
  }

  let { missions, loading, status, statusKind, onRefresh, onUpdate, onLaunch }: Props = $props();

  let selectedMission = $state<string | null>(null);

  // ── Helpers ────────────────────────────────────────────────────────────────

  /** Extract a human-readable map name from the mission filename. */
  function mapName(mission: string): string {
    // Missions are typically "MapName.MissionType" e.g. "chernarusplus.DayZCommunityOfflineMode"
    const base = mission.split('.')[0] ?? mission;
    // CamelCase → words, replace underscores/hyphens
    return base
      .replace(/([a-z])([A-Z])/g, '$1 $2')
      .replace(/[_-]+/g, ' ')
      .replace(/\b\w/g, c => c.toUpperCase())
      .trim() || mission;
  }

  /** Extract a short mission type tag from the filename suffix. */
  function missionTag(mission: string): string {
    const parts = mission.split('.');
    if (parts.length < 2) return 'MISSION';
    const tag = parts.slice(1).join('.').toLowerCase();
    if (tag.includes('offline')) return 'OFFLINE';
    if (tag.includes('coop'))    return 'COOP';
    if (tag.includes('pvp'))     return 'PVP';
    if (tag.includes('surv'))    return 'SURVIVAL';
    return parts[parts.length - 1]?.toUpperCase().slice(0, 8) ?? 'MISSION';
  }

  /** Icon per map name keyword. */
  function mapIcon(mission: string): string {
    const n = mission.toLowerCase();
    if (n.includes('chernarus'))  return 'ph:map-trifold';
    if (n.includes('livonia'))    return 'ph:tree-evergreen';
    if (n.includes('namalsk'))    return 'ph:snowflake';
    if (n.includes('takistan'))   return 'ph:mountains';
    if (n.includes('esseker'))    return 'ph:factory';
    if (n.includes('deer'))       return 'ph:island';
    return 'ph:map-pin';
  }

  const statusColors: Record<string, string> = {
    success: 'text-success bg-success/10 border-success/25',
    error:   'text-error   bg-error/10   border-error/25',
    warning: 'text-warning bg-warning/10 border-warning/25',
    info:    'text-info    bg-info/10    border-info/25',
  };
</script>

<div class="flex h-full overflow-hidden">

  <!-- ── Left: mission list ─────────────────────────────────────────────────── -->
  <div class="flex flex-col w-72 flex-shrink-0 border-r border-base-300 bg-base-100 overflow-hidden">

    <!-- Header -->
    <div class="flex items-center gap-2 px-3 py-2 bg-base-200 border-b border-base-300 flex-shrink-0">
      <Icon icon="ph:game-controller" class="size-3.5 text-primary" />
      <span class="text-xs font-semibold flex-1">Offline Missions</span>
      {#if missions.length > 0}
        <span class="text-xs text-base-content/30">{missions.length}</span>
      {/if}
      <button
        class="btn btn-ghost btn-xs p-1"
        onclick={onRefresh}
        disabled={loading}
        title="Refresh mission list"
      >
        <Icon icon="ph:arrows-clockwise" class="size-3.5" />
      </button>
    </div>

    <!-- Status bar -->
    {#if status}
      <div class="flex items-start gap-2 mx-3 mt-2.5 mb-0.5 px-2.5 py-2 rounded-lg border text-xs flex-shrink-0
                  {statusColors[statusKind] ?? statusColors.info}">
        <Icon
          icon={statusKind === 'success' ? 'ph:check-circle'
              : statusKind === 'error'   ? 'ph:warning-circle'
              : statusKind === 'warning' ? 'ph:warning'
              :                            'ph:info'}
          class="size-3.5 shrink-0 mt-0.5"
        />
        <span class="leading-snug">{status}</span>
      </div>
    {/if}

    <!-- Mission cards -->
    {#if loading && missions.length === 0}
      <div class="flex-1 p-3 space-y-2">
        {#each [1,2,3] as _}
          <div class="rounded-lg bg-base-200 animate-pulse h-16"></div>
        {/each}
      </div>
    {:else if missions.length === 0}
      <div class="flex flex-col items-center justify-center flex-1 gap-3 px-5 text-center">
        <div class="size-12 rounded-full bg-base-200 flex items-center justify-center">
          <Icon icon="ph:game-controller" class="size-6 text-base-content/20" />
        </div>
        <div>
          <p class="text-sm font-medium text-base-content/60">No missions installed</p>
          <p class="text-xs text-base-content/35 mt-1 leading-relaxed">
            Click <span class="font-semibold text-primary">Install</span> to download<br>DayZ Community Offline Mode
          </p>
        </div>
      </div>
    {:else}
      <div class="flex-1 overflow-y-auto p-2 space-y-1">
        {#each missions as mission}
          {@const isSel = selectedMission === mission}
          <button
            class="w-full text-left rounded-lg px-3 py-2.5 transition-colors border
                   {isSel
                     ? 'bg-primary/10 border-primary/30'
                     : 'hover:bg-base-200/70 border-transparent hover:border-base-300/50'}"
            onclick={() => (selectedMission = mission)}
            ondblclick={() => onLaunch(mission)}
          >
            <div class="flex items-center gap-2.5">
              <div class="size-8 rounded-md flex items-center justify-center flex-shrink-0
                          {isSel ? 'bg-primary/15 text-primary' : 'bg-base-200 text-base-content/35'}">
                <Icon icon={mapIcon(mission)} class="size-4" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-xs font-semibold truncate text-base-content/90 leading-tight">
                  {mapName(mission)}
                </p>
                <p class="text-base-content/35 font-mono truncate mt-0.5" style="font-size:9px;">
                  {mission}
                </p>
              </div>
              <span class="text-primary/60 font-semibold shrink-0" style="font-size:8px; letter-spacing:0.06em;">
                {missionTag(mission)}
              </span>
            </div>
          </button>
        {/each}
      </div>
    {/if}

    <!-- Install / Update button -->
    <div class="px-3 py-2.5 border-t border-base-300 flex-shrink-0">
      <button
        class="btn btn-primary btn-sm w-full gap-2"
        onclick={onUpdate}
        disabled={loading}
      >
        {#if loading}
          <span class="loading loading-spinner loading-xs"></span>
          Installing…
        {:else}
          <Icon icon="ph:download-simple" class="size-3.5" />
          Install / Update
        {/if}
      </button>
    </div>
  </div>

  <!-- ── Right: detail / launch pane ──────────────────────────────────────── -->
  <div class="flex-1 flex flex-col items-center justify-center overflow-hidden bg-base-100">
    {#if selectedMission}
      {@const icon = mapIcon(selectedMission)}
      {@const name = mapName(selectedMission)}
      {@const tag  = missionTag(selectedMission)}

      <div class="flex flex-col items-center gap-6 max-w-xs w-full px-8">

        <!-- Map icon large -->
        <div class="size-24 rounded-2xl bg-primary/10 border border-primary/20 flex items-center justify-center">
          <Icon icon={icon} class="size-12 text-primary/70" />
        </div>

        <!-- Info -->
        <div class="text-center">
          <h2 class="text-xl font-bold text-base-content leading-tight">{name}</h2>
          <span class="inline-block mt-2 px-2 py-0.5 rounded text-primary/70 bg-primary/10 border border-primary/20 font-semibold" style="font-size:10px; letter-spacing:0.08em;">
            {tag}
          </span>
          <p class="text-xs text-base-content/35 font-mono mt-3 break-all">{selectedMission}</p>
        </div>

        <!-- Launch button -->
        <button
          class="btn btn-primary btn-lg w-full gap-2 shadow-lg"
          onclick={() => onLaunch(selectedMission!)}
        >
          <Icon icon="ph:play" class="size-5" />
          Launch Mission
        </button>

        <p class="text-xs text-base-content/30">Double-click a mission in the list to launch directly</p>
      </div>

    {:else}
      <!-- Nothing selected -->
      <div class="flex flex-col items-center gap-3 text-center px-8">
        <div class="size-16 rounded-2xl bg-base-200 flex items-center justify-center">
          <Icon icon="ph:cursor-click" class="size-8 text-base-content/20" />
        </div>
        <div>
          <p class="text-sm font-medium text-base-content/50">Select a mission to launch</p>
          <p class="text-xs text-base-content/30 mt-1">or double-click to launch immediately</p>
        </div>
      </div>
    {/if}
  </div>

</div>
