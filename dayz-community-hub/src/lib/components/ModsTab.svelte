<script lang="ts">
  import type { InstalledModDto } from '$lib/types';
  import { sortIcon as _sortIcon } from '$lib/utils';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import Icon from '@iconify/svelte';
  import { onMount } from 'svelte';

  let copiedKey = $state('');
  async function copyText(key: string, text: string) {
    await writeText(text);
    copiedKey = key;
    setTimeout(() => {
      if (copiedKey === key) copiedKey = '';
    }, 1500);
  }

  interface Props {
    mods: InstalledModDto[];
    loading: boolean;
    checking: boolean;
    staleCount: number;
    steamApiKey: string;
    onRefresh: () => void;
    onCheckUpdates: () => void;
    onDelete: (mod: InstalledModDto) => void;
    onToggleManaged: (mod: InstalledModDto) => Promise<void>;
    onUpdate: (mod: InstalledModDto) => void;
    onUpdateAll: () => void;
    onUpdateStale: () => void;
    onCleanup: () => void;
    onOpenWorkshopDir: () => void;
    onOpenModDir: (mod: InstalledModDto) => void;
    onDeleteSelected: (ids: number[]) => void;
    onUpdateSelected: (ids: number[]) => void;
    onInstallMods: (workshopIds: number[]) => void;
  }

  let {
    mods,
    loading,
    checking,
    staleCount,
    steamApiKey,
    onRefresh,
    onCheckUpdates,
    onDelete,
    onToggleManaged,
    onUpdate,
    onUpdateAll,
    onUpdateStale,
    onCleanup,
    onOpenWorkshopDir,
    onOpenModDir,
    onDeleteSelected,
    onUpdateSelected,
    onInstallMods,
  }: Props = $props();

  // ── Manual install modal ───────────────────────────────────────────────────
  let showInstallModal = $state(false);
  let installInput = $state('');
  let installError = $state('');

  function openInstallModal() {
    installInput = '';
    installError = '';
    showInstallModal = true;
  }

  function closeInstallModal() {
    showInstallModal = false;
  }

  /** Parse one line: raw ID or Workshop URL → number, or null if invalid. */
  function parseOne(raw: string): number | null {
    const trimmed = raw.trim();
    if (!trimmed) return null;
    const urlMatch = trimmed.match(/[?&]id=(\d+)/i);
    const idStr = urlMatch ? urlMatch[1] : trimmed.replace(/\D/g, '');
    const id = parseInt(idStr, 10);
    return id > 0 ? id : null;
  }

  /** Parsed, deduplicated IDs from the textarea (live derived). */
  let parsedIds = $derived(
    (() => {
      const seen = new Set<number>();
      for (const line of installInput.split(/[\n,]+/)) {
        const id = parseOne(line);
        if (id !== null) seen.add(id);
      }
      return [...seen];
    })(),
  );

  function submitInstall() {
    if (parsedIds.length === 0) {
      installError = 'No valid Workshop IDs found.';
      return;
    }
    installError = '';
    closeInstallModal();
    onInstallMods(parsedIds);
  }

  function handleInstallKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') closeInstallModal();
  }

  /** Whether Steam Workshop update checking is available (requires Steam API key). */
  let canCheckUpdates = $derived(!!steamApiKey);

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

  // ── Sorting ──────────────────────────────────────────────────────────────
  type SortCol = 'name' | 'id' | 'size' | 'local' | 'remote';
  let sortCol = $state<SortCol>('name');
  let sortAsc = $state(true);

  function toggleSort(col: SortCol) {
    if (sortCol === col) {
      sortAsc = !sortAsc;
    } else {
      sortCol = col;
      sortAsc = col === 'name'; // text cols default asc; numeric cols default desc
    }
  }

  function sortIcon(col: SortCol) {
    return _sortIcon(col, sortCol, sortAsc);
  }

  let sorted = $derived(
    (() => {
      const arr = mods.slice();
      const dir = sortAsc ? 1 : -1;
      arr.sort((a, b) => {
        switch (sortCol) {
          case 'name':
            return dir * a.name.localeCompare(b.name);
          case 'id':
            return dir * (a.id - b.id);
          case 'size':
            return dir * (a.size - b.size);
          case 'local':
            return dir * (a.local_updated - b.local_updated);
          case 'remote':
            return dir * ((a.remote_updated ?? 0) - (b.remote_updated ?? 0));
          default:
            return 0;
        }
      });
      return arr;
    })(),
  );

  let totalSize = $derived(mods.reduce((acc, m) => acc + m.size, 0));

  // ── Selection ─────────────────────────────────────────────────────────────
  let selectedIds = $state(new Set<number>());

  function toggleSelect(id: number) {
    const next = new Set(selectedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selectedIds = next;
  }

  function toggleSelectAll() {
    if (selectedIds.size === sorted.length) {
      selectedIds = new Set();
    } else {
      selectedIds = new Set(sorted.map((m) => m.id));
    }
  }

  // Clear selection whenever mod list changes (e.g. after delete/refresh)
  $effect(() => {
    const ids = new Set(mods.map((m) => m.id));
    const pruned = new Set([...selectedIds].filter((id) => ids.has(id)));
    if (pruned.size !== selectedIds.size) selectedIds = pruned;
  });

  let selectedMods = $derived(mods.filter((m) => selectedIds.has(m.id)));
  let allSelected = $derived(sorted.length > 0 && selectedIds.size === sorted.length);
  let someSelected = $derived(selectedIds.size > 0 && !allSelected);
  let selectedStaleCount = $derived(selectedMods.filter((m) => m.update_available).length);
  let selectedSize = $derived(selectedMods.reduce((acc, m) => acc + m.size, 0));
  let selectedUnlinkedCount = $derived(selectedMods.filter((m) => !m.managed).length);
  let selectedLinkedCount = $derived(selectedMods.filter((m) => m.managed).length);

  let bulkLinking = $state(false);

  async function bulkToggleManaged(linkMode: boolean) {
    // linkMode=true → link unlinked mods; linkMode=false → unlink linked mods
    const targets = selectedMods.filter((m) => (linkMode ? !m.managed : m.managed));
    if (targets.length === 0) return;
    bulkLinking = true;
    try {
      for (const mod of targets) {
        await onToggleManaged(mod);
      }
    } finally {
      bulkLinking = false;
    }
  }

  function formatSize(bytes: number): string {
    const mb = bytes / 1024 / 1024;
    if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
    if (mb >= 1) return `${mb.toFixed(1)} MB`;
    return `${Math.round(bytes / 1024)} KB`;
  }

  function formatDate(ts: number): string {
    if (!ts) return '—';
    return new Date(ts * 1000).toLocaleString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  }

  // ── Keyboard navigation ───────────────────────────────────────────────────
  let focusedIdx = $state<number>(-1);
  let tableRef = $state<HTMLElement | null>(null);

  // Keep focusedIdx in bounds when the sorted list changes
  $effect(() => {
    if (sorted.length === 0) {
      focusedIdx = -1;
      return;
    }
    if (focusedIdx >= sorted.length) focusedIdx = sorted.length - 1;
  });

  function handleKeydown(e: KeyboardEvent) {
    if (sorted.length === 0) return;
    // Skip when a real input/select/textarea has focus
    const tag = (e.target as HTMLElement)?.tagName;
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
    // Skip global shortcuts (Ctrl+…)
    if (e.ctrlKey || e.metaKey) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusedIdx = focusedIdx < 0 ? 0 : Math.min(focusedIdx + 1, sorted.length - 1);
      scrollRowIntoView(focusedIdx);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusedIdx = focusedIdx < 0 ? 0 : Math.max(focusedIdx - 1, 0);
      scrollRowIntoView(focusedIdx);
    } else if (e.key === ' ') {
      if (focusedIdx >= 0 && focusedIdx < sorted.length) {
        e.preventDefault();
        toggleSelect(sorted[focusedIdx].id);
      }
    } else if (e.key === 'm' || e.key === 'M') {
      if (focusedIdx >= 0 && focusedIdx < sorted.length) {
        handleToggleManaged(sorted[focusedIdx]);
      }
    }
  }

  function scrollRowIntoView(idx: number) {
    if (!tableRef) return;
    const row = tableRef.querySelector<HTMLElement>(`tbody tr:nth-child(${idx + 1})`);
    row?.scrollIntoView({ block: 'nearest' });
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
    return () => window.removeEventListener('keydown', handleKeydown);
  });
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
      <span class="text-secondary/60">{formatSize(totalSize)}</span>
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
      <!-- Manual install -->
      <button
        class="btn btn-ghost btn-xs gap-1.5"
        onclick={openInstallModal}
        title="Install a mod manually by Workshop ID or URL"
      >
        <Icon icon="ph:download-simple" class="size-3.5" />
        Install mod
      </button>

      <div class="w-px h-4 bg-base-300 mx-0.5"></div>

      <!-- Open workshop directory -->
      <button
        class="btn btn-ghost btn-xs gap-1.5"
        onclick={onOpenWorkshopDir}
        disabled={mods.length === 0}
        title="Open Workshop mods directory in file manager"
      >
        <Icon icon="ph:folder-open" class="size-3.5" />
        Open folder
      </button>

      <!-- Check for updates — requires Steam API key -->
      {#if canCheckUpdates}
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
      {/if}

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
      <button class="btn btn-ghost btn-xs" onclick={onRefresh} disabled={loading} title="Refresh mod list">
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
    <div class="overflow-auto flex-1" bind:this={tableRef}>
      <table class="w-full text-xs" style="table-layout: fixed; border-collapse: collapse;">
        <thead class="sticky top-0 z-10">
          <tr
            class="bg-base-200/95 backdrop-blur-sm text-base-content/50 uppercase tracking-wider border-b border-base-300 select-none"
            style="font-size:10px;"
          >
            <!-- Select-all checkbox -->
            <th class="w-8 px-2 py-2 text-center">
              <input
                type="checkbox"
                class="checkbox checkbox-xs"
                checked={allSelected}
                indeterminate={someSelected}
                onchange={toggleSelectAll}
                title={allSelected ? 'Deselect all' : 'Select all'}
              />
            </th>
            <th
              class="px-3 py-2 text-left font-medium cursor-pointer hover:text-base-content transition-colors"
              onclick={() => toggleSort('name')}
            >
              <span class="flex items-center gap-1">Name <Icon icon={sortIcon('name')} class="size-2.5" /></span>
            </th>
            <th
              class="w-36 px-3 py-2 font-medium text-left cursor-pointer hover:text-info transition-colors"
              onclick={() => toggleSort('id')}
            >
              <span class="flex items-center gap-1">Workshop ID <Icon icon={sortIcon('id')} class="size-2.5" /></span>
            </th>
            <th
              class="w-20 px-3 py-2 font-medium text-right cursor-pointer hover:text-secondary transition-colors"
              onclick={() => toggleSort('size')}
            >
              <span class="flex items-center justify-end gap-1"
                >Size <Icon icon={sortIcon('size')} class="size-2.5" /></span
              >
            </th>
            <th
              class="w-40 px-3 py-2 font-medium text-left cursor-pointer hover:text-base-content transition-colors"
              onclick={() => toggleSort('local')}
            >
              <span class="flex items-center gap-1">Local <Icon icon={sortIcon('local')} class="size-2.5" /></span>
            </th>
            {#if canCheckUpdates}
              <th
                class="w-40 px-3 py-2 font-medium text-left cursor-pointer hover:text-base-content transition-colors"
                onclick={() => toggleSort('remote')}
              >
                <span class="flex items-center gap-1">Remote <Icon icon={sortIcon('remote')} class="size-2.5" /></span>
              </th>
            {/if}
            <th
              class="w-16 px-3 py-2 font-medium text-center"
              title="UPDATE = new version on Workshop; OK = up to date; LINKED = symlink active; UNLINKED = no symlink"
              >Status</th
            >
            <th class="w-24 px-3 py-2"></th>
          </tr>
        </thead>
        <tbody>
          {#each sorted as mod, idx}
            {@const stale = mod.update_available}
            {@const selected = selectedIds.has(mod.id)}
            {@const focused = idx === focusedIdx}
            <tr
              class="group/row border-b border-base-300/40 transition-colors hover:bg-base-200/60 cursor-pointer
                     {stale ? 'bg-warning/5' : ''}
                     {selected ? 'bg-primary/8 hover:bg-primary/12' : ''}
                     {focused ? 'outline outline-1 outline-primary/60 outline-offset-[-1px]' : ''}"
              onclick={() => (focusedIdx = idx)}
              ondblclick={() => onOpenModDir(mod)}
            >
              <!-- Checkbox -->
              <td
                class="px-2 py-2 text-center"
                onclick={(e) => {
                  e.stopPropagation();
                  toggleSelect(mod.id);
                }}
              >
                <input
                  type="checkbox"
                  class="checkbox checkbox-xs"
                  checked={selected}
                  onchange={() => toggleSelect(mod.id)}
                  onclick={(e) => e.stopPropagation()}
                />
              </td>

              <!-- Name -->
              <td class="px-3 py-2 max-w-0">
                <div class="flex items-center gap-2 min-w-0">
                  {#if stale}
                    <span title="Update available">
                      <Icon icon="ph:arrow-circle-up" class="size-3.5 text-warning shrink-0" />
                    </span>
                  {:else if mod.remote_updated !== null}
                    <span title="Up to date"
                      ><Icon icon="ph:check-circle" class="size-3.5 text-success/60 shrink-0" /></span
                    >
                  {:else if mod.managed}
                    <span title="Managed"><Icon icon="ph:link-simple" class="size-3.5 text-info/40 shrink-0" /></span>
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
                    class="font-mono text-info/50 hover:text-info transition-colors flex items-center gap-1"
                    onclick={() => openUrl(`https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.id}`)}
                    title="Open on Steam Workshop"
                  >
                    {mod.id}
                    <Icon icon="mdi:steam" class="size-3 opacity-0 group-hover/ws:opacity-100 transition-opacity" />
                  </button>
                  <button
                    class="opacity-0 group-hover/ws:opacity-100 transition-opacity text-base-content/40 hover:text-info"
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
              <td class="px-3 py-2 text-right tabular-nums text-secondary/60">{mod.size_human}</td>

              <!-- Local updated -->
              <td
                class="px-3 py-2 {mod.remote_updated && mod.local_updated < mod.remote_updated
                  ? 'text-base-content/40'
                  : 'text-base-content/70'}">{formatDate(mod.local_updated)}</td
              >

              <!-- Remote updated — only shown when Steam API key is set -->
              {#if canCheckUpdates}
                <td class="px-3 py-2">
                  {#if checking}
                    <span class="text-base-content/25">…</span>
                  {:else if mod.remote_updated}
                    <span
                      class={stale
                        ? 'text-warning font-medium'
                        : mod.local_updated < mod.remote_updated
                          ? 'text-base-content/70'
                          : 'text-base-content/40'}
                    >
                      {formatDate(mod.remote_updated)}
                    </span>
                  {:else}
                    <span class="text-base-content/25">—</span>
                  {/if}
                </td>
              {/if}

              <!-- Status badge -->
              <td class="px-3 py-2 text-center">
                {#if canCheckUpdates && stale}
                  <span
                    class="inline-flex items-center gap-0.5 text-warning font-semibold rounded-md px-1.5 py-0.5 bg-warning/15"
                    style="font-size:9px;"
                  >
                    <Icon icon="ph:arrow-circle-up" class="size-2.5" />
                    UPDATE
                  </span>
                {:else if canCheckUpdates && mod.remote_updated !== null}
                  <span
                    class="inline-flex items-center gap-0.5 text-success/80 font-semibold rounded-md px-1.5 py-0.5 bg-success/10"
                    style="font-size:9px;"
                  >
                    <Icon icon="ph:check-circle" class="size-2.5" />
                    OK
                  </span>
                {:else if mod.managed}
                  <span
                    class="inline-flex items-center gap-0.5 text-info/70 font-semibold rounded-md px-1.5 py-0.5 bg-info/10"
                    style="font-size:9px;"
                  >
                    <Icon icon="ph:link-simple" class="size-2.5" />
                    LINKED
                  </span>
                {:else}
                  <span
                    class="inline-flex items-center gap-0.5 text-base-content/30 font-medium rounded-md px-1.5 py-0.5 bg-base-300/30"
                    style="font-size:9px;"
                  >
                    <Icon icon="ph:link-simple-break" class="size-2.5" />
                    UNLINKED
                  </span>
                {/if}
              </td>

              <!-- Actions -->
              <td class="px-2 py-2">
                <div class="flex gap-1 items-center justify-end">
                  <!-- Update button — amber when stale -->
                  <button
                    class="size-6 rounded flex items-center justify-center transition-colors
                           {stale
                      ? 'text-warning hover:bg-warning/15'
                      : 'text-base-content/35 hover:bg-base-300 hover:text-base-content/70'}"
                    title={stale ? 'Update available — click to update' : 'Force re-validate via steamcmd'}
                    onclick={() => onUpdate(mod)}
                  >
                    <Icon icon="ph:arrows-clockwise" class="size-3.5" />
                  </button>
                  <!-- Toggle managed -->
                  <button
                    class="size-6 rounded flex items-center justify-center transition-colors
                           {mod.managed
                      ? 'text-info/70 hover:bg-info/10'
                      : 'text-base-content/30 hover:bg-base-300 hover:text-base-content/60'}
                           {togglingIds.has(mod.id) ? 'opacity-60 pointer-events-none' : ''}"
                    title={togglingIds.has(mod.id)
                      ? 'Updating…'
                      : mod.managed
                        ? 'Linked: symlink active — this mod is loaded when connecting to modded servers. Click to remove the symlink.'
                        : "Unlinked: no symlink — this mod is installed but won't be loaded. Click to create a symlink so it is included in server connections."}
                    onclick={() => handleToggleManaged(mod)}
                    disabled={togglingIds.has(mod.id)}
                  >
                    {#if togglingIds.has(mod.id)}
                      <span class="loading loading-spinner loading-xs"></span>
                    {:else}
                      <Icon icon={mod.managed ? 'ph:link-simple' : 'ph:link-simple-break'} class="size-3.5" />
                    {/if}
                  </button>
                  <!-- Delete -->
                  <button
                    class="size-6 rounded flex items-center justify-center text-base-content/35 hover:bg-error/10 hover:text-error transition-colors"
                    title="Delete mod"
                    onclick={() => onDelete(mod)}
                  >
                    <Icon icon="ph:trash" class="size-3.5" />
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  <!-- ── Manual install modal ──────────────────────────────────────────── -->
  {#if showInstallModal}
    <!-- Backdrop -->
    <div
      class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
      onclick={closeInstallModal}
      onkeydown={(e) => {
        if (e.key === 'Escape') closeInstallModal();
      }}
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Install mod"
    >
      <!-- Panel -->
      <div
        class="bg-base-100 rounded-2xl border border-base-300 shadow-2xl w-full max-w-md mx-4 overflow-hidden"
        onclick={(e) => e.stopPropagation()}
        role="presentation"
      >
        <!-- Header -->
        <div class="px-5 py-4 bg-base-200 border-b border-base-300 flex items-center gap-3">
          <Icon icon="ph:download-simple" class="size-5 text-primary shrink-0" />
          <div class="flex-1">
            <p class="font-semibold text-sm">Install Mod</p>
            <p class="text-xs text-base-content/50 mt-0.5">Download via Steam Workshop ID or URL</p>
          </div>
          <button
            class="btn btn-ghost btn-xs btn-square text-base-content/40 hover:text-base-content"
            onclick={closeInstallModal}
            title="Close"
          >
            <Icon icon="ph:x" class="size-4" />
          </button>
        </div>

        <!-- Body -->
        <div class="p-5 space-y-4">
          <div class="form-control">
            <label class="label py-0 pb-1.5" for="install-mod-input">
              <span class="label-text text-xs text-base-content/60 flex items-center gap-1.5">
                <Icon icon="mdi:steam" class="size-3.5" />
                Workshop IDs or URLs
              </span>
              <span class="label-text-alt text-base-content/35 text-xs">one per line, or comma-separated</span>
            </label>
            <!-- svelte-ignore a11y_autofocus -->
            <textarea
              id="install-mod-input"
              class="textarea textarea-bordered w-full font-mono text-xs leading-relaxed resize-none h-32
                     {installError ? 'textarea-error' : ''}"
              placeholder={'1559212036\n2116157322\nhttps://steamcommunity.com/sharedfiles/filedetails/?id=1564026768'}
              bind:value={installInput}
              onkeydown={handleInstallKeydown}
              autofocus
            ></textarea>
            {#if installError}
              <p class="label py-0 pt-1">
                <span class="label-text-alt text-error text-xs">{installError}</span>
              </p>
            {/if}
          </div>

          <!-- Live parsed preview -->
          {#if parsedIds.length > 0}
            <div class="rounded-lg bg-base-200 px-3 py-2.5 space-y-1.5">
              <p class="text-xs text-base-content/40 uppercase tracking-wide font-semibold">
                {parsedIds.length} mod{parsedIds.length > 1 ? 's' : ''} queued
              </p>
              <div class="flex flex-wrap gap-1.5 max-h-24 overflow-y-auto">
                {#each parsedIds as id}
                  <span class="font-mono text-xs bg-base-300 text-base-content/70 px-1.5 py-0.5 rounded">
                    {id}
                  </span>
                {/each}
              </div>
            </div>
          {:else if installInput.trim()}
            <p class="text-xs text-error/70">No valid IDs detected.</p>
          {/if}
        </div>

        <!-- Footer -->
        <div class="px-5 py-4 bg-base-200/50 border-t border-base-300 flex gap-2 justify-end">
          <button class="btn btn-ghost btn-sm" onclick={closeInstallModal}>Cancel</button>
          <button class="btn btn-primary btn-sm gap-1.5" onclick={submitInstall} disabled={parsedIds.length === 0}>
            <Icon icon="ph:download-simple" class="size-4" />
            Install {parsedIds.length > 1 ? `${parsedIds.length} mods` : 'mod'}
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- ── Selection footer ───────────────────────────────────────────────── -->
  {#if selectedIds.size > 0}
    <div class="flex items-center gap-3 px-4 py-2.5 bg-base-200 border-t-2 border-primary/40 flex-shrink-0 text-xs">
      <!-- Selection summary -->
      <div class="flex items-center gap-2 text-base-content/70">
        <Icon icon="ph:selection-all" class="size-4 text-primary/70" />
        <span class="font-medium text-base-content/90">{selectedIds.size}</span>
        <span>mod{selectedIds.size > 1 ? 's' : ''} selected</span>
        <span class="text-base-content/30">·</span>
        <span class="text-secondary/60">{formatSize(selectedSize)}</span>
        {#if selectedStaleCount > 0}
          <span class="text-base-content/30">·</span>
          <span class="text-warning flex items-center gap-1">
            <Icon icon="ph:arrow-circle-up" class="size-3.5" />
            {selectedStaleCount} update{selectedStaleCount > 1 ? 's' : ''} available
          </span>
        {/if}
      </div>

      <div class="flex items-center gap-1.5 ml-auto">
        <!-- Update selected (only if any are stale) -->
        {#if selectedStaleCount > 0}
          <button
            class="btn btn-warning btn-xs gap-1.5"
            onclick={() => onUpdateSelected(selectedMods.filter((m) => m.update_available).map((m) => m.id))}
            disabled={loading}
            title="Update {selectedStaleCount} selected mod{selectedStaleCount > 1 ? 's' : ''} with available updates"
          >
            <Icon icon="ph:arrow-circle-up" class="size-3.5" />
            Update {selectedStaleCount} stale
          </button>
        {/if}

        <!-- Update all selected -->
        <button
          class="btn btn-ghost btn-xs gap-1.5"
          onclick={() => onUpdateSelected(selectedMods.map((m) => m.id))}
          disabled={loading}
          title="Force re-validate {selectedIds.size} selected mod{selectedIds.size > 1 ? 's' : ''} via steamcmd"
        >
          <Icon icon="ph:arrows-clockwise" class="size-3.5" />
          Re-validate {selectedIds.size}
        </button>

        <div class="w-px h-4 bg-base-300"></div>

        <!-- Link selected (show when some are unlinked) -->
        {#if selectedUnlinkedCount > 0}
          <button
            class="btn btn-ghost btn-xs gap-1.5 text-info/70 hover:text-info hover:bg-info/10"
            onclick={() => bulkToggleManaged(true)}
            disabled={loading || bulkLinking}
            title="Link {selectedUnlinkedCount} mod{selectedUnlinkedCount > 1
              ? 's'
              : ''} — create symlinks so they are loaded when connecting to servers"
          >
            {#if bulkLinking}
              <span class="loading loading-spinner loading-xs"></span>
            {:else}
              <Icon icon="ph:link-simple" class="size-3.5" />
            {/if}
            Link {selectedUnlinkedCount}
          </button>
        {/if}

        <!-- Unlink selected (show when some are linked) -->
        {#if selectedLinkedCount > 0}
          <button
            class="btn btn-ghost btn-xs gap-1.5 text-base-content/50 hover:text-base-content/80 hover:bg-base-300/50"
            onclick={() => bulkToggleManaged(false)}
            disabled={loading || bulkLinking}
            title="Unlink {selectedLinkedCount} mod{selectedLinkedCount > 1
              ? 's'
              : ''} — remove symlinks so they won't be loaded"
          >
            {#if bulkLinking}
              <span class="loading loading-spinner loading-xs"></span>
            {:else}
              <Icon icon="ph:link-simple-break" class="size-3.5" />
            {/if}
            Unlink {selectedLinkedCount}
          </button>
        {/if}

        <div class="w-px h-4 bg-base-300"></div>

        <!-- Delete selected -->
        <button
          class="btn btn-ghost btn-xs gap-1.5 text-error/70 hover:text-error hover:bg-error/10"
          onclick={() => onDeleteSelected(selectedMods.map((m) => m.id))}
          disabled={loading}
          title="Delete {selectedIds.size} selected mod{selectedIds.size > 1 ? 's' : ''}"
        >
          <Icon icon="ph:trash" class="size-3.5" />
          Delete {selectedIds.size}
        </button>

        <div class="w-px h-4 bg-base-300"></div>

        <!-- Dismiss -->
        <button
          class="btn btn-ghost btn-xs gap-1 text-base-content/40 hover:text-base-content/70"
          onclick={() => {
            selectedIds = new Set();
          }}
          title="Clear selection"
        >
          <Icon icon="ph:x" class="size-3.5" />
          Clear
        </button>
      </div>
    </div>
  {/if}
</div>
