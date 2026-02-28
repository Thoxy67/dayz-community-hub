<script lang="ts">
  import Icon from '@iconify/svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { getVersion, getName } from '@tauri-apps/api/app';
  import { invoke } from '@tauri-apps/api/core';
  import { onMount, onDestroy } from 'svelte';
  import GlitchText from '$lib/components/GlitchText.svelte';
  import SuikaGame from '$lib/components/SuikaGame.svelte';
  import { APP_AUTHOR, REPO_URL, ISSUES_URL, RELEASES_URL, WIKI_URL, LICENSE_URL } from '$lib/constants';

  type UpdateState = 'idle' | 'checking' | 'up_to_date' | 'available' | 'downloading' | 'done' | 'error';

  type UpdateInfo = {
    version: string;
    currentVersion: string;
    body: string | null;
    date: string | null;
  };

  interface Props {
    onExport: (includeMods: boolean) => void;
    onImport: () => void;
    onReset: () => void;
    updateState?: UpdateState;
    updateInfo?: UpdateInfo | null;
    updateError?: string;
    dlPercent?: number;
    dlReceived?: number;
    dlTotal?: number;
    onCheckForUpdate?: () => void;
    onInstallUpdate?: () => void;
  }

  let {
    onExport,
    onImport,
    onReset,
    updateState = 'idle',
    updateInfo = null,
    updateError = '',
    dlPercent = 0,
    dlReceived = 0,
    dlTotal = 0,
    onCheckForUpdate,
    onInstallUpdate,
  }: Props = $props();

  let appVersion = $state('');
  let appName = $state('DayZ Community Hub');

  // Platform: 'windows' | 'linux' | 'macos' | '' (unknown until loaded)
  let platform = $state('');

  // Windows SteamCMD install state
  let cmdDownloading = $state(false);
  let cmdDownloadErr = $state('');
  let cmdDownloadOk = $state(false);

  // ── Entrance animation ─────────────────────────────────────────────────────
  let mounted = $state(false);

  // Update state is lifted to +page.svelte and passed in as props.

  // ── Konami code → easter egg ───────────────────────────────────────────────
  const KONAMI = [
    'ArrowUp',
    'ArrowUp',
    'ArrowDown',
    'ArrowDown',
    'ArrowLeft',
    'ArrowRight',
    'ArrowLeft',
    'ArrowRight',
    'b',
    'a',
  ];
  let konamiIndex = 0;
  let eggOpen = $state(false);

  function onKeydown(e: KeyboardEvent) {
    if (e.key === KONAMI[konamiIndex]) {
      konamiIndex++;
      if (konamiIndex === KONAMI.length) {
        konamiIndex = 0;
        if (!eggOpen) eggOpen = true;
        return;
      }
    } else {
      konamiIndex = e.key === KONAMI[0] ? 1 : 0;
    }
  }

  onMount(async () => {
    requestAnimationFrame(() => {
      mounted = true;
    });
    try {
      [appVersion, appName] = await Promise.all([getVersion(), getName()]);
    } catch {
      /* ignore in dev */
    }
    try {
      const status = await invoke<{ found: boolean; path: string | null; platform: string }>('detect_steamcmd');
      platform = status.platform;
    } catch {
      /* ignore */
    }
    if (platform === 'windows') onCheckForUpdate?.();
    window.addEventListener('keydown', onKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', onKeydown);
  });

  async function installSteamcmdWindows() {
    cmdDownloading = true;
    cmdDownloadErr = '';
    cmdDownloadOk = false;
    try {
      await invoke('download_steamcmd_windows');
      cmdDownloadOk = true;
    } catch (e) {
      cmdDownloadErr = String(e);
    } finally {
      cmdDownloading = false;
    }
  }

  // Export option: include managed mod list
  let exportIncludeMods = $state(true);

  const AUTHOR = APP_AUTHOR;
</script>

<!-- ── Easter Egg ─────────────────────────────────────────────────────────── -->
{#if eggOpen}
  <SuikaGame
    onClose={() => {
      eggOpen = false;
    }}
  />
{/if}

<div class="h-full overflow-y-auto">
  <div class="max-w-6xl mx-auto px-6 py-6">
    <!-- ── Hero (full width) ─────────────────────────────────────────────── -->
    <div
      class="relative rounded-2xl overflow-hidden border border-base-300/60 bg-base-200/40 mb-6 stagger-item"
      class:visible={mounted}
      style="transition-delay: 0ms"
    >
      <div
        class="absolute inset-0 bg-gradient-to-br from-primary/8 via-transparent to-secondary/5 pointer-events-none"
      ></div>
      <div class="relative px-6 py-5 flex items-center gap-5">
        <div class="relative shrink-0 icon-hover-spin">
          <div class="absolute inset-0 rounded-2xl bg-primary/20 blur-xl scale-110 pointer-events-none"></div>
          <img src="/icon.svg" alt="DayZ Community Hub" class="relative w-14 h-14 rounded-2xl shadow-lg" />
        </div>
        <div class="flex-1 min-w-0">
          <GlitchText
            text={appName}
            tag="h1"
            class="text-xl font-bold text-base-content tracking-tight leading-tight font-mono"
          />
          <p class="text-xs text-base-content/50 mt-0.5">Server browser &amp; mod manager for DayZ Standalone</p>
          <div class="flex flex-wrap items-center gap-2 mt-2">
            {#if appVersion}
              <span
                class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-primary/10 border border-primary/20 text-xs text-primary font-mono font-medium"
              >
                <Icon icon="ph:tag" class="size-3" />v{appVersion}
              </span>
            {/if}
            <span
              class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-base-300/60 border border-base-300 text-xs text-base-content/50"
            >
              <Icon icon="ph:user" class="size-3" />{AUTHOR}
            </span>
            <button
              class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-base-300/60 border border-base-300 text-xs text-base-content/50 hover:bg-primary/10 hover:border-primary/30 hover:text-primary transition-all"
              onclick={() => openUrl(REPO_URL)}
              title="Source repository"
            >
              <Icon icon="catppuccin:forgejo" class="size-3" />Forgejo
            </button>
          </div>
        </div>
        <!-- Feature highlights inline on the right -->
        <div class="hidden lg:flex items-center gap-4 shrink-0 border-l border-base-300/40 pl-6">
          {#each [{ icon: 'mdi:server-network', label: 'Server Browser', color: 'text-sky-400' }, { icon: 'mdi:puzzle', label: 'Mod Manager', color: 'text-fuchsia-400' }, { icon: 'ph:chart-line-up', label: 'BattleMetrics', color: 'text-emerald-400' }, { icon: 'ph:rocket-launch', label: 'One-click Launch', color: 'text-orange-400' }] as feat}
            <div class="feat-icon flex flex-col items-center gap-1 text-center cursor-default">
              <Icon icon={feat.icon} class="size-5 {feat.color}" />
              <span class="text-xs text-base-content/50 font-medium whitespace-nowrap">{feat.label}</span>
            </div>
          {/each}
        </div>
      </div>
    </div>

    <!-- ── Two-column layout ─────────────────────────────────────────────── -->
    <div class="grid grid-cols-1 lg:grid-cols-[1fr_320px] gap-6 items-start">
      <!-- ── LEFT COLUMN ─────────────────────────────────────────────────── -->
      <div class="space-y-6">
        <!-- Updates (Windows only) -->
        {#if platform === 'windows'}
          <section class="stagger-item" class:visible={mounted} style="transition-delay: 60ms">
            <div class="flex items-center gap-2 mb-3">
              <Icon icon="ph:arrow-circle-up" class="size-4 text-primary shrink-0" />
              <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Updates</h2>
            </div>
            <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden">
              {#if updateState === 'idle' || updateState === 'checking'}
                <div class="flex items-center gap-3 px-4 py-3.5">
                  <span class="loading loading-spinner loading-sm text-primary shrink-0"></span>
                  <span class="text-sm text-base-content/60">Checking for updates…</span>
                </div>
              {:else if updateState === 'up_to_date'}
                <div class="flex items-center gap-3 px-4 py-3.5">
                  <div
                    class="size-8 rounded-lg bg-success/10 border border-success/20 flex items-center justify-center shrink-0"
                  >
                    <Icon icon="ph:check-circle" class="size-4 text-success" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-semibold text-base-content">You're up to date</p>
                    <p class="text-xs text-base-content/45 mt-0.5">v{appVersion} is the latest version</p>
                  </div>
                  <button class="btn btn-xs btn-ghost text-base-content/40 gap-1" onclick={onCheckForUpdate}>
                    <Icon icon="ph:arrows-clockwise" class="size-3.5" />Re-check
                  </button>
                </div>
              {:else if updateState === 'available'}
                <div class="flex items-start gap-3 px-4 py-3.5 border-b border-base-300/40">
                  <div
                    class="size-8 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center shrink-0 mt-0.5"
                  >
                    <Icon icon="ph:arrow-circle-up" class="size-4 text-primary" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-semibold text-base-content">
                      Update available — <span class="text-primary font-mono">v{updateInfo?.version}</span>
                    </p>
                    <p class="text-xs text-base-content/45 mt-0.5">
                      Current: <span class="font-mono">v{updateInfo?.currentVersion}</span>
                      {#if updateInfo?.date}
                        · Released {new Date(updateInfo.date).toLocaleDateString()}{/if}
                    </p>
                    {#if updateInfo?.body}
                      <p class="text-xs text-base-content/50 mt-2 leading-relaxed line-clamp-3">{updateInfo.body}</p>
                    {/if}
                  </div>
                </div>
                <div class="flex items-center justify-end gap-2 px-4 py-2.5 bg-base-200/60">
                  <button class="btn btn-sm btn-primary gap-1.5" onclick={onInstallUpdate}>
                    <Icon icon="ph:download-simple" class="size-3.5" />Install update
                  </button>
                </div>
              {:else if updateState === 'downloading'}
                <div class="px-4 py-3.5 space-y-2.5">
                  <div class="flex items-center gap-2">
                    <span class="loading loading-spinner loading-sm text-primary shrink-0"></span>
                    <span class="text-sm font-semibold text-base-content"
                      >Downloading v{updateInfo?.version ?? '…'}…</span
                    >
                    {#if dlTotal > 0}
                      <span class="ml-auto text-xs text-base-content/40 tabular-nums font-mono shrink-0"
                        >{dlPercent}%</span
                      >
                    {/if}
                  </div>
                  <div class="w-full rounded-full bg-base-300 overflow-hidden" style="height:6px;">
                    <div
                      class="h-full bg-primary rounded-full transition-all duration-200"
                      style="width:{dlTotal > 0 ? dlPercent : 0}%"
                    ></div>
                  </div>
                  {#if dlTotal > 0}
                    <p class="text-xs text-base-content/35 tabular-nums">
                      {(dlReceived / 1024 / 1024).toFixed(1)} MB / {(dlTotal / 1024 / 1024).toFixed(1)} MB
                    </p>
                  {/if}
                </div>
              {:else if updateState === 'done'}
                <div class="flex items-center gap-3 px-4 py-3.5">
                  <div
                    class="size-8 rounded-lg bg-success/10 border border-success/20 flex items-center justify-center shrink-0"
                  >
                    <Icon icon="ph:check-circle" class="size-4 text-success" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-semibold text-base-content">Download complete</p>
                    <p class="text-xs text-base-content/45 mt-0.5">
                      The installer is launching — the app will restart automatically.
                    </p>
                  </div>
                </div>
              {:else if updateState === 'error'}
                <div class="px-4 py-3.5 space-y-2">
                  <div class="flex items-start gap-2 text-error">
                    <Icon icon="ph:warning-circle" class="size-4 shrink-0 mt-0.5" />
                    <p class="text-xs leading-relaxed break-all">{updateError}</p>
                  </div>
                  <button class="btn btn-xs btn-ghost text-base-content/40 gap-1" onclick={onCheckForUpdate}>
                    <Icon icon="ph:arrows-clockwise" class="size-3.5" />Retry
                  </button>
                </div>
              {/if}
            </div>
          </section>
        {/if}

        <!-- Quick Start -->
        <section class="stagger-item" class:visible={mounted} style="transition-delay: 120ms">
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:rocket-launch" class="size-4 text-primary shrink-0" />
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Quick Start</h2>
          </div>
          <div class="space-y-2">
            {#each [{ n: 1, icon: 'ph:user-gear', title: 'Open account settings', body: 'Click your name (or "Set up account") in the title bar to configure your player name, Steam credentials, and API keys.' }, { n: 2, icon: 'ph:game-controller', title: 'Set your in-game name', body: 'This is the character name passed as -name= when launching DayZ.' }, { n: 3, icon: 'mdi:server-network', title: 'Browse & connect', body: 'Use the Servers tab to find a server. Double-click a row or click Connect to launch DayZ directly.' }] as step, i}
              <div
                class="stagger-item flex items-start gap-4 px-4 py-3 rounded-xl bg-base-200/50 border border-base-300/50 hover:border-primary/20 hover:bg-base-200/80 transition-colors"
                class:visible={mounted}
                style="transition-delay: {180 + i * 60}ms"
              >
                <span
                  class="size-6 rounded-full bg-primary text-primary-content text-xs font-bold flex items-center justify-center shrink-0 mt-0.5 shadow-sm"
                  >{step.n}</span
                >
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-semibold text-base-content flex items-center gap-1.5">
                    <Icon icon={step.icon} class="size-3.5 text-primary/70 shrink-0" />{step.title}
                  </p>
                  <p class="text-xs text-base-content/50 mt-0.5 leading-relaxed">{step.body}</p>
                </div>
              </div>
            {/each}
          </div>
        </section>

        <!-- SteamCMD & Mods -->
        <section class="stagger-item" class:visible={mounted} style="transition-delay: 360ms">
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:terminal-window" class="size-4 text-primary shrink-0" />
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">SteamCMD &amp; Mods</h2>
          </div>
          <div class="rounded-xl border border-base-300/60 bg-base-200/40 divide-y divide-base-300/40">
            <div class="px-4 py-3">
              <p class="text-xs font-semibold text-base-content/70 mb-1.5">What is SteamCMD?</p>
              <p class="text-xs text-base-content/50 leading-relaxed">
                A command-line tool from Valve used to download Steam Workshop content without the full Steam client.
                The app uses it to install and update DayZ mods in the background. If the title bar shows <span
                  class="text-warning/80 font-semibold">SteamCMD not found</span
                >, install it below or set the path manually in account settings.
              </p>
              {#if platform === 'linux' || platform === ''}
                <div class="mt-2 space-y-1 text-xs font-mono">
                  {#each [{ label: 'Debian / Ubuntu', cmd: 'sudo apt install steamcmd' }, { label: 'Arch / Manjaro', cmd: 'yay -S steamcmd' }, { label: 'Fedora / RHEL', cmd: 'sudo dnf install steamcmd' }] as row}
                    <div class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-base-300/40">
                      <Icon icon="simple-icons:linux" class="size-3.5 text-base-content/40 shrink-0" />
                      <span class="text-base-content/40 shrink-0 w-28">{row.label}</span>
                      <span class="text-primary ml-auto">{row.cmd}</span>
                    </div>
                  {/each}
                </div>
              {/if}
              {#if platform === 'windows'}
                <div class="mt-3 space-y-2">
                  <p class="text-xs text-base-content/50 leading-relaxed">
                    SteamCMD will be downloaded from Valve and installed to
                    <span class="font-mono text-base-content/70">%APPDATA%\dayz-community-hub\steamcmd\</span>.
                  </p>
                  {#if cmdDownloadOk}
                    <div
                      class="flex items-center gap-2 px-3 py-2 rounded-lg bg-success/10 border border-success/25 text-xs text-success"
                    >
                      <Icon icon="ph:check-circle" class="size-4 shrink-0" />SteamCMD installed successfully.
                    </div>
                  {:else}
                    {#if cmdDownloadErr}
                      <div
                        class="flex items-start gap-2 px-3 py-2 rounded-lg bg-error/10 border border-error/25 text-xs text-error"
                      >
                        <Icon icon="ph:warning-circle" class="size-3.5 shrink-0 mt-0.5" />
                        <span class="break-all">{cmdDownloadErr}</span>
                      </div>
                    {/if}
                    <button
                      class="btn btn-sm btn-primary gap-2 w-full"
                      onclick={installSteamcmdWindows}
                      disabled={cmdDownloading}
                    >
                      {#if cmdDownloading}
                        <span class="loading loading-spinner loading-xs"></span>Downloading SteamCMD…
                      {:else}
                        <Icon icon="ph:download-simple" class="size-4" />Install SteamCMD
                      {/if}
                    </button>
                  {/if}
                </div>
              {/if}
            </div>
            <div class="px-4 py-3">
              <p class="text-xs font-semibold text-base-content/70 mb-2">Mod workflow</p>
              <div class="space-y-2">
                {#each ['Connect to a server — the app offers to install any missing mods automatically.', 'Go to the Mods tab and click Check updates to see which mods are outdated.', 'Click Update N to update all stale mods in one batch, or update them individually.'] as step, i}
                  <div class="flex items-start gap-3 text-xs text-base-content/50">
                    <span
                      class="size-4 rounded-full bg-base-300/80 text-base-content/50 text-xs font-bold flex items-center justify-center shrink-0 mt-0.5"
                      >{i + 1}</span
                    >
                    <span class="leading-relaxed">{step}</span>
                  </div>
                {/each}
              </div>
            </div>
          </div>
        </section>

        <!-- Keyboard Shortcuts -->
        <section class="stagger-item" class:visible={mounted} style="transition-delay: 420ms">
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:keyboard" class="size-4 text-primary shrink-0" />
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Keyboard Shortcuts</h2>
          </div>
          <div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <div>
              <p class="text-xs font-semibold text-base-content/35 uppercase tracking-widest mb-1 px-1">Global</p>
              <div class="rounded-xl border border-base-300/50 overflow-hidden">
                {#each [{ keys: ['Ctrl', '1 – 9'], desc: 'Switch to tab 1–9' }, { keys: ['Ctrl', 'R'], desc: 'Refresh server list' }, { keys: ['Ctrl', 'U'], desc: 'Update stale mods' }, { keys: ['Ctrl', 'L'], desc: 'Reconnect to last server' }] as row, i}
                  <div
                    class="flex items-center gap-2 px-3 py-1.5 {i % 2 === 0
                      ? 'bg-base-200/30'
                      : ''} border-b border-base-300/30 last:border-0"
                  >
                    <div class="flex items-center gap-1 shrink-0 w-28">
                      {#each row.keys as k}<kbd class="kbd kbd-xs">{k}</kbd>{/each}
                    </div>
                    <span class="text-xs text-base-content/55">{row.desc}</span>
                  </div>
                {/each}
              </div>
            </div>

            <div>
              <p class="text-xs font-semibold text-base-content/35 uppercase tracking-widest mb-1 px-1">
                Servers · Favorites · History
              </p>
              <div class="rounded-xl border border-base-300/50 overflow-hidden">
                {#each [{ keys: ['↑', '↓'], desc: 'Navigate the list' }, { keys: ['Enter'], desc: 'Connect to server' }, { keys: ['D'], desc: 'Open in Direct Connect' }, { keys: ['F'], desc: 'Toggle favorite' }, { keys: ['I'], desc: 'Toggle info panel' }, { keys: ['P'], desc: 'Ping selected server' }, { keys: ['Esc'], desc: 'Close panel / deselect' }, { keys: ['Dbl-click'], desc: 'Connect to server' }] as row, i}
                  <div
                    class="flex items-center gap-2 px-3 py-1.5 {i % 2 === 0
                      ? 'bg-base-200/30'
                      : ''} border-b border-base-300/30 last:border-0"
                  >
                    <div class="flex items-center gap-1 shrink-0 w-28">
                      {#each row.keys as k}<kbd class="kbd kbd-xs">{k}</kbd>{/each}
                    </div>
                    <span class="text-xs text-base-content/55">{row.desc}</span>
                  </div>
                {/each}
              </div>
            </div>

            <div>
              <p class="text-xs font-semibold text-base-content/35 uppercase tracking-widest mb-1 px-1">Mods</p>
              <div class="rounded-xl border border-base-300/50 overflow-hidden">
                {#each [{ keys: ['↑', '↓'], desc: 'Navigate the list' }, { keys: ['Space'], desc: 'Toggle checkbox' }, { keys: ['M'], desc: 'Toggle managed' }, { keys: ['Dbl-click'], desc: 'Open mod folder' }] as row, i}
                  <div
                    class="flex items-center gap-2 px-3 py-1.5 {i % 2 === 0
                      ? 'bg-base-200/30'
                      : ''} border-b border-base-300/30 last:border-0"
                  >
                    <div class="flex items-center gap-1 shrink-0 w-28">
                      {#each row.keys as k}<kbd class="kbd kbd-xs">{k}</kbd>{/each}
                    </div>
                    <span class="text-xs text-base-content/55">{row.desc}</span>
                  </div>
                {/each}
              </div>
            </div>
          </div>
        </section>

        <!-- Sharing & Quick Connect -->
        <section class="stagger-item" class:visible={mounted} style="transition-delay: 480ms">
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:share-network" class="size-4 text-primary shrink-0" />
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">
              Sharing &amp; Quick Connect
            </h2>
          </div>
          <div class="rounded-xl border border-base-300/60 bg-base-200/40 divide-y divide-base-300/40">
            <!-- How it works -->
            <div class="px-4 py-3">
              <p class="text-xs font-semibold text-base-content/70 mb-1.5">How it works</p>
              <p class="text-xs text-base-content/50 leading-relaxed">
                Share a server with friends by copying a <span
                  class="font-mono bg-base-300/60 px-1 rounded text-base-content/60">dzch://</span
                >
                URL or exporting a
                <span class="font-mono bg-base-300/60 px-1 rounded text-base-content/60">.dzch</span> file from the Direct
                Connect tab. When someone opens the link or file, the app automatically fills in the server address, queries
                for live info, and offers to install any missing mods before connecting.
              </p>
            </div>

            <!-- dzch:// URLs -->
            <div class="px-4 py-3">
              <div class="flex items-center gap-2 mb-2">
                <Icon icon="ph:link" class="size-3.5 text-indigo-400 shrink-0" />
                <p class="text-xs font-semibold text-base-content/70">dzch:// URLs</p>
              </div>
              <p class="text-xs text-base-content/50 leading-relaxed mb-2">
                Compact links that encode a server address, mods, and optional password. Ideal for sharing in Discord,
                forums, or anywhere you can paste a link.
              </p>
              <div class="space-y-1.5 text-xs font-mono">
                <div class="px-3 py-1.5 rounded-lg bg-base-300/40">
                  <span class="text-base-content/35 select-none">Basic: </span><span class="text-primary break-all"
                    >dzch://192.168.1.1:2302</span
                  >
                </div>
                <div class="px-3 py-1.5 rounded-lg bg-base-300/40">
                  <span class="text-base-content/35 select-none">With mods: </span><span class="text-primary break-all"
                    >dzch://192.168.1.1:2302?mods=123456,789012</span
                  >
                </div>
                <div class="px-3 py-1.5 rounded-lg bg-base-300/40">
                  <span class="text-base-content/35 select-none">Full: </span><span class="text-primary break-all"
                    >dzch://192.168.1.1:2302?qport=27016&name=My%20Server&password=secret&mods=123456,789012</span
                  >
                </div>
              </div>
              <div class="mt-2 space-y-1">
                {#each [{ param: 'qport', desc: 'Query port for A2S (falls back to heuristics if omitted)' }, { param: 'name', desc: 'Server display name (informational, URL-encoded)' }, { param: 'password', desc: 'Connection password (omit if none)' }, { param: 'mods', desc: 'Comma-separated Workshop IDs' }] as row}
                  <div class="flex items-start gap-2 text-xs">
                    <span class="font-mono text-primary/80 shrink-0 w-20 text-right">{row.param}</span>
                    <span class="text-base-content/45">{row.desc}</span>
                  </div>
                {/each}
              </div>
            </div>

            <!-- .dzch files -->
            <div class="px-4 py-3">
              <div class="flex items-center gap-2 mb-2">
                <Icon icon="ph:file-text" class="size-3.5 text-emerald-400 shrink-0" />
                <p class="text-xs font-semibold text-base-content/70">.dzch files</p>
              </div>
              <p class="text-xs text-base-content/50 leading-relaxed mb-2">
                A small JSON file containing the full server config including mod names. Double-click one to open it in
                the app, or use the
                <span class="font-semibold text-base-content/60">Export .dzch</span> button in the Direct Connect tab to create
                one.
              </p>
              <pre
                class="px-3 py-2 rounded-lg bg-base-300/40 text-xs font-mono text-base-content/60 leading-relaxed overflow-x-auto">{`{
  "version": 1,
  "ip": "192.168.1.1",
  "port": 2302,
  "query_port": 27016,
  "name": "My Server",
  "mods": [
    { "id": 123456, "name": "Mod A" },
    { "id": 789012, "name": "Mod B" }
  ]
}`}</pre>
            </div>

            <!-- How to share -->
            <div class="px-4 py-3">
              <p class="text-xs font-semibold text-base-content/70 mb-2">Sharing from Direct Connect</p>
              <div class="space-y-2">
                {#each ['Query a server in the Direct Connect tab (or press D on a server in the list).', 'Click Copy URL to copy a dzch:// link to the clipboard, or Export .dzch to save a file.', 'Send the link or file to friends — they open it and the app handles the rest.'] as step, i}
                  <div class="flex items-start gap-3 text-xs text-base-content/50">
                    <span
                      class="size-4 rounded-full bg-base-300/80 text-base-content/50 text-xs font-bold flex items-center justify-center shrink-0 mt-0.5"
                      >{i + 1}</span
                    >
                    <span class="leading-relaxed">{step}</span>
                  </div>
                {/each}
              </div>
            </div>
          </div>
        </section>

        <!-- Profile Management -->
        <section class="stagger-item" class:visible={mounted} style="transition-delay: 540ms">
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:archive" class="size-4 text-primary shrink-0" />
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Profile Management</h2>
          </div>
          <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden divide-y divide-base-300/40">
            <div class="flex items-center gap-4 px-4 py-3">
              <div
                class="size-8 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center shrink-0"
              >
                <Icon icon="ph:upload-simple" class="size-4 text-primary" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-semibold text-base-content">Export</p>
                <p class="text-xs text-base-content/45 mt-0.5 leading-relaxed">
                  Save settings, favorites, history to a
                  <span class="font-mono bg-base-300/60 px-1 rounded text-base-content/60">.dchub</span> file (zstd-compressed,
                  portable between machines).
                </p>
                <label class="flex items-center gap-2 mt-2 cursor-pointer select-none">
                  <input
                    type="checkbox"
                    class="checkbox checkbox-xs checkbox-primary"
                    bind:checked={exportIncludeMods}
                  />
                  <span class="text-xs text-base-content/60">Include managed mod list</span>
                </label>
              </div>
              <button class="btn btn-sm btn-primary shrink-0 gap-1.5" onclick={() => onExport(exportIncludeMods)}>
                <Icon icon="ph:upload-simple" class="size-3.5" />Export
              </button>
            </div>
            <div class="flex items-center gap-4 px-4 py-3">
              <div
                class="size-8 rounded-lg bg-base-300/60 border border-base-300 flex items-center justify-center shrink-0"
              >
                <Icon icon="ph:download-simple" class="size-4 text-base-content/50" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-semibold text-base-content">Import</p>
                <p class="text-xs text-base-content/45 mt-0.5 leading-relaxed">
                  Restore a previously exported
                  <span class="font-mono bg-base-300/60 px-1 rounded text-base-content/60">.dchub</span> file. Overwrites
                  current profile and mod list.
                </p>
              </div>
              <button class="btn btn-sm btn-ghost shrink-0 gap-1.5" onclick={onImport}>
                <Icon icon="ph:download-simple" class="size-3.5" />Import
              </button>
            </div>
            <div class="flex items-center gap-4 px-4 py-3">
              <div
                class="size-8 rounded-lg bg-error/10 border border-error/20 flex items-center justify-center shrink-0"
              >
                <Icon icon="ph:arrow-counter-clockwise" class="size-4 text-error/70" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-semibold text-error/80">Reset to defaults</p>
                <p class="text-xs text-base-content/45 mt-0.5 leading-relaxed">
                  Wipe profile back to factory defaults — clears all settings, favorites, history and launch options.
                  Installed mods on disk are not affected.
                </p>
              </div>
              <button class="btn btn-sm btn-error btn-outline shrink-0 gap-1.5" onclick={onReset}>
                <Icon icon="ph:arrow-counter-clockwise" class="size-3.5" />Reset
              </button>
            </div>
          </div>
        </section>
      </div>
      <!-- end left column -->

      <!-- ── RIGHT SIDEBAR ───────────────────────────────────────────────── -->
      <div class="space-y-6">
        <!-- Tabs reference -->
        <section class="stagger-item" class:visible={mounted} style="transition-delay: 150ms">
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:tabs" class="size-4 text-primary shrink-0" />
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Tabs</h2>
          </div>
          <div class="rounded-xl border border-base-300/50 overflow-hidden divide-y divide-base-300/30">
            {#each [{ icon: 'mdi:server-network', label: 'Servers', color: 'text-sky-400', desc: 'Live server list with search, filters and sortable table.' }, { icon: 'ph:star', label: 'Favorites', color: 'text-yellow-400', desc: 'Starred servers with live counts and ping.' }, { icon: 'ph:clock-clockwise', label: 'History', color: 'text-teal-400', desc: 'Previously joined servers with timestamps and live stats.' }, { icon: 'mdi:puzzle', label: 'Mods', color: 'text-fuchsia-400', desc: 'Manage Workshop mods — check, update, and clean up via SteamCMD.' }, { icon: 'ph:newspaper', label: 'News', color: 'text-rose-400', desc: 'Latest DayZ news from the official website.' }, { icon: 'ph:plugs-connected', label: 'Connect', color: 'text-indigo-400', desc: 'Direct connect by IP:port.' }, { icon: 'ph:sliders', label: 'Options', color: 'text-orange-400', desc: 'Toggle DayZ launch options.' }, { icon: 'ph:mountains', label: 'Offline', color: 'text-emerald-400', desc: 'Community Offline Mode — solo missions without a server.' }] as tab, i}
              <div
                class="flex items-start gap-2.5 px-3 py-2.5 {i % 2 === 0
                  ? 'bg-base-200/20'
                  : ''} hover:bg-base-200/40 transition-colors"
              >
                <Icon icon={tab.icon} class="size-3.5 {tab.color} shrink-0 mt-0.5" />
                <div class="flex-1 min-w-0">
                  <span class="text-xs font-semibold text-base-content/80">{tab.label}</span>
                  <span class="text-xs text-base-content/40 leading-snug block mt-0.5">{tab.desc}</span>
                </div>
              </div>
            {/each}
          </div>
        </section>

        <!-- Optional API Keys -->
        <section class="stagger-item" class:visible={mounted} style="transition-delay: 240ms">
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:key" class="size-4 text-primary shrink-0" />
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Optional API Keys</h2>
          </div>
          <div class="space-y-2">
            <!-- Steam -->
            <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden">
              <div class="flex items-center gap-2.5 px-3 py-2.5 border-b border-base-300/40 bg-base-200/60">
                <Icon icon="ph:identification-card" class="size-3.5 text-sky-400 shrink-0" />
                <div class="flex-1 min-w-0">
                  <p class="text-xs font-semibold text-base-content">Steam API Key &amp; Steam ID</p>
                  <p class="text-xs text-base-content/40">Avatar in title bar · mod update checks</p>
                </div>
              </div>
              <div class="px-3 py-2.5 space-y-1.5">
                <div class="flex items-start gap-1.5 text-xs text-base-content/50">
                  <Icon icon="ph:user-circle" class="size-3 text-sky-400/60 shrink-0 mt-0.5" />
                  <span
                    ><span class="font-semibold text-base-content/65">Avatar</span> — Steam profile picture in the title bar.</span
                  >
                </div>
                <div class="flex items-start gap-1.5 text-xs text-base-content/50">
                  <Icon icon="mdi:puzzle-outline" class="size-3 text-sky-400/60 shrink-0 mt-0.5" />
                  <span
                    ><span class="font-semibold text-base-content/65">Mod checks</span> — Workshop API queries at higher rate
                    limits.</span
                  >
                </div>
                <div class="flex flex-wrap gap-1.5 pt-0.5">
                  <button
                    class="inline-flex items-center gap-1 px-2 py-1 rounded-lg bg-base-300/50 border border-base-300 text-xs text-primary hover:bg-primary/10 hover:border-primary/30 transition-all"
                    onclick={() => openUrl('https://steamcommunity.com/dev/apikey')}
                  >
                    <Icon icon="mdi:steam" class="size-3" />API key
                  </button>
                  <button
                    class="inline-flex items-center gap-1 px-2 py-1 rounded-lg bg-base-300/50 border border-base-300 text-xs text-primary hover:bg-primary/10 hover:border-primary/30 transition-all"
                    onclick={() => openUrl('https://steamdb.info/calculator/')}
                  >
                    <Icon icon="ph:calculator" class="size-3" />Steam ID
                  </button>
                </div>
              </div>
            </div>
            <!-- BattleMetrics -->
            <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden">
              <div class="flex items-center gap-2.5 px-3 py-2.5 border-b border-base-300/40 bg-base-200/60">
                <Icon icon="ph:chart-line-up" class="size-3.5 text-emerald-400 shrink-0" />
                <div class="flex-1 min-w-0">
                  <p class="text-xs font-semibold text-base-content">BattleMetrics Token</p>
                  <p class="text-xs text-base-content/40">Rankings · uptime · distance · 24 h player graph</p>
                </div>
              </div>
              <div class="px-3 py-2.5 space-y-1.5">
                <div class="grid grid-cols-2 gap-1.5">
                  {#each [{ icon: 'ph:trophy', color: 'text-yellow-400/70', label: 'Global rank' }, { icon: 'ph:globe', color: 'text-sky-400/70', label: 'Country' }, { icon: 'ph:map-pin', color: 'text-rose-400/70', label: 'Distance' }, { icon: 'ph:clock', color: 'text-teal-400/70', label: 'Uptime %' }, { icon: 'ph:chart-line-up', color: 'text-emerald-400/70', label: 'Player graph' }] as feat}
                    <div
                      class="flex items-center gap-1.5 px-2 py-1.5 rounded-lg bg-base-300/30 border border-base-300/40"
                    >
                      <Icon icon={feat.icon} class="size-3 {feat.color} shrink-0" />
                      <span class="text-xs text-base-content/60">{feat.label}</span>
                    </div>
                  {/each}
                </div>
                <div class="pt-0.5">
                  <button
                    class="inline-flex items-center gap-1 px-2 py-1 rounded-lg bg-base-300/50 border border-base-300 text-xs text-primary hover:bg-primary/10 hover:border-primary/30 transition-all"
                    onclick={() => openUrl('https://www.battlemetrics.com/developers')}
                  >
                    <Icon icon="ph:chart-line-up" class="size-3" />battlemetrics.com/developers
                  </button>
                </div>
              </div>
            </div>
          </div>
        </section>

        <!-- Built with -->
        <section class="stagger-item" class:visible={mounted} style="transition-delay: 330ms">
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:code" class="size-4 text-primary shrink-0" />
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Built with</h2>
          </div>
          <div class="flex flex-wrap gap-2">
            {#each [{ icon: 'simple-icons:tauri', label: 'Tauri 2', url: 'https://tauri.app', color: 'text-sky-400' }, { icon: 'simple-icons:svelte', label: 'Svelte 5', url: 'https://svelte.dev', color: 'text-orange-400' }, { icon: 'simple-icons:rust', label: 'Rust', url: 'https://www.rust-lang.org', color: 'text-orange-600' }, { icon: 'simple-icons:daisyui', label: 'DaisyUI 5', url: 'https://daisyui.com', color: 'text-violet-400' }, { icon: 'simple-icons:tailwindcss', label: 'Tailwind', url: 'https://tailwindcss.com', color: 'text-cyan-400' }] as tech}
              <button
                class="tech-badge inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-base-200/60 border border-base-300/60 text-xs text-base-content/50 hover:bg-base-200 hover:border-base-300 hover:text-base-content/80"
                onclick={() => openUrl(tech.url)}
              >
                <Icon icon={tech.icon} class="size-3.5 {tech.color}" />{tech.label}
              </button>
            {/each}
          </div>
        </section>

        <!-- Links -->
        <section class="stagger-item" class:visible={mounted} style="transition-delay: 420ms">
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:link" class="size-4 text-primary shrink-0" />
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Links</h2>
          </div>
          <div class="flex flex-wrap gap-2">
            {#each [{ icon: 'ph:git-branch', label: 'Releases', url: RELEASES_URL, color: 'text-emerald-400' }, { icon: 'ph:bug', label: 'Report a Bug', url: ISSUES_URL, color: 'text-orange-400' }, { icon: 'ph:book-open', label: 'Wiki', url: WIKI_URL, color: 'text-sky-400' }, { icon: 'ph:scroll', label: 'License', url: LICENSE_URL, color: 'text-violet-400' }] as link}
              <button
                class="tech-badge inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-base-200/60 border border-base-300/60 text-xs text-base-content/50 hover:bg-base-200 hover:border-base-300 hover:text-base-content/80"
                onclick={() => openUrl(link.url)}
              >
                <Icon icon={link.icon} class="{link.color} size-3.5" />{link.label}
              </button>
            {/each}
          </div>
        </section>
      </div>
      <!-- end right sidebar -->
    </div>
    <!-- end two-column grid -->

    <!-- ── Footer (full width) ──────────────────────────────────────────── -->
    <div
      class="stagger-item flex items-center justify-between pt-3 pb-4 mt-6 border-t border-base-300/40 text-xs text-base-content/25"
      class:visible={mounted}
      style="transition-delay: 600ms"
    >
      <span>{appName}{appVersion ? ` v${appVersion}` : ''}</span>
      <span class="flex items-center gap-1.5">
        Made with <Icon icon="ph:heart-fill" class="size-3 text-error/40" /> by {AUTHOR}
        <button
          class="hover:text-primary transition-colors ml-1"
          onclick={() => openUrl(REPO_URL)}
          title="Source repository"
        >
          <Icon icon="catppuccin:forgejo" class="size-3.5" />
        </button>
      </span>
    </div>
  </div>
</div>

<style>
  /* Hero icon: subtle rotate on hover */
  .icon-hover-spin {
    transition: transform 0.6s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .icon-hover-spin:hover {
    transform: rotate(15deg) scale(1.1);
  }

  /* Stagger fade-slide-up for list items */
  .stagger-item {
    opacity: 0;
    transform: translateY(12px);
    transition:
      opacity 0.35s ease,
      transform 0.35s ease;
  }
  .stagger-item.visible {
    opacity: 1;
    transform: translateY(0);
  }

  /* Tech badge: bounce on hover */
  .tech-badge {
    transition:
      transform 0.2s cubic-bezier(0.34, 1.56, 0.64, 1),
      background-color 0.2s,
      border-color 0.2s,
      color 0.2s;
  }
  .tech-badge:hover {
    transform: translateY(-3px) scale(1.06);
  }

  /* Feature highlight icons: float on hover */
  .feat-icon {
    transition: transform 0.3s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .feat-icon:hover {
    transform: translateY(-4px) scale(1.15);
  }
</style>
