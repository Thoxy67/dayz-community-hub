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

  // ── Log line colorizer ────────────────────────────────────────────────────
  type LineKind = 'error' | 'warning' | 'success' | 'progress' | 'login' | 'dim' | 'normal';

  function classifyLine(line: string): LineKind {
    const l = line.toLowerCase();
    if (
      l.includes('error') || l.includes('failed') || l.includes('failure') ||
      l.includes('abort') || l.includes('not found') ||
      l.includes('cached credentials not found') ||
      l.includes('invalid password') || l.includes('access denied')
    ) return 'error';
    if (
      l.includes('warning') || l.includes('timed out') ||
      l.includes('retry') || l.includes('steam guard')
    ) return 'warning';
    if (
      l.includes('success') || l.includes('already up to date') ||
      l.includes('fully installed')
    ) return 'success';
    if (
      l.includes('downloading item') || l.includes('workshop_download_item') ||
      l.includes('update state') || l.includes('reconfiguring') ||
      l.includes('validating') || l.includes(' kb,') || l.includes(' mb,') ||
      /\d+\s*%/.test(l)
    ) return 'progress';
    if (
      l.includes('logging in') || l.includes('logged in ok') ||
      l.includes('+login') || l.includes('connecting to') ||
      l.includes('loading steam') || l.includes('steamcmd') ||
      l.startsWith('steam>')
    ) return 'login';
    if (
      l.startsWith('[') || l.includes('appinfo') ||
      l.includes('waiting on') || l.includes('idle') ||
      l.trim() === '' || /^\s*\d+\s*$/.test(l)
    ) return 'dim';
    return 'normal';
  }

  const KIND_CLASS: Record<LineKind, string> = {
    error:    'log-error',
    warning:  'log-warning',
    success:  'log-success',
    progress: 'log-progress',
    login:    'log-login',
    dim:      'log-dim',
    normal:   'log-normal',
  };
</script>

