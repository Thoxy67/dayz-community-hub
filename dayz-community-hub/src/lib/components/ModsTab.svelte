<script lang="ts">
  import type { InstalledModDto } from '$lib/types';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import Icon from '@iconify/svelte';

  let copiedKey = $state('');
  async function copyText(key: string, text: string) {
    await writeText(text);
    copiedKey = key;
    setTimeout(() => { if (copiedKey === key) copiedKey = ''; }, 1500);
  }

  interface Props {
    mods: InstalledModDto[];
    loading: boolean;
    checking: boolean;
    staleCount: number;
    onRefresh: () => void;
    onCheckUpdates: () => void;
    onDelete: (mod: InstalledModDto) => void;
    onToggleManaged: (mod: InstalledModDto) => Promise<void>;
    onUpdate: (mod: InstalledModDto) => void;
    onUpdateAll: () => void;
    onUpdateStale: () => void;
    onCleanup: () => void;
  }

  let {
    mods, loading, checking, staleCount,
    onRefresh, onCheckUpdates, onDelete, onToggleManaged,
    onUpdate, onUpdateAll, onUpdateStale, onCleanup,
  }: Props = $props();

  // Track which mod IDs are currently being toggled so we can show a per-row spinner.
  let togglingIds = $state(new Set<number>());

  async function handleToggleManaged(mod: InstalledModDto) {
    togglingIds = new Set([...togglingIds, mod.id]);
    try {
      await onToggleManaged(mod);
    } finally {
      togglingIds = new Set([...togglingIds].filter((id) => id !== mod.id));
    }
  }

  let totalSize = $derived(mods.reduce((acc, m) => acc + m.size, 0));

  function formatSize(bytes: number): string {
    const mb = bytes / 1024 / 1024;
    if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
    if (mb >= 1) return `${mb.toFixed(1)} MB`;
    return `${Math.round(bytes / 1024)} KB`;
  }

  function formatDate(ts: number): string {
    if (!ts) return '—';
    return new Date(ts * 1000).toLocaleDateString(undefined, {
      year: 'numeric', month: 'short', day: 'numeric',
    });
  }
</script>

