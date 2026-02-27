<script lang="ts">
  import { tick } from 'svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import type { ModOpState } from '$lib/types';
  import Icon from '@iconify/svelte';

  interface Props {
    modOp: ModOpState;
    onDismiss: () => void;
    onSendPassword?: (password: string) => void;
    /** Steam login name — used to build the manual steamcmd command. */
    steamLogin?: string | null;
    /** Called when user clicks "I don't trust" — should cancel the operation. */
    onDontTrust?: () => void;
    /** Called to cancel the running mod operation (kills steamcmd). */
    onCancel?: () => void;
  }

  let { modOp, onDismiss, onSendPassword, steamLogin, onDontTrust, onCancel }: Props = $props();

  let isSteamGuard = $derived(modOp.phase === 'steam_guard_mobile');
  let isPasswordRequired = $derived(modOp.phase === 'password_required');
  let logContainer: HTMLElement | null = $state(null);

  // Password input state
  let passwordInput = $state('');
  let showPasswordInput = $state(false);
  let passwordSending = $state(false);

  // "I don't trust" flow — shows manual login instructions after copying command
  let dontTrustShown = $state(false);
  let cmdCopied = $state(false);

  function submitPassword() {
    if (!passwordInput || !onSendPassword) return;
    passwordSending = true;
    onSendPassword(passwordInput);
    passwordInput = '';
    // passwordSending will be cleared when the phase changes away from password_required
  }

  async function handleDontTrust() {
    const login = steamLogin || 'YOUR_USERNAME';
    const cmd = `steamcmd +login ${login} +quit`;
    await writeText(cmd);
    cmdCopied = true;
    dontTrustShown = true;
    setTimeout(() => { cmdCopied = false; }, 2000);
  }

  function confirmDontTrust() {
    onDontTrust?.();
  }

  // Reset password state when we leave the password_required phase
  $effect(() => {
    if (modOp.phase !== 'password_required') {
      passwordSending = false;
      showPasswordInput = false;
      dontTrustShown = false;
      cmdCopied = false;
    }
  });

  // ── Steam Guard countdown (~65 s per attempt before SteamCMD times out) ─
  const STEAM_GUARD_TIMEOUT = 65;
  let sgCountdown = $state(STEAM_GUARD_TIMEOUT);

  $effect(() => {
    if (modOp.phase === 'steam_guard_mobile') {
      sgCountdown = STEAM_GUARD_TIMEOUT;
      const iv = setInterval(() => {
        sgCountdown = Math.max(0, sgCountdown - 1);
      }, 1000);
      return () => clearInterval(iv);
    }
  });

  // Reset countdown when SteamCMD retries (it gives another ~60 s).
  let prevLogLen = $state(0);
  $effect(() => {
    const len = modOp.log.length;
    if (modOp.phase === 'steam_guard_mobile' && len > prevLogLen) {
      // Check only the newly added lines for "Retrying"
      for (let i = prevLogLen; i < len; i++) {
        if (modOp.log[i].toLowerCase().includes('retrying')) {
          sgCountdown = STEAM_GUARD_TIMEOUT;
          break;
        }
      }
    }
    prevLogLen = len;
  });

  let sgCountdownPct = $derived(
    Math.round((sgCountdown / STEAM_GUARD_TIMEOUT) * 100)
  );

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
    if (modOp.phase === 'password_required') return 'Waiting for password…';
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
        <!-- Steam Guard Mobile Required (yellow / warning) -->
        <div class="flex items-start gap-4 p-4 rounded-xl bg-warning/8 border border-warning/20">
          <div class="relative shrink-0 size-12 flex items-center justify-center">
            <div class="sg-pulse-ring"></div>
            <div class="relative z-10 size-10 rounded-full bg-warning/15 border border-warning/25 flex items-center justify-center">
              <Icon icon="ph:shield-warning-fill" class="size-6 text-warning" />
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
                <span class="inline-flex items-center justify-center size-[18px] rounded-full bg-warning/15 border border-warning/25 text-[10px] font-bold text-warning shrink-0">1</span>
                <span class="text-[11px] text-base-content/60 whitespace-nowrap">Open Steam app on phone</span>
              </div>
              <Icon icon="ph:arrow-right" class="size-3 text-base-content/30 shrink-0" />
              <div class="flex items-center gap-1.5">
                <span class="inline-flex items-center justify-center size-[18px] rounded-full bg-warning/15 border border-warning/25 text-[10px] font-bold text-warning shrink-0">2</span>
                <span class="text-[11px] text-base-content/60 whitespace-nowrap">Tap the approval notification</span>
              </div>
              <Icon icon="ph:arrow-right" class="size-3 text-base-content/30 shrink-0" />
              <div class="flex items-center gap-1.5">
                <span class="inline-flex items-center justify-center size-[18px] rounded-full bg-success/15 border border-success/25 text-[10px] font-bold text-success shrink-0">3</span>
                <span class="text-[11px] text-success/80 whitespace-nowrap">Download resumes</span>
              </div>
            </div>

            <!-- Countdown timer -->
            <div class="mt-3">
              <div class="flex items-center justify-between mb-1">
                <div class="flex items-center gap-2 text-xs text-base-content/50">
                  <span class="loading loading-dots loading-xs text-warning/60"></span>
                  <span>Waiting for confirmation…</span>
                </div>
                <span class="text-xs font-mono tabular-nums {sgCountdown <= 15 ? 'text-error font-semibold' : sgCountdown <= 30 ? 'text-warning' : 'text-base-content/40'}">
                  {Math.floor(sgCountdown / 60)}:{String(sgCountdown % 60).padStart(2, '0')}
                </span>
              </div>
              <progress
                class="progress w-full h-1.5 {sgCountdown <= 15 ? 'progress-error' : 'progress-warning'}"
                value={sgCountdownPct}
                max="100"
              ></progress>
              {#if sgCountdown <= 15 && sgCountdown > 0}
                <p class="text-[11px] text-error/70 mt-1">Hurry — SteamCMD will time out soon!</p>
              {:else if sgCountdown === 0}
                <p class="text-[11px] text-error/70 mt-1">SteamCMD may have timed out. Check the log below.</p>
              {/if}
            </div>
            <div class="flex justify-end mt-3">
              <button class="btn btn-xs btn-error btn-outline gap-1 opacity-70 hover:opacity-100" onclick={onCancel}>
                <Icon icon="ph:x" class="size-3" />
                Cancel
              </button>
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

        {#if isPasswordRequired}
          <div class="mt-3 p-3 rounded-xl bg-warning/8 border border-warning/20">
            {#if dontTrustShown}
              <!-- Manual login instructions after "I don't trust" -->
              <div class="flex items-center gap-2 mb-2">
                <div class="size-8 rounded-full bg-info/15 border border-info/25 flex items-center justify-center shrink-0">
                  <Icon icon="ph:terminal-window-fill" class="size-4 text-info" />
                </div>
                <div>
                  <h4 class="font-semibold text-sm text-base-content">Login Manually via Terminal</h4>
                </div>
              </div>
              <div class="text-xs text-base-content/70 space-y-2">
                <p>The following command has been copied to your clipboard:</p>
                <code class="block bg-base-300/60 rounded-lg px-3 py-2 font-mono text-[11px] text-base-content/90 select-all">steamcmd +login {steamLogin || 'YOUR_USERNAME'} +quit</code>
                <ol class="list-decimal list-inside space-y-1 text-base-content/60">
                  <li>Open a terminal and paste the command</li>
                  <li>Enter your password and complete Steam Guard if prompted</li>
                  <li>SteamCMD will cache your credentials locally</li>
                  <li>Come back here and retry — no password will be needed</li>
                </ol>
                <p class="text-[11px] text-base-content/40 leading-snug">
                  This app never sees your password. SteamCMD stores an encrypted credential token
                  on your machine that it reuses for future logins.
                </p>
              </div>
              <div class="flex justify-end mt-3">
                <button class="btn btn-sm btn-primary gap-1" onclick={confirmDontTrust}>
                  <Icon icon="ph:check" class="size-3.5" />
                  Got it, cancel operation
                </button>
              </div>
            {:else}
              <!-- Normal password entry -->
              <div class="flex items-center gap-2 mb-2">
                <div class="size-8 rounded-full bg-warning/15 border border-warning/25 flex items-center justify-center shrink-0">
                  <Icon icon="ph:lock-fill" class="size-4 text-warning" />
                </div>
                <div>
                  <h4 class="font-semibold text-sm text-base-content">Steam Password Required</h4>
                  <p class="text-[11px] text-base-content/50 leading-snug">SteamCMD needs your password to log in. It will not be stored.</p>
                </div>
              </div>
              <form
                class="flex items-center gap-2"
                onsubmit={(e) => { e.preventDefault(); submitPassword(); }}
              >
                <label class="input input-sm input-bordered flex items-center gap-2 flex-1 bg-base-200/60">
                  <Icon icon="ph:key" class="size-3.5 text-base-content/40" />
                  {#if showPasswordInput}
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                      type="text"
                      class="grow text-xs"
                      placeholder="Enter Steam password"
                      bind:value={passwordInput}
                      disabled={passwordSending}
                      autofocus
                    />
                  {:else}
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                      type="password"
                      class="grow text-xs"
                      placeholder="Enter Steam password"
                      bind:value={passwordInput}
                      disabled={passwordSending}
                      autofocus
                    />
                  {/if}
                  <button
                    type="button"
                    class="btn btn-ghost btn-xs btn-circle"
                    onclick={() => showPasswordInput = !showPasswordInput}
                    tabindex={-1}
                  >
                    <Icon icon={showPasswordInput ? 'ph:eye-slash' : 'ph:eye'} class="size-3.5 text-base-content/40" />
                  </button>
                </label>
                <button
                  type="submit"
                  class="btn btn-sm btn-warning gap-1"
                  disabled={!passwordInput || passwordSending}
                >
                  {#if passwordSending}
                    <span class="loading loading-spinner loading-xs"></span>
                  {:else}
                    <Icon icon="ph:paper-plane-tilt-fill" class="size-3.5" />
                  {/if}
                  Send
                </button>
              </form>
              <!-- "I don't trust" option -->
              <div class="flex justify-end mt-2">
                <button
                  class="btn btn-xs btn-error btn-outline gap-1 opacity-70 hover:opacity-100"
                  onclick={handleDontTrust}
                >
                  <Icon icon="ph:shield-slash" class="size-3" />
                  {cmdCopied ? 'Copied!' : "I don't trust this"}
                </button>
              </div>
            {/if}
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