{#if modOp.active}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
    style="top: 36px;"
    role="presentation"
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="bg-base-100 rounded-xl shadow-2xl w-full max-w-lg mx-4 p-6"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
    >

      {#if isSteamGuard}
        <!-- Steam Guard Mobile Required -->
        <div class="flex items-start gap-4 p-4 rounded-xl bg-error/8 border border-error/20">
          <div class="relative shrink-0 size-12 flex items-center justify-center">
            <div class="sg-pulse-ring"></div>
            <div class="relative z-10 size-10 rounded-full bg-error/15 border border-error/25 flex items-center justify-center">
              <Icon icon="ph:shield-warning-fill" class="size-6 text-error" />
            </div>
          </div>
          <div class="flex-1 min-w-0">
            <h3 class="font-bold text-base text-base-content">Steam Guard Authorization Required</h3>
            <p class="text-sm text-base-content/70 mt-1 leading-snug">
              Open the <strong class="text-base-content/90">Steam Mobile app</strong> on your phone
              and approve the sign-in to continue.
            </p>
            <div class="flex items-center gap-1.5 mt-3 flex-wrap">
              <div class="flex items-center gap-1.5">
                <span class="inline-flex items-center justify-center size-[18px] rounded-full bg-error/15 border border-error/25 text-[10px] font-bold text-error shrink-0">1</span>
                <span class="text-[11px] text-base-content/60 whitespace-nowrap">Open Steam app on phone</span>
              </div>
              <Icon icon="ph:arrow-right" class="size-3 text-base-content/30 shrink-0" />
              <div class="flex items-center gap-1.5">
                <span class="inline-flex items-center justify-center size-[18px] rounded-full bg-error/15 border border-error/25 text-[10px] font-bold text-error shrink-0">2</span>
                <span class="text-[11px] text-base-content/60 whitespace-nowrap">Tap the approval notification</span>
              </div>
              <Icon icon="ph:arrow-right" class="size-3 text-base-content/30 shrink-0" />
              <div class="flex items-center gap-1.5">
                <span class="inline-flex items-center justify-center size-[18px] rounded-full bg-success/15 border border-success/25 text-[10px] font-bold text-success shrink-0">3</span>
                <span class="text-[11px] text-success/80 whitespace-nowrap">Download resumes</span>
              </div>
            </div>
            <div class="flex items-center gap-2 mt-3 text-xs text-base-content/40">
              <span class="loading loading-dots loading-xs text-error/50"></span>
              <span>Waiting for confirmation…</span>
            </div>
          </div>
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

        <p class="mt-2 text-sm {modOp.hint ? 'text-error' : modOp.phase === 'finished' && modOp.failed === 0 ? 'text-success' : 'text-base-content/70'}">
          {statusText()}
        </p>

        {#if modOp.phase === 'downloading' || modOp.phase === 'finished'}
          <div class="mt-3">
            <progress class="progress progress-primary w-full" value={progressPct} max="100"></progress>
            <p class="text-xs text-right text-base-content/40 mt-0.5">{progressPct}%</p>
          </div>
        {/if}

        {#if modOp.hint}
          <div class="alert alert-error mt-3 text-xs">
            <pre class="whitespace-pre-wrap font-mono">{modOp.hint}</pre>
          </div>
        {/if}

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

      <!-- SteamCMD log -->
      {#if modOp.log.length > 0}
        <div class="mt-3">
          <div class="flex items-center justify-between mb-1 px-0.5">
            <span class="text-xs font-mono text-base-content/30 tracking-widest uppercase">$ steamcmd</span>
            <span class="text-xs font-mono text-base-content/20 tabular-nums">{modOp.log.length} lines</span>
          </div>
          <!-- terminal-wrap uses inline style so scoped CSS doesn't get hashed away -->
          <div
            class="terminal-wrap rounded-lg overflow-hidden"
            style="background:#0d0f10; position:relative; border:1px solid rgba(255,255,255,0.07);"
          >
            <!-- scanline overlay -->
            <div class="terminal-scanlines" aria-hidden="true"></div>
            <!-- log content -->
            <div
              bind:this={logContainer}
              class="terminal-log max-h-64 overflow-y-auto px-3 py-2.5 font-mono text-xs space-y-px"
              style="position:relative; z-index:2;"
            >
              {#each modOp.log as line}
                {@const kind = classifyLine(line)}
                <div class="leading-snug whitespace-pre-wrap break-all {KIND_CLASS[kind]}">
                  {line}
                </div>
              {/each}
              {#if modOp.phase !== 'finished'}
                <div class="terminal-cursor" aria-hidden="true"></div>
              {/if}
            </div>
          </div>
        </div>
      {/if}

      <div class="flex justify-end mt-4">
        <button
          class="btn btn-sm {canDismiss ? 'btn-primary' : 'btn-disabled'}"
          disabled={!canDismiss}
          onclick={onDismiss}
        >
          {canDismiss ? 'Dismiss' : 'Working…'}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* ── Scanline overlay ───────────────────────────────────────────────────── */
  .terminal-scanlines {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 1;
    border-radius: inherit;
    background: repeating-linear-gradient(
      to bottom,
      transparent        0px,
      transparent        3px,
      rgba(255,255,255,0.04) 3px,
      rgba(255,255,255,0.04) 4px
    );
  }

  /* ── Log line colors ────────────────────────────────────────────────────── */
  .log-error    { color: #f87171; font-weight: 600; }  /* red-400   */
  .log-warning  { color: #fbbf24; }                    /* amber-400 */
  .log-success  { color: #34d399; font-weight: 600; }  /* emerald-400 */
  .log-progress { color: #7dd3fc; }                    /* sky-300   */
  .log-login    { color: #94a3b8; }                    /* slate-400 */
  .log-dim      { color: rgba(255,255,255,0.18); }
  .log-normal   { color: rgba(255,255,255,0.82); }

  /* ── Blinking cursor ────────────────────────────────────────────────────── */
  .terminal-cursor {
    display: inline-block;
    width: 7px;
    height: 0.85em;
    background: rgba(255, 255, 255, 0.65);
    vertical-align: text-bottom;
    margin-top: 2px;
    animation: cur-blink 1s step-end infinite;
  }

  @keyframes cur-blink {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0; }
  }
</style>