<div class="flex flex-col h-full overflow-hidden">

  <!-- ── Toolbar ─────────────────────────────────────────────────────────── -->
  <div class="flex items-center gap-2 px-3 py-2 bg-base-200 border-b border-base-300 flex-shrink-0">

    <!-- Stats -->
    <div class="flex items-center gap-3 text-xs text-base-content/50">
      <span class="flex items-center gap-1">
        <Icon icon="mdi:puzzle-outline" class="size-3.5 text-base-content/35" />
        <span class="font-medium text-base-content/80">{mods.length}</span> mods
      </span>
      <span class="text-base-content/25">·</span>
      <span>{formatSize(totalSize)}</span>
      {#if staleCount > 0}
        <span class="text-base-content/25">·</span>
        <span class="flex items-center gap-1 text-warning font-medium">
          <Icon icon="ph:arrow-circle-up" class="size-3.5" />
          {staleCount} update{staleCount > 1 ? 's' : ''} available
        </span>
      {:else if !checking && mods.length > 0}
        <span class="text-base-content/25">·</span>
        <span class="flex items-center gap-1 text-success/70">
          <Icon icon="ph:check-circle" class="size-3.5" />
          All up to date
        </span>
      {/if}
    </div>

    <div class="ml-auto flex items-center gap-1">

      <!-- Check for updates -->
      <button
        class="btn btn-ghost btn-xs gap-1.5"
        onclick={onCheckUpdates}
        disabled={checking || loading || mods.length === 0}
        title="Check Steam Workshop for updates"
      >
        {#if checking}
          <span class="loading loading-spinner loading-xs"></span>
          Checking…
        {:else}
          <Icon icon="ph:cloud-arrow-down" class="size-3.5" />
          Check updates
        {/if}
      </button>

      <!-- Update stale / Update all -->
      {#if staleCount > 0}
        <button
          class="btn btn-warning btn-xs gap-1.5"
          onclick={onUpdateStale}
          disabled={loading}
          title="Update only the {staleCount} mod{staleCount > 1 ? 's' : ''} with available updates"
        >
          <Icon icon="ph:arrow-circle-up" class="size-3.5" />
          Update {staleCount}
        </button>
        <button
          class="btn btn-ghost btn-xs gap-1"
          onclick={onUpdateAll}
          disabled={loading}
          title="Force re-validate all mods via steamcmd"
        >
          <Icon icon="ph:arrows-clockwise" class="size-3" />
          All
        </button>
      {:else}
        <button
          class="btn btn-ghost btn-xs gap-1.5"
          onclick={onUpdateAll}
          disabled={loading || mods.length === 0}
          title="Force re-validate all mods via steamcmd"
        >
          <Icon icon="ph:arrows-clockwise" class="size-3.5" />
          Update all
        </button>
      {/if}

      <div class="w-px h-4 bg-base-300 mx-0.5"></div>

      <!-- Refresh -->
      <button
        class="btn btn-ghost btn-xs"
        onclick={onRefresh}
        disabled={loading}
        title="Refresh mod list"
      >
        {#if loading}
          <span class="loading loading-spinner loading-xs"></span>
        {:else}
          <Icon icon="ph:arrows-clockwise" class="size-3.5" />
        {/if}
      </button>

      <!-- Cleanup -->
      <button
        class="btn btn-ghost btn-xs text-error/60 hover:text-error"
        onclick={onCleanup}
        disabled={mods.length === 0}
        title="Remove all managed mods and symlinks"
      >
        <Icon icon="ph:trash" class="size-3.5" />
      </button>
    </div>
  </div>

  <!-- ── Body ───────────────────────────────────────────────────────────── -->
  {#if loading && mods.length === 0}
    <div class="flex items-center justify-center h-full gap-2 text-base-content/50">
      <span class="loading loading-spinner loading-md"></span>
      <span class="text-sm">Loading mods…</span>
    </div>
  {:else if mods.length === 0}
    <div class="flex flex-col items-center justify-center h-full gap-2 text-base-content/30">
      <Icon icon="mdi:puzzle-outline" class="size-10 opacity-30" />
      <span class="text-sm">No mods installed</span>
      <span class="text-xs">Connect to a modded server to install mods</span>
    </div>
  {:else}
    <div class="overflow-auto flex-1">
      <table class="w-full text-xs" style="table-layout: fixed; border-collapse: collapse;">
        <thead class="sticky top-0 z-10">
          <tr class="bg-base-200/95 backdrop-blur-sm text-base-content/50 uppercase tracking-wider border-b border-base-300 select-none" style="font-size:10px;">
            <th class="px-3 py-2 text-left font-medium">Name</th>
            <th class="w-36 px-3 py-2 font-medium text-left">Workshop ID</th>
            <th class="w-20 px-3 py-2 font-medium text-right">Size</th>
            <th class="w-28 px-3 py-2 font-medium text-left">Local</th>
            <th class="w-28 px-3 py-2 font-medium text-left">Remote</th>
            <th class="w-16 px-3 py-2 font-medium text-center" title="UPDATE = new version on Workshop; OK = up to date; MANAGED = tracked but not yet checked for updates">Status</th>
            <th class="w-24 px-3 py-2"></th>
          </tr>
        </thead>
        <tbody>
          {#each mods as mod}
            {@const stale = mod.update_available}
            <tr class="group/row border-b border-base-300/40 transition-colors hover:bg-base-200/60
                       {stale ? 'bg-warning/5' : ''}">

              <!-- Name -->
              <td class="px-3 py-2 max-w-0">
                <div class="flex items-center gap-2 min-w-0">
                  {#if stale}
                    <span title="Update available">
                      <Icon icon="ph:arrow-circle-up" class="size-3.5 text-warning shrink-0" />
                    </span>
                  {:else if mod.remote_updated !== null}
                    <Icon icon="ph:check-circle" class="size-3.5 text-success/50 shrink-0" />
                  {:else}
                    <Icon icon="mdi:puzzle-outline" class="size-3.5 text-base-content/20 shrink-0" />
                  {/if}
                  <button
                    class="truncate font-medium text-base-content/90 hover:text-base-content transition-colors text-left group/name
                           {stale ? 'text-base-content' : ''}"
                    title="Copy mod name"
                    onclick={() => copyText(`name-${mod.id}`, mod.name)}
                  >
                    {#if copiedKey === `name-${mod.id}`}
                      <span class="text-success text-xs font-normal">Copied!</span>
                    {:else}
                      {mod.name}
                    {/if}
                  </button>
                </div>
              </td>

              <!-- Workshop ID -->
              <td class="px-3 py-2">
                <div class="flex items-center gap-1 group/ws">
                  <button
                    class="font-mono text-base-content/45 hover:text-primary transition-colors flex items-center gap-1"
                    onclick={() => openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.id}`)}
                    title="Open on Steam Workshop"
                  >
                    {mod.id}
                    <Icon icon="mdi:steam" class="size-3 opacity-0 group-hover/ws:opacity-100 transition-opacity" />
                  </button>
                  <button
                    class="opacity-0 group-hover/ws:opacity-100 transition-opacity text-base-content/40 hover:text-base-content/80"
                    title="Copy workshop ID"
                    onclick={() => copyText(`ws-${mod.id}`, String(mod.id))}
                  >
                    {#if copiedKey === `ws-${mod.id}`}
                      <Icon icon="ph:check" class="size-3 text-success" />
                    {:else}
                      <Icon icon="ph:copy" class="size-3" />
                    {/if}
                  </button>
                </div>
              </td>

              <!-- Size -->
              <td class="px-3 py-2 text-right tabular-nums text-base-content/50">{mod.size_human}</td>

              <!-- Local updated -->
              <td class="px-3 py-2 text-base-content/50">{formatDate(mod.local_updated)}</td>

              <!-- Remote updated -->
              <td class="px-3 py-2">
                {#if checking}
                  <span class="text-base-content/25">…</span>
                {:else if mod.remote_updated}
                  <span class="{stale ? 'text-warning font-medium' : 'text-base-content/50'}">
                    {formatDate(mod.remote_updated)}
                  </span>
                {:else}
                  <span class="text-base-content/25">—</span>
                {/if}
              </td>

              <!-- Status badge -->
              <td class="px-3 py-2 text-center">
                {#if stale}
                  <span class="inline-flex items-center gap-1 text-warning font-semibold rounded px-1.5 py-0.5 bg-warning/15" style="font-size:9px;">
                    UPDATE
                  </span>
                {:else if mod.remote_updated !== null}
                  <span class="inline-flex items-center gap-1 text-success/70 rounded px-1.5 py-0.5 bg-success/10" style="font-size:9px;">
                    OK
                  </span>
                {:else if mod.managed}
                  <span class="inline-flex items-center gap-1 text-base-content/40 rounded px-1.5 py-0.5 bg-base-300/50" style="font-size:9px;">
                    MANAGED
                  </span>
                {:else}
                  <span class="text-base-content/20" style="font-size:9px;">—</span>
                {/if}
              </td>

              <!-- Actions -->
              <td class="px-2 py-2">
                <div class="flex gap-1 items-center justify-end">
                  <!-- Update button — amber when stale -->
                  <span title={stale ? 'Update available — click to update' : 'Force re-validate via steamcmd'}>
                    <button
                      class="size-6 rounded flex items-center justify-center transition-colors
                             {stale
                               ? 'text-warning hover:bg-warning/15'
                               : 'text-base-content/35 hover:bg-base-300 hover:text-base-content/70'}"
                      onclick={() => onUpdate(mod)}
                    >
                      <Icon icon="ph:arrows-clockwise" class="size-3.5" />
                    </button>
                  </span>
                  <!-- Toggle managed -->
                  <span title={togglingIds.has(mod.id) ? 'Updating…' : mod.managed
                    ? 'Managed: this mod is tracked and included when connecting to modded servers. Click to unmanage.'
                    : 'Unmanaged: this mod is installed but not tracked. Click to mark as managed so it is included in server connections.'
                  }>
                    <button
                      class="size-6 rounded flex items-center justify-center transition-colors
                             {mod.managed
                               ? 'text-success/60 hover:bg-success/10'
                               : 'text-base-content/35 hover:bg-base-300 hover:text-base-content/70'}
                             {togglingIds.has(mod.id) ? 'opacity-60 pointer-events-none' : ''}"
                      onclick={() => handleToggleManaged(mod)}
                      disabled={togglingIds.has(mod.id)}
                    >
                      {#if togglingIds.has(mod.id)}
                        <span class="loading loading-spinner loading-xs"></span>
                      {:else}
                        <Icon icon={mod.managed ? 'ph:check-square' : 'ph:square'} class="size-3.5" />
                      {/if}
                    </button>
                  </span>
                  <!-- Delete -->
                  <span title="Delete mod">
                    <button
                      class="size-6 rounded flex items-center justify-center text-base-content/35 hover:bg-error/10 hover:text-error transition-colors"
                      onclick={() => onDelete(mod)}
                    >
                      <Icon icon="ph:trash" class="size-3.5" />
                    </button>
                  </span>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
