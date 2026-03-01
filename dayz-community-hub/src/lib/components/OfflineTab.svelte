<script lang="ts">
  import Icon from '@iconify/svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import * as m from '$lib/paraglide/messages.js';

  const REPO_URL = 'https://github.com/Arkensor/DayZCommunityOfflineMode';

  interface Props {
    missions: string[];
    loading: boolean;
    status: string;
    statusKind: 'info' | 'success' | 'error' | 'warning';
    onRefresh: () => void;
    onUpdate: () => void;
    onLaunch: (mission: string) => void;
    onRemoveOfflineMode: () => void;
    onRemoveMission: (mission: string) => void;
    onClearSaves: () => void;
    onOpenMissionDir: (mission: string) => void;
    onOpenMissionsDir: () => void;
  }

  let {
    missions,
    loading,
    status,
    statusKind,
    onRefresh,
    onUpdate,
    onLaunch,
    onRemoveOfflineMode,
    onRemoveMission,
    onClearSaves,
    onOpenMissionDir,
    onOpenMissionsDir,
  }: Props = $props();

  let selectedMission = $state<string | null>(null);

  // ── Helpers ────────────────────────────────────────────────────────────────

  /** Extract a human-readable map name from the mission filename. */
  function mapName(mission: string): string {
    // Missions are typically "MapName.MissionType" e.g. "chernarusplus.DayZCommunityOfflineMode"
    const base = mission.split('.')[0] ?? mission;
    // CamelCase → words, replace underscores/hyphens
    return (
      base
        .replace(/([a-z])([A-Z])/g, '$1 $2')
        .replace(/[_-]+/g, ' ')
        .replace(/\b\w/g, (c) => c.toUpperCase())
        .trim() || mission
    );
  }

  /** Extract a short mission type tag from the filename suffix. */
  function missionTag(mission: string): string {
    const parts = mission.split('.');
    if (parts.length < 2) return m.mission_tag();
    const tag = parts.slice(1).join('.').toLowerCase();
    if (tag.includes('offline')) return m.mission_offline();
    if (tag.includes('coop')) return m.mission_coop();
    if (tag.includes('pvp')) return m.mission_pvp();
    if (tag.includes('surv')) return m.mission_survival();
    return parts[parts.length - 1]?.toUpperCase().slice(0, 8) ?? m.mission_tag();
  }

  /** Icon per map name keyword. */
  function mapIcon(mission: string): string {
    const n = mission.toLowerCase();
    if (n.includes('chernarus')) return 'ph:map-trifold';
    if (n.includes('livonia')) return 'ph:tree-evergreen';
    if (n.includes('namalsk')) return 'ph:snowflake';
    if (n.includes('takistan')) return 'ph:mountains';
    if (n.includes('esseker')) return 'ph:factory';
    if (n.includes('deer')) return 'ph:island';
    return 'ph:map-pin';
  }

  const statusColors: Record<string, string> = {
    success: 'text-success bg-success/10 border-success/25',
    error: 'text-error   bg-error/10   border-error/25',
    warning: 'text-warning bg-warning/10 border-warning/25',
    info: 'text-info    bg-info/10    border-info/25',
  };
</script>

