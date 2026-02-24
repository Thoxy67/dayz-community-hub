<script lang="ts">
  import type { ModOpState } from '$lib/types';
  import Icon from '@iconify/svelte';

  interface Props {
    state: ModOpState;
    onDismiss: () => void;
  }

  let { state, onDismiss }: Props = $props();

  let isSteamGuard = $derived(state.phase === 'steam_guard_mobile');

  let progressPct = $derived(
    state.total > 0 ? Math.round((state.completed.length / state.total) * 100) : 0
  );

  let statusText = $derived(() => {
    if (state.phase === 'shutting_down') return 'Closing Steam before SteamCMD can run…';
    if (state.phase === 'steam_guard_mobile') return 'Waiting for Steam Guard…';
    if (state.phase === 'finished') {
      if (state.hint) return 'Login failed or credentials expired';
      if (state.failed === 0) return `Done — ${state.ok} mod${state.ok !== 1 ? 's' : ''} completed`;
      return `Done — ${state.ok} OK, ${state.failed} failed`;
    }
    return state.currentName ? `Downloading: ${state.currentName}` : 'Preparing…';
  });

  let canDismiss = $derived(state.phase === 'finished');

  // Auto-dismiss after 2.5 s when everything succeeded (no failures, no hint).
  $effect(() => {
    if (state.phase === 'finished' && state.failed === 0 && !state.hint) {
      const t = setTimeout(onDismiss, 2500);
      return () => clearTimeout(t);
    }
  });
</script>

{#if state.active}
  <div class="modal modal-open">
    <div class="modal-box max-w-lg">

      {#if isSteamGuard}
        <!-- Steam Guard Mobile Required — full red alert, replaces progress -->
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
          {#if state.phase !== 'finished'}
            <span class="loading loading-spinner loading-sm text-primary"></span>
          {:else if state.failed > 0 || state.hint}
            <Icon icon="ph:x-circle" class="size-5 text-error" />
          {:else}
            <Icon icon="ph:check-circle" class="size-5 text-success" />
          {/if}
          Mod Operation
          {#if state.total > 0}
            <span class="text-base-content/50 text-sm font-normal">
              [{state.completed.length}/{state.total}]
            </span>
          {/if}
        </h3>

        <!-- Status line -->
        <p class="mt-2 text-sm {state.hint ? 'text-error' : state.phase === 'finished' && state.failed === 0 ? 'text-success' : 'text-base-content/70'}">
          {statusText()}
        </p>

        <!-- Progress bar -->
        {#if state.phase === 'downloading' || state.phase === 'finished'}
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
        {#if state.hint}
          <div class="alert alert-error mt-3 text-xs">
            <pre class="whitespace-pre-wrap font-mono">{state.hint}</pre>
          </div>
        {/if}

        <!-- Completed mods list -->
        {#if state.completed.length > 0}
          <div class="mt-3 max-h-40 overflow-y-auto rounded-lg bg-base-200 p-2 space-y-0.5">
            {#each state.completed.slice(-12) as entry}
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
