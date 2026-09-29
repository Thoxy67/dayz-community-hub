<script lang="ts">
  import { app } from '$lib/state.svelte';
  import Icon from '@iconify/svelte';

  const kindStyles = {
    error: 'bg-error/15 text-error border-error/30',
    success: 'bg-success/15 text-success border-success/30',
    warning: 'bg-warning/15 text-warning border-warning/30',
    info: 'bg-info/10 text-info border-info/30',
  } as const;

  const kindIcons = {
    error: 'ph:x-circle',
    success: 'ph:check-circle',
    warning: 'ph:warning',
    info: 'ph:info',
  } as const;
</script>

{#if app.toasts.length > 0}
  <div
    class="flex flex-col gap-1 px-3 py-1 border-b border-base-300 flex-shrink-0"
    role="status"
    aria-live="polite"
    aria-atomic="false"
  >
    {#each app.toasts as toast (toast.id)}
      <div class="flex items-center gap-2 px-2 py-1 text-xs rounded border {kindStyles[toast.kind]}">
        <Icon icon={kindIcons[toast.kind]} class="size-3.5 shrink-0" />
        <span class="flex-1 truncate">{toast.message}</span>
        <button
          type="button"
          class="p-0.5 rounded hover:bg-base-content/10 transition-colors"
          aria-label="Dismiss notification"
          onclick={() => app.dismissToast(toast.id)}
        >
          <Icon icon="ph:x" class="size-3" />
        </button>
      </div>
    {/each}
  </div>
{/if}