<div class="flex h-full overflow-hidden">
  <!-- ── Left: mission list ─────────────────────────────────────────────────── -->
  <div class="flex flex-col flex-1 min-w-0 border-r border-base-300 bg-base-100 overflow-hidden">
    <!-- Header -->
    <div class="flex items-center gap-2 px-3 py-2 bg-base-200 border-b border-base-300 flex-shrink-0">
      <Icon icon="ph:game-controller" class="size-3.5 text-primary" />
      <span class="text-xs font-semibold flex-1">{m.offline_title()}</span>
      {#if missions.length > 0}
        <span class="text-xs text-base-content/30">{missions.length}</span>
      {/if}
      <button class="btn btn-ghost btn-xs p-1" onclick={onRefresh} disabled={loading} title={m.offline_refresh_title()}>
        <Icon icon="ph:arrows-clockwise" class="size-3.5" />
      </button>
    </div>

    <!-- Status bar -->
    {#if status}
      <div
        class="flex items-start gap-2 mx-3 mt-2.5 mb-0.5 px-2.5 py-2 rounded-lg border text-xs flex-shrink-0
                  {statusColors[statusKind] ?? statusColors.info}"
      >
        <Icon
          icon={statusKind === 'success'
            ? 'ph:check-circle'
            : statusKind === 'error'
              ? 'ph:warning-circle'
              : statusKind === 'warning'
                ? 'ph:warning'
                : 'ph:info'}
          class="size-3.5 shrink-0 mt-0.5"
        />
        <span class="leading-snug">{status}</span>
      </div>
    {/if}

    <!-- Mission cards -->
    {#if loading && missions.length === 0}
      <div class="flex-1 p-3 space-y-2">
        {#each [1, 2, 3] as _}
          <div class="rounded-lg bg-base-200 animate-pulse h-16"></div>
        {/each}
      </div>
    {:else if missions.length === 0}
      <div class="flex flex-col items-center justify-center flex-1 gap-3 px-5 text-center">
        <div class="size-12 rounded-full bg-base-200 flex items-center justify-center">
          <Icon icon="ph:game-controller" class="size-6 text-base-content/20" />
        </div>
        <div>
          <p class="text-sm font-medium text-base-content/60">{m.offline_no_missions()}</p>
          <p class="text-xs text-base-content/35 mt-1 leading-relaxed">
            {m.offline_no_missions_hint({ installButton: m.offline_install_button() })}
          </p>
        </div>
      </div>
    {:else}
      <div class="flex-1 overflow-y-auto p-2 space-y-1">
        {#each missions as mission}
          {@const isSel = selectedMission === mission}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="group w-full text-left rounded-lg px-3 py-2.5 transition-colors border cursor-pointer
                   {isSel
              ? 'bg-primary/10 border-primary/30'
              : 'hover:bg-base-200/70 border-transparent hover:border-base-300/50'}"
            onclick={() => (selectedMission = mission)}
            ondblclick={() => onLaunch(mission)}
            onkeydown={(e) => {
              if (e.key === 'Enter') onLaunch(mission);
              if (e.key === ' ') {
                e.preventDefault();
                selectedMission = mission;
              }
            }}
            role="button"
            tabindex="0"
          >
            <div class="flex items-center gap-2.5">
              <div
                class="size-8 rounded-md flex items-center justify-center flex-shrink-0
                          {isSel ? 'bg-primary/15 text-primary' : 'bg-base-200 text-base-content/35'}"
              >
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
              <span class="text-primary/60 font-semibold shrink-0 mr-1" style="font-size:8px; letter-spacing:0.06em;">
                {missionTag(mission)}
              </span>
              <!-- Per-mission actions -->
              <div class="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
                <button
                  type="button"
                  class="btn btn-ghost btn-xs p-1 text-base-content/40 hover:text-primary"
                  onclick={(e) => {
                    e.stopPropagation();
                    onOpenMissionDir(mission);
                  }}
                  title={m.offline_open_folder()}
                >
                  <Icon icon="ph:folder-open" class="size-3.5" />
                </button>
                <button
                  type="button"
                  class="btn btn-ghost btn-xs p-1 text-base-content/40 hover:text-error"
                  onclick={(e) => {
                    e.stopPropagation();
                    onRemoveMission(mission);
                  }}
                  title={m.offline_remove_mission()}
                >
                  <Icon icon="ph:trash" class="size-3.5" />
                </button>
              </div>
            </div>
          </div>
        {/each}
      </div>
    {/if}

    <!-- Install / Update button -->
    <div class="px-3 pt-2.5 border-t border-base-300 flex-shrink-0">
      <button class="btn btn-primary btn-sm w-full gap-2" onclick={onUpdate} disabled={loading}>
        {#if loading}
          <span class="loading loading-spinner loading-xs"></span>
          {m.offline_installing()}
        {:else}
          <Icon icon="ph:download-simple" class="size-3.5" />
          {m.offline_install_update()}
        {/if}
      </button>
    </div>

    <!-- Actions -->
    <div class="px-3 pb-2.5 pt-2 flex-shrink-0 flex gap-1.5">
      <button
        class="btn btn-ghost btn-xs flex-1 gap-1 text-base-content/50 hover:text-primary hover:bg-primary/10"
        onclick={onOpenMissionsDir}
        title={m.offline_explore_title()}
      >
        <Icon icon="ph:folder-open" class="size-3.5" />
        {m.offline_explore()}
      </button>
      <button
        class="btn btn-ghost btn-xs flex-1 gap-1 text-warning/70 hover:text-warning hover:bg-warning/10"
        onclick={onClearSaves}
        disabled={loading || missions.length === 0}
        title={m.offline_clear_saves_title()}
      >
        <Icon icon="ph:eraser" class="size-3.5" />
        {m.offline_clear_saves()}
      </button>
      <button
        class="btn btn-ghost btn-xs flex-1 gap-1 text-error/60 hover:text-error hover:bg-error/10"
        onclick={onRemoveOfflineMode}
        disabled={loading || missions.length === 0}
        title={m.offline_remove_all_title()}
      >
        <Icon icon="ph:trash" class="size-3.5" />
        {m.offline_remove_all()}
      </button>
    </div>
  </div>

  <!-- ── Right: detail / launch pane ──────────────────────────────────────── -->
  <div
    class="w-72 flex-shrink-0 flex flex-col items-center justify-center overflow-hidden bg-base-100 border-l border-base-300"
  >
    {#if selectedMission}
      {@const icon = mapIcon(selectedMission)}
      {@const name = mapName(selectedMission)}
      {@const tag = missionTag(selectedMission)}

      <div class="flex flex-col items-center gap-4 w-full px-5">
        <!-- Map icon -->
        <div class="size-16 rounded-xl bg-primary/10 border border-primary/20 flex items-center justify-center">
          <Icon {icon} class="size-8 text-primary/70" />
        </div>

        <!-- Info -->
        <div class="text-center w-full">
          <h2 class="text-base font-bold text-base-content leading-tight truncate">{name}</h2>
          <span
            class="inline-block mt-1.5 px-2 py-0.5 rounded text-primary/70 bg-primary/10 border border-primary/20 font-semibold"
            style="font-size:9px; letter-spacing:0.08em;"
          >
            {tag}
          </span>
          <p class="text-xs text-base-content/35 font-mono mt-2 break-all line-clamp-2">{selectedMission}</p>
        </div>

        <!-- Launch button -->
        <button class="btn btn-primary btn-sm w-full gap-2" onclick={() => onLaunch(selectedMission!)}>
          <Icon icon="ph:play" class="size-4" />
          {m.offline_launch()}
        </button>

        <p class="text-xs text-base-content/30 text-center">{m.offline_doubleclick_hint()}</p>

        <!-- Repo link -->
        <button
          class="flex items-center gap-1.5 text-xs text-base-content/30 hover:text-base-content/60 transition-colors"
          onclick={() => openUrl(REPO_URL)}
          title={REPO_URL}
        >
          <Icon icon="mdi:github" class="size-3" />
          <span style="font-size:10px;">DayZCommunityOfflineMode</span>
          <Icon icon="ph:arrow-square-out" class="size-2.5" />
        </button>
      </div>
    {:else}
      <!-- Nothing selected -->
      <div class="flex flex-col items-center gap-3 text-center px-5">
        <div class="size-12 rounded-xl bg-base-200 flex items-center justify-center">
          <Icon icon="ph:cursor-click" class="size-6 text-base-content/20" />
        </div>
        <div>
          <p class="text-sm font-medium text-base-content/50">{m.offline_select_mission()}</p>
          <p class="text-xs text-base-content/30 mt-0.5">{m.offline_select_hint()}</p>
        </div>

        <!-- Repo link -->
        <button
          class="flex items-center gap-1.5 text-xs text-base-content/35 hover:text-base-content/60 transition-colors border border-base-300/50 hover:border-base-300 rounded-lg px-2.5 py-1.5"
          onclick={() => openUrl(REPO_URL)}
          title={REPO_URL}
        >
          <Icon icon="mdi:github" class="size-3.5 shrink-0" />
          <span class="text-left" style="font-size:10px;">
            <span class="font-medium text-base-content/50">DayZCommunityOfflineMode</span>
          </span>
          <Icon icon="ph:arrow-square-out" class="size-2.5 shrink-0" />
        </button>
      </div>
    {/if}
  </div>
</div>
