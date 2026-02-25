<script lang="ts">
  import { tick } from 'svelte';
  import type { ModOpState } from '$lib/types';
  import Icon from '@iconify/svelte';

  interface Props {
    modOp: ModOpState;
    onDismiss: () => void;
  }

  let { modOp, onDismiss }: Props = $props();

  let isSteamGuard = $derived(modOp.phase === 'steam_guard_mobile');
  let logContainer: HTMLElement | null = $state(null);

  // Auto-scroll log to bottom whenever a new line arrives.
  $effect(() => {
    if (modOp.log.length && logContainer) {
      tick().then(() => {
        logContainer!.scrollTop = logContainer!.scrollHeight;
      });
    }
  });

  let progressPct = $derived(
    modOp.total > 0 ? Math.round((modOp.completed.length / modOp.total) * 100) : 0
  );

  let statusText = $derived(() => {
    if (modOp.phase === 'shutting_down') return 'Closing Steam before SteamCMD can run…';
    if (modOp.phase === 'steam_guard_mobile') return 'Waiting for Steam Guard…';
    if (modOp.phase === 'finished') {
      if (modOp.hint) return 'Login failed or credentials expired';
      if (modOp.failed === 0) return `Done — ${modOp.ok} mod${modOp.ok !== 1 ? 's' : ''} completed`;
      return `Done — ${modOp.ok} OK, ${modOp.failed} failed`;
    }
    return modOp.currentName ? `Downloading: ${modOp.currentName}` : 'Preparing…';
  });

  let canDismiss = $derived(modOp.phase === 'finished');

  // Auto-dismiss after 2.5 s when everything succeeded (no failures, no hint).
  $effect(() => {
    if (modOp.phase === 'finished' && modOp.failed === 0 && !modOp.hint) {
      const t = setTimeout(onDismiss, 2500);
      return () => clearTimeout(t);
    }
  });
</script>

{#if modOp.active}
  <div class="modal modal-open">
    <div class="modal-box max-w-lg">

      {#if isSteamGuard}
        <!-- Steam Guard Mobile Required — full red alert -->
        <div class="alert alert-error flex-col items-start gap-3">
          <div class="flex items-center gap-3">
            <Icon icon="ph:shield-warning" class="size-8 flex-shrink-0" />
            <div>
              <h3 class="font-bold text-lg">Steam Guard Authorization Required</h3>
              <p class="text-sm mt-1">
                Open the <strong>Steam Mobile app</strong> on your phone and confirm the login to continue.
              </p>
            </div>
          </div>
          <p class="text-xs opacity-70">The download will resume automatically once you approve it.</p>
        </div>
      {:else}
        <h3 class="font-bold text-lg flex items-center gap-2">
          {#if modOp.phase !== 'finished'}
            <span class="loading loading-spinner loading-sm text-primary"></span>
          {:else if modOp.failed > 0 || modOp.hint}
            <Icon icon="ph:x-circle" class="size-5 text-error" />
          {:else}
            <Icon icon="ph:check-circle" class="size-5 text-success" />
          {/if}
          Mod Operation
          {#if modOp.total > 0}
            <span class="text-base-content/50 text-sm font-normal">
              [{modOp.completed.length}/{modOp.total}]
            </span>
          {/if}
        </h3>

        <!-- Status line -->
        <p class="mt-2 text-sm {modOp.hint ? 'text-error' : modOp.phase === 'finished' && modOp.failed === 0 ? 'text-success' : 'text-base-content/70'}">
          {statusText()}
        </p>

        <!-- Progress bar -->
        {#if modOp.phase === 'downloading' || modOp.phase === 'finished'}
          <div class="mt-3">
            <progress
              class="progress progress-primary w-full"
              value={progressPct}
              max="100"
            ></progress>
            <p class="text-xs text-right text-base-content/40 mt-0.5">{progressPct}%</p>
          </div>
        {/if}

        <!-- SteamCMD hint -->
        {#if modOp.hint}
          <div class="alert alert-error mt-3 text-xs">
            <pre class="whitespace-pre-wrap font-mono">{modOp.hint}</pre>
          </div>
        {/if}

        <!-- Completed mods list -->
        {#if modOp.completed.length > 0}
          <div class="mt-3 max-h-40 overflow-y-auto rounded-lg bg-base-200 p-2 space-y-0.5">
            {#each modOp.completed.slice(-12) as entry}
              <div class="flex items-center gap-2 text-xs">
                {#if entry.ok}
                  <Icon icon="ph:check" class="size-3.5 text-success flex-shrink-0" />
                {:else}
                  <Icon icon="ph:x" class="size-3.5 text-error flex-shrink-0" />
                {/if}
                <span class="text-base-content/80 truncate">{entry.name}</span>
                <span class="text-base-content/40 ml-auto font-mono">{entry.id}</span>
              </div>
            {/each}
          </div>
        {/if}
      {/if}

      <!-- steamcmd log — shown in all phases whenever lines are available -->
      {#if modOp.log.length > 0}
        <div class="mt-3">
          <p class="text-xs text-base-content/40 mb-1">steamcmd output</p>
          <div
            bind:this={logContainer}
            class="max-h-48 overflow-y-auto rounded-lg bg-black/60 p-2 font-mono text-xs text-base-content/70"
          >
            {#each modOp.log as line}
              <div class="leading-snug whitespace-pre-wrap break-all">{line}</div>
            {/each}
          </div>
        </div>
      {/if}

      <div class="modal-action">
        <button
          class="btn btn-sm {canDismiss ? 'btn-primary' : 'btn-disabled'}"
          disabled={!canDismiss}
          onclick={onDismiss}
        >
          {canDismiss ? 'Dismiss' : 'Working…'}
        </button>
      </div>
    </div>
    <div class="modal-backdrop bg-base-content/20"></div>
  </div>
{/if}
