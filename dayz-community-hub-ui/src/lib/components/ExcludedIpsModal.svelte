<script lang="ts">
  import Icon from '@iconify/svelte';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    excludedIps: string[];
    onUnexclude: (ip: string) => void;
    onClose: () => void;
  }

  let { excludedIps, onUnexclude, onClose }: Props = $props();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
  }
</script>

<!-- z-[60] so it sits above the profile modal (z-50) -->
<div
  class="fixed inset-0 modal-backdrop-window z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm"
  style="top: 36px;"
  role="presentation"
  onclick={onClose}
>
  <div
    class="bg-base-100 rounded-xl shadow-2xl w-full max-w-sm mx-4 flex flex-col overflow-hidden max-h-[70vh]"
    role="dialog"
    aria-modal="true"
    aria-label={m.excluded_ips_title()}
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={handleKeydown}
  >
    <!-- Header -->
    <div class="flex items-center gap-3 px-5 py-4 bg-base-200 border-b border-base-300 flex-shrink-0">
      <div class="size-8 rounded-lg bg-error/10 border border-error/20 flex items-center justify-center shrink-0">
        <Icon icon="ph:prohibit" class="size-4 text-error/70" />
      </div>
      <div class="flex-1 min-w-0">
        <p class="text-sm font-semibold text-base-content">{m.excluded_ips_title()}</p>
        <p class="text-xs text-base-content/45 mt-0.5">{m.excluded_ips_desc()}</p>
      </div>
      <button
        class="size-7 rounded flex items-center justify-center text-base-content/40 hover:bg-base-300 hover:text-base-content transition-colors shrink-0"
        onclick={onClose}
        title={m.settings_close()}
      >
        <Icon icon="ph:x" class="size-3.5" />
      </button>
    </div>

    <!-- Body -->
    <div class="flex-1 overflow-y-auto">
      {#if excludedIps.length === 0}
        <div class="flex flex-col items-center justify-center py-12 gap-2 text-base-content/30">
          <Icon icon="ph:check-circle" class="size-8" />
          <span class="text-sm">{m.excluded_ips_none()}</span>
        </div>
      {:else}
        <div class="divide-y divide-base-300/40">
          {#each excludedIps as ip}
            <div class="flex items-center gap-3 px-4 py-2.5 hover:bg-base-200/50 transition-colors group">
              <Icon icon="ph:prohibit-fill" class="size-3.5 text-error/50 shrink-0" />
              <span class="flex-1 text-xs font-mono text-base-content/70">{ip}</span>
              <button
                type="button"
                class="text-base-content/25 hover:text-error transition-colors shrink-0 opacity-0 group-hover:opacity-100"
                onclick={() => onUnexclude(ip)}
                title={m.excluded_ips_remove({ ip })}
              >
                <Icon icon="ph:x" class="size-3.5" />
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </div>

    <!-- Footer -->
    <div class="flex items-center justify-between px-5 py-3 border-t border-base-300 bg-base-200/60 flex-shrink-0">
      <span class="text-xs text-base-content/40">
        {excludedIps.length === 1
          ? m.excluded_ips_count_one({ count: excludedIps.length })
          : m.excluded_ips_count_other({ count: excludedIps.length })}
      </span>
      <button class="btn btn-ghost btn-sm" onclick={onClose}>{m.settings_close()}</button>
    </div>
  </div>
</div>
