<script lang="ts">
  import type { InstalledModDto } from '$lib/types';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '@iconify/svelte';
  import * as m from '$lib/paraglide/messages.js';

  type ModInfo = {
    id: number;
    name: string;
    local_updated?: number;
    remote_updated?: number | null;
    size_human?: string;
  };

  interface Props {
    mods: ModInfo[];
    mode: 'update' | 'install';
    onConfirm: () => void;
    onCancel: () => void;
  }

  let { mods, mode, onConfirm, onCancel }: Props = $props();

  function workshopUrl(modId: number): string {
    return `https://steamcommunity.com/sharedfiles/filedetails/?id=${modId}`;
  }

  function openWorkshop(modId: number) {
    openUrl(workshopUrl(modId)).catch(() => {
      window.open(workshopUrl(modId), '_blank');
    });
  }

  function formatDate(timestamp: number | undefined | null): string {
    if (!timestamp) return '—';
    const date = new Date(timestamp * 1000);
    return date.toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    });
  }

  function getDaysDiff(local: number | undefined, remote: number | undefined | null): number | null {
    if (!local || !remote) return null;
    const diffMs = (remote - local) * 1000;
    const days = Math.floor(diffMs / (1000 * 60 * 60 * 24));
    return days > 0 ? days : null;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  role="dialog"
  aria-modal="true"
  aria-labelledby="mod-confirm-title"
  tabindex="-1"
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
  onkeydown={handleKeydown}
  onclick={(e) => e.target === e.currentTarget && onCancel()}
>
  <div class="bg-base-100 border border-base-300 rounded-lg shadow-xl w-[500px] max-h-[80vh] flex flex-col">
    <!-- Header -->
    <div class="flex items-center justify-between px-4 py-3 border-b border-base-300 flex-shrink-0">
      <h2 id="mod-confirm-title" class="text-sm font-semibold">
        {mode === 'update'
          ? m.mods_confirm_title_update({ count: mods.length })
          : m.mods_confirm_title_install({ count: mods.length })}
      </h2>
      <button type="button" class="btn btn-ghost btn-xs btn-square" aria-label="Close" onclick={onCancel}>
        <Icon icon="ph:x" class="size-4" />
      </button>
    </div>

    <!-- Mod list (scrollable, max 400px height) -->
    <div class="overflow-y-auto p-3 space-y-2 max-h-[400px]">
      {#each mods as mod (mod.id)}
        {@const daysDiff = getDaysDiff(mod.local_updated, mod.remote_updated)}
        <div class="bg-base-200/50 rounded-lg p-3 border border-base-300/50">
          <!-- Mod name (clickable) -->
          <button
            type="button"
            class="flex items-center gap-2 text-left text-sm font-medium text-primary hover:underline"
            onclick={() => openWorkshop(mod.id)}
            title={m.mods_confirm_open_workshop()}
          >
            <Icon icon="ph:link-simple" class="size-3.5 shrink-0" />
            <span class="truncate">{mod.name || `Workshop ID: ${mod.id}`}</span>
            <Icon icon="ph:arrow-square-out" class="size-3 shrink-0 opacity-50" />
          </button>

          <!-- Dates (for updates) -->
          {#if mode === 'update' && (mod.local_updated || mod.remote_updated)}
            <div class="mt-2 grid grid-cols-2 gap-2 text-xs">
              <div class="flex items-center gap-1.5 text-base-content/60">
                <Icon icon="ph:hard-drive" class="size-3.5" />
                <span>{m.mods_confirm_local_date()}:</span>
                <span class="text-base-content/80">{formatDate(mod.local_updated)}</span>
              </div>
              <div class="flex items-center gap-1.5">
                <Icon icon="ph:cloud" class="size-3.5 text-info/70" />
                <span class="text-base-content/60">{m.mods_confirm_remote_date()}:</span>
                <span class="text-info">{formatDate(mod.remote_updated)}</span>
                {#if daysDiff}
                  <span class="text-warning text-[10px]">
                    ({m.mods_confirm_newer({ days: daysDiff })})
                  </span>
                {/if}
              </div>
            </div>
          {/if}

          <!-- Size (if available) -->
          {#if mod.size_human}
            <div class="mt-1 text-xs text-base-content/40">
              {mod.size_human}
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <!-- Warning -->
    <div class="px-4 py-2 border-t border-base-300 bg-warning/5">
      <div class="flex items-center gap-2 text-xs text-warning">
        <Icon icon="ph:warning" class="size-4 shrink-0" />
        <span>{m.mods_confirm_warning()}</span>
      </div>
    </div>

    <!-- Footer -->
    <div class="flex items-center justify-end gap-2 px-4 py-3 border-t border-base-300 flex-shrink-0">
      <button type="button" class="btn btn-ghost btn-sm" onclick={onCancel}>
        {m.mods_confirm_cancel()}
      </button>
      <button type="button" class="btn btn-primary btn-sm gap-1.5" onclick={onConfirm}>
        <Icon icon={mode === 'update' ? 'ph:arrows-clockwise' : 'ph:download-simple'} class="size-4" />
        {mode === 'update'
          ? m.mods_confirm_update({ count: mods.length })
          : m.mods_confirm_install({ count: mods.length })}
      </button>
    </div>
  </div>
</div>
