<script lang="ts">
  import Icon from '@iconify/svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { getVersion, getName } from '@tauri-apps/api/app';
  import { invoke, Channel } from '@tauri-apps/api/core';
  import { onMount } from 'svelte';

  interface Props {
    onExport: () => void;
    onImport: () => void;
    onReset: () => void;
  }

  let { onExport, onImport, onReset }: Props = $props();

  let appVersion = $state('');
  let appName    = $state('DayZ Community Hub');

  // Platform: 'windows' | 'linux' | 'macos' | '' (unknown until loaded)
  let platform = $state('');

  // Windows SteamCMD install state
  let cmdDownloading = $state(false);
  let cmdDownloadErr = $state('');
  let cmdDownloadOk  = $state(false);

  // ── Update checker (Windows only) ──────────────────────────────────────────

  type UpdateInfo = {
    version: string;
    currentVersion: string;
    body: string | null;
    date: string | null;
  };

  type DownloadEvent =
    | { event: 'Started';   data: { contentLength: number | null } }
    | { event: 'Progress';  data: { chunkLength: number } }
    | { event: 'Finished' };

  type UpdateState =
    | 'idle'          // haven't checked yet
    | 'checking'      // check_for_update in flight
    | 'up_to_date'    // no update found
    | 'available'     // update info present, not yet installing
    | 'downloading'   // install_update in progress
    | 'done'          // finished — app will restart shortly
    | 'error';        // something went wrong

  let updateState  = $state<UpdateState>('idle');
  let updateInfo   = $state<UpdateInfo | null>(null);
  let updateError  = $state('');
  let dlReceived   = $state(0);
  let dlTotal      = $state(0);

  let dlPercent = $derived(
    dlTotal > 0 ? Math.round((dlReceived / dlTotal) * 100) : 0
  );

  async function checkForUpdate() {
    updateState = 'checking';
    updateError = '';
    try {
      const info = await invoke<UpdateInfo | null>('check_for_update');
      if (info) {
        updateInfo  = info;
        updateState = 'available';
      } else {
        updateState = 'up_to_date';
      }
    } catch (e) {
      updateError = String(e);
      updateState = 'error';
    }
  }

  async function installUpdate() {
    updateState = 'downloading';
    dlReceived  = 0;
    dlTotal     = 0;
    updateError = '';

    const onEvent = new Channel<DownloadEvent>();
    onEvent.onmessage = (ev) => {
      if (ev.event === 'Started') {
        dlTotal = ev.data.contentLength ?? 0;
      } else if (ev.event === 'Progress') {
        dlReceived += ev.data.chunkLength;
      } else if (ev.event === 'Finished') {
        updateState = 'done';
      }
    };

    try {
      await invoke('install_update', { onEvent });
    } catch (e) {
      updateError = String(e);
      updateState = 'error';
    }
  }

  onMount(async () => {
    try {
      [appVersion, appName] = await Promise.all([getVersion(), getName()]);
    } catch { /* ignore in dev */ }
    try {
      const status = await invoke<{ found: boolean; path: string | null; platform: string }>('detect_steamcmd');
      platform = status.platform;
    } catch { /* ignore */ }

    // Auto-check for updates on Windows
    if (platform === 'windows') {
      checkForUpdate();
    }
  });

  async function installSteamcmdWindows() {
    cmdDownloading = true;
    cmdDownloadErr = '';
    cmdDownloadOk  = false;
    try {
      await invoke('download_steamcmd_windows');
      cmdDownloadOk = true;
    } catch (e) {
      cmdDownloadErr = String(e);
    } finally {
      cmdDownloading = false;
    }
  }

  const AUTHOR   = 'Thoxy';
  const REPO_URL = 'https://git.thoxy.xyz/thoxy/dayz-community-hub';
</script>

<div class="h-full overflow-y-auto">
  <div class="max-w-2xl mx-auto px-6 py-8 space-y-8">

    <!-- ── Hero ─────────────────────────────────────────────────────────── -->
    <div class="relative rounded-2xl overflow-hidden border border-base-300/60 bg-base-200/40">
      <!-- Subtle gradient backdrop -->
      <div class="absolute inset-0 bg-gradient-to-br from-primary/8 via-transparent to-secondary/5 pointer-events-none"></div>

      <div class="relative px-6 py-6 flex items-center gap-5">
        <!-- App icon with glow ring -->
        <div class="relative shrink-0">
          <div class="absolute inset-0 rounded-2xl bg-primary/20 blur-xl scale-110 pointer-events-none"></div>
          <img src="/icon.svg" alt="DayZ Community Hub" class="relative w-16 h-16 rounded-2xl shadow-lg" />
        </div>

        <!-- Identity block -->
        <div class="flex-1 min-w-0">
          <h1 class="text-2xl font-bold text-base-content tracking-tight leading-tight">{appName}</h1>
          <p class="text-sm text-base-content/50 mt-0.5">Server browser &amp; mod manager for DayZ Standalone</p>

          <!-- Meta pills -->
          <div class="flex flex-wrap items-center gap-2 mt-3">
            {#if appVersion}
              <span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-primary/10 border border-primary/20 text-xs text-primary font-mono font-medium">
                <Icon icon="ph:tag" class="size-3" />
                v{appVersion}
              </span>
            {/if}
            <span class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-base-300/60 border border-base-300 text-xs text-base-content/50">
              <Icon icon="ph:user" class="size-3" />
              {AUTHOR}
            </span>
            <button
              class="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-base-300/60 border border-base-300 text-xs text-base-content/50 hover:bg-primary/10 hover:border-primary/30 hover:text-primary transition-all"
              onclick={() => openUrl(REPO_URL)}
              title="Source repository"
            >
              <Icon icon="catppuccin:forgejo" class="size-3" />
              Forgejo
            </button>
          </div>
        </div>
      </div>

      <!-- Feature highlights grid -->
      <div class="relative border-t border-base-300/50 grid grid-cols-2 divide-x divide-y divide-base-300/50 sm:grid-cols-4 sm:divide-y-0">
        {#each [
          { icon: 'mdi:server-network',   label: 'Server Browser',   color: 'text-sky-400'     },
          { icon: 'mdi:puzzle',           label: 'Mod Manager',       color: 'text-fuchsia-400' },
          { icon: 'ph:chart-line-up',     label: 'BattleMetrics',     color: 'text-emerald-400' },
          { icon: 'ph:rocket-launch',     label: 'One-click Launch',  color: 'text-orange-400'  },
        ] as feat}
          <div class="flex items-center gap-2 px-4 py-3">
            <Icon icon={feat.icon} class="size-4 {feat.color} shrink-0" />
            <span class="text-xs text-base-content/60 font-medium">{feat.label}</span>
          </div>
        {/each}
      </div>
    </div>

    <!-- ── Updates (Windows only) ──────────────────────────────────────── -->
    {#if platform === 'windows'}
      <section>
        <div class="flex items-center gap-2 mb-3">
          <Icon icon="ph:arrow-circle-up" class="size-4 text-primary shrink-0" />
          <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Updates</h2>
        </div>

        <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden">

          {#if updateState === 'idle' || updateState === 'checking'}
            <!-- Checking / idle -->
            <div class="flex items-center gap-3 px-4 py-3.5">
              <span class="loading loading-spinner loading-sm text-primary shrink-0"></span>
              <span class="text-sm text-base-content/60">Checking for updates…</span>
            </div>

          {:else if updateState === 'up_to_date'}
            <!-- Up to date -->
            <div class="flex items-center gap-3 px-4 py-3.5">
              <div class="size-8 rounded-lg bg-success/10 border border-success/20 flex items-center justify-center shrink-0">
                <Icon icon="ph:check-circle" class="size-4 text-success" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-semibold text-base-content">You're up to date</p>
                <p class="text-xs text-base-content/45 mt-0.5">v{appVersion} is the latest version</p>
              </div>
              <button
                class="btn btn-xs btn-ghost text-base-content/40 gap-1"
                onclick={checkForUpdate}
              >
                <Icon icon="ph:arrows-clockwise" class="size-3.5" />
                Re-check
              </button>
            </div>

          {:else if updateState === 'available'}
            <!-- Update available -->
            <div class="flex items-start gap-3 px-4 py-3.5 border-b border-base-300/40">
              <div class="size-8 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center shrink-0 mt-0.5">
                <Icon icon="ph:arrow-circle-up" class="size-4 text-primary" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-semibold text-base-content">
                  Update available —
                  <span class="text-primary font-mono">v{updateInfo?.version}</span>
                </p>
                <p class="text-xs text-base-content/45 mt-0.5">
                  Current: <span class="font-mono">v{updateInfo?.currentVersion}</span>
                  {#if updateInfo?.date}
                    · Released {new Date(updateInfo.date).toLocaleDateString()}
                  {/if}
                </p>
                {#if updateInfo?.body}
                  <p class="text-xs text-base-content/50 mt-2 leading-relaxed line-clamp-3">{updateInfo.body}</p>
                {/if}
              </div>
            </div>
            <div class="flex items-center justify-end gap-2 px-4 py-2.5 bg-base-200/60">
              <button
                class="btn btn-sm btn-primary gap-1.5"
                onclick={installUpdate}
              >
                <Icon icon="ph:download-simple" class="size-3.5" />
                Install update
              </button>
            </div>

          {:else if updateState === 'downloading'}
            <!-- Downloading -->
            <div class="px-4 py-3.5 space-y-2.5">
              <div class="flex items-center gap-2">
                <span class="loading loading-spinner loading-sm text-primary shrink-0"></span>
                <span class="text-sm font-semibold text-base-content">
                  Downloading v{updateInfo?.version}…
                </span>
                {#if dlTotal > 0}
                  <span class="ml-auto text-xs text-base-content/40 tabular-nums font-mono shrink-0">
                    {dlPercent}%
                  </span>
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
            <!-- Done — installer launching -->
            <div class="flex items-center gap-3 px-4 py-3.5">
              <div class="size-8 rounded-lg bg-success/10 border border-success/20 flex items-center justify-center shrink-0">
                <Icon icon="ph:check-circle" class="size-4 text-success" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-semibold text-base-content">Download complete</p>
                <p class="text-xs text-base-content/45 mt-0.5">The installer is launching — the app will restart automatically.</p>
              </div>
            </div>

          {:else if updateState === 'error'}
            <!-- Error -->
            <div class="px-4 py-3.5 space-y-2">
              <div class="flex items-start gap-2 text-error">
                <Icon icon="ph:warning-circle" class="size-4 shrink-0 mt-0.5" />
                <p class="text-xs leading-relaxed break-all">{updateError}</p>
              </div>
              <button
                class="btn btn-xs btn-ghost text-base-content/40 gap-1"
                onclick={checkForUpdate}
              >
                <Icon icon="ph:arrows-clockwise" class="size-3.5" />
                Retry
              </button>
            </div>
          {/if}

        </div>
      </section>
    {/if}

    <!-- ── Quick Start ──────────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:rocket-launch" class="size-4 text-primary shrink-0" />
        <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Quick Start</h2>
      </div>
      <div class="space-y-2">
        {#each [
          { n: 1, icon: 'ph:user-gear',        title: 'Open account settings',    body: 'Click your name (or "Set up account") in the title bar to configure your player name, Steam credentials, and API keys.' },
          { n: 2, icon: 'ph:game-controller',  title: 'Set your in-game name',    body: 'This is the character name passed as -name= when launching DayZ.' },
          { n: 3, icon: 'mdi:server-network',  title: 'Browse & connect',         body: 'Use the Servers tab to find a server. Double-click a row or click Connect to launch DayZ directly.' },
        ] as step}
          <div class="flex items-start gap-4 px-4 py-3 rounded-xl bg-base-200/50 border border-base-300/50 hover:border-primary/20 hover:bg-base-200/80 transition-colors">
            <span class="size-6 rounded-full bg-primary text-primary-content text-xs font-bold flex items-center justify-center shrink-0 mt-0.5 shadow-sm">{step.n}</span>
            <div class="flex-1 min-w-0">
              <p class="text-sm font-semibold text-base-content flex items-center gap-1.5">
                <Icon icon={step.icon} class="size-3.5 text-primary/70 shrink-0" />
                {step.title}
              </p>
              <p class="text-xs text-base-content/50 mt-0.5 leading-relaxed">{step.body}</p>
            </div>
          </div>
        {/each}
      </div>
    </section>

    <!-- ── Tabs ──────────────────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:tabs" class="size-4 text-primary shrink-0" />
        <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Tabs</h2>
      </div>
      <div class="grid grid-cols-1 gap-1.5">
        {#each [
          { icon: 'mdi:server-network',   label: 'Servers',  color: 'text-sky-400',     desc: 'Live public server list with search, filters (1P, password, BE, mods, map) and a sortable table. Click a row for live A2S data, BattleMetrics stats, and the mod list.' },
          { icon: 'ph:star',              label: 'Favorites',color: 'text-yellow-400',  desc: 'Starred servers for quick re-join. Shows live player counts, ping, map and in-game time when the server is online.' },
          { icon: 'ph:clock-clockwise',   label: 'History',  color: 'text-teal-400',    desc: 'Every server you have connected to, with relative timestamps and live stats if the server is still online.' },
          { icon: 'mdi:puzzle',           label: 'Mods',     color: 'text-fuchsia-400', desc: 'All installed Workshop mods. Check for updates via the Steam Workshop API, download, update, or clean up managed mods using SteamCMD.' },
          { icon: 'ph:newspaper',         label: 'News',     color: 'text-rose-400',    desc: 'Latest DayZ news articles fetched directly from the official DayZ website.' },
          { icon: 'ph:plugs-connected',   label: 'Connect',  color: 'text-indigo-400',  desc: 'Connect directly by IP:port without browsing the list. Supports game port and query port.' },
          { icon: 'ph:sliders',           label: 'Options',  color: 'text-orange-400',  desc: 'Toggle DayZ launch options such as -noPause and -filePatching. Changes apply on the next game launch.' },
          { icon: 'ph:mountains',         label: 'Offline',  color: 'text-emerald-400', desc: 'Download DayZ Community Offline Mode and launch solo missions without a server.' },
        ] as tab}
          <div class="flex items-start gap-3 px-4 py-2.5 rounded-lg bg-base-200/40 border border-base-300/40 hover:border-base-300/80 transition-colors">
            <Icon icon={tab.icon} class="size-4 {tab.color} shrink-0 mt-0.5" />
            <div class="flex-1 min-w-0">
              <span class="text-xs font-semibold text-base-content mr-2">{tab.label}</span>
              <span class="text-xs text-base-content/45 leading-relaxed">{tab.desc}</span>
            </div>
          </div>
        {/each}
      </div>
    </section>

    <!-- ── API Keys ───────────────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:key" class="size-4 text-primary shrink-0" />
        <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Optional API Keys</h2>
      </div>
      <div class="space-y-3">

        <!-- Steam API -->
        <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden">
          <div class="flex items-center gap-3 px-4 py-3 border-b border-base-300/40 bg-base-200/60">
            <Icon icon="ph:identification-card" class="size-4 text-sky-400 shrink-0" />
            <div class="flex-1 min-w-0">
              <p class="text-sm font-semibold text-base-content">Steam API Key &amp; Steam ID</p>
              <p class="text-xs text-base-content/40 mt-0.5">Avatar in title bar + mod update checks</p>
            </div>
            <span class="text-xs px-2 py-0.5 rounded-full bg-base-300/60 text-base-content/40 border border-base-300/60">optional</span>
          </div>
          <div class="px-4 py-3 space-y-2">
            <div class="flex items-start gap-2 text-xs text-base-content/55">
              <Icon icon="ph:user-circle" class="size-3.5 text-sky-400/60 shrink-0 mt-0.5" />
              <span><span class="font-semibold text-base-content/70">Steam avatar</span> — shows your Steam profile picture in the title bar. Requires both API key and your 64-bit Steam ID.</span>
            </div>
            <div class="flex items-start gap-2 text-xs text-base-content/55">
              <Icon icon="mdi:puzzle-outline" class="size-3.5 text-sky-400/60 shrink-0 mt-0.5" />
              <span><span class="font-semibold text-base-content/70">Mod update checks</span> — queries the Steam Workshop API for newer mod versions. Works without the key but at lower rate limits.</span>
            </div>
            <div class="flex flex-wrap gap-2 pt-1">
              <button
                class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-base-300/50 border border-base-300 text-xs text-primary hover:bg-primary/10 hover:border-primary/30 transition-all"
                onclick={() => openUrl('https://steamcommunity.com/dev/apikey')}
              >
                <Icon icon="mdi:steam" class="size-3.5" />
                steamcommunity.com/dev/apikey
              </button>
              <button
                class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-base-300/50 border border-base-300 text-xs text-primary hover:bg-primary/10 hover:border-primary/30 transition-all"
                onclick={() => openUrl('https://steamdb.info/calculator/')}
              >
                <Icon icon="ph:calculator" class="size-3.5" />
                Find your Steam ID
              </button>
            </div>
          </div>
        </div>

        <!-- BattleMetrics -->
        <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden">
          <div class="flex items-center gap-3 px-4 py-3 border-b border-base-300/40 bg-base-200/60">
            <Icon icon="ph:chart-line-up" class="size-4 text-emerald-400 shrink-0" />
            <div class="flex-1 min-w-0">
              <p class="text-sm font-semibold text-base-content">BattleMetrics API Token</p>
              <p class="text-xs text-base-content/40 mt-0.5">Server rankings, uptime &amp; 24 h player graph</p>
            </div>
            <span class="text-xs px-2 py-0.5 rounded-full bg-base-300/60 text-base-content/40 border border-base-300/60">optional</span>
          </div>
          <div class="px-4 py-3 space-y-2">
            <div class="grid grid-cols-2 gap-2">
              {#each [
                { icon: 'ph:trophy',        color: 'text-yellow-400/70', label: 'Global rank',     desc: 'Popularity relative to all DayZ servers' },
                { icon: 'ph:globe',         color: 'text-sky-400/70',    label: 'Country & status', desc: 'Location + BM online/offline indicator'  },
                { icon: 'ph:clock',         color: 'text-teal-400/70',   label: 'Uptime %',        desc: '30-day uptime percentage'                },
                { icon: 'ph:chart-line-up', color: 'text-emerald-400/70',label: 'Player graph',    desc: '24 h player count sparkline'             },
              ] as feat}
                <div class="flex items-start gap-2 px-3 py-2 rounded-lg bg-base-300/30 border border-base-300/40">
                  <Icon icon={feat.icon} class="size-3.5 {feat.color} shrink-0 mt-0.5" />
                  <div>
                    <p class="text-xs font-semibold text-base-content/70">{feat.label}</p>
                    <p class="text-xs text-base-content/40 leading-snug">{feat.desc}</p>
                  </div>
                </div>
              {/each}
            </div>
            <div class="flex pt-1">
              <button
                class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-base-300/50 border border-base-300 text-xs text-primary hover:bg-primary/10 hover:border-primary/30 transition-all"
                onclick={() => openUrl('https://www.battlemetrics.com/developers')}
              >
                <Icon icon="ph:chart-line-up" class="size-3.5" />
                battlemetrics.com/developers
              </button>
            </div>
          </div>
        </div>

      </div>
    </section>

    <!-- ── SteamCMD & Mods ───────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:terminal-window" class="size-4 text-primary shrink-0" />
        <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">SteamCMD &amp; Mods</h2>
      </div>
      <div class="rounded-xl border border-base-300/60 bg-base-200/40 divide-y divide-base-300/40">
        <div class="px-4 py-3">
          <p class="text-xs font-semibold text-base-content/70 mb-1.5">What is SteamCMD?</p>
          <p class="text-xs text-base-content/50 leading-relaxed">
            A command-line tool from Valve used to download Steam Workshop content without the full Steam client.
            The app uses it to install and update DayZ mods in the background.
            If the title bar shows <span class="text-warning/80 font-semibold">SteamCMD not found</span>,
            install it below or set the path manually in account settings.
          </p>

          <!-- Linux install instructions -->
          {#if platform === 'linux' || platform === ''}
            <div class="mt-2 space-y-1 text-xs font-mono">
              {#each [
                { label: 'Debian / Ubuntu', cmd: 'sudo apt install steamcmd'  },
                { label: 'Arch / Manjaro',  cmd: 'yay -S steamcmd'            },
                { label: 'Fedora / RHEL',   cmd: 'sudo dnf install steamcmd'  },
              ] as row}
                <div class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-base-300/40">
                  <Icon icon="simple-icons:linux" class="size-3.5 text-base-content/40 shrink-0" />
                  <span class="text-base-content/40 shrink-0 w-28">{row.label}</span>
                  <span class="text-primary ml-auto">{row.cmd}</span>
                </div>
              {/each}
            </div>
          {/if}

          <!-- Windows install button -->
          {#if platform === 'windows'}
            <div class="mt-3 space-y-2">
              <p class="text-xs text-base-content/50 leading-relaxed">
                SteamCMD will be downloaded from Valve and installed to
                <span class="font-mono text-base-content/70">%APPDATA%\dayz_community_hub\steamcmd\</span>.
              </p>
              {#if cmdDownloadOk}
                <div class="flex items-center gap-2 px-3 py-2 rounded-lg bg-success/10 border border-success/25 text-xs text-success">
                  <Icon icon="ph:check-circle" class="size-4 shrink-0" />
                  SteamCMD installed successfully.
                </div>
              {:else}
                {#if cmdDownloadErr}
                  <div class="flex items-start gap-2 px-3 py-2 rounded-lg bg-error/10 border border-error/25 text-xs text-error">
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
                    <span class="loading loading-spinner loading-xs"></span>
                    Downloading SteamCMD…
                  {:else}
                    <Icon icon="ph:download-simple" class="size-4" />
                    Install SteamCMD
                  {/if}
                </button>
              {/if}
            </div>
          {/if}
        </div>
        <div class="px-4 py-3">
          <p class="text-xs font-semibold text-base-content/70 mb-2">Mod workflow</p>
          <div class="space-y-2">
            {#each [
              'Connect to a server — the app offers to install any missing mods automatically.',
              'Go to the Mods tab and click Check updates to see which mods are outdated.',
              'Click Update N to update all stale mods in one batch, or update them individually.',
            ] as step, i}
              <div class="flex items-start gap-3 text-xs text-base-content/50">
                <span class="size-4 rounded-full bg-base-300/80 text-base-content/50 text-xs font-bold flex items-center justify-center shrink-0 mt-0.5">{i + 1}</span>
                <span class="leading-relaxed">{step}</span>
              </div>
            {/each}
          </div>
        </div>
      </div>
    </section>

    <!-- ── Tips ──────────────────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:lightbulb" class="size-4 text-primary shrink-0" />
        <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Tips &amp; Shortcuts</h2>
      </div>
      <div class="grid grid-cols-1 gap-1.5">
        {#each [
          { icon: 'ph:arrows-down-up',   tip: 'Click any column header in the server list to sort. Click again to reverse.' },
          { icon: 'ph:keyboard',         tip: 'Arrow keys navigate the server list; Enter connects to the selected server.' },
          { icon: 'ph:copy',             tip: 'Click any IP address to copy it to the clipboard instantly.' },
          { icon: 'ph:star',             tip: 'Star a server from the footer bar or Info panel to add it to Favorites.' },
          { icon: 'ph:info',             tip: 'The Info panel shows live A2S data: online players, live ping, and mod list.' },
          { icon: 'ph:chart-line-up',    tip: 'The Info panel also shows BattleMetrics rank, status, uptime %, and player history graph (requires API token).' },
          { icon: 'ph:sun',              tip: 'Toggle light / dark theme with the sun/moon button in the title bar.' },
        ] as item}
          <div class="flex items-start gap-3 px-3 py-2.5 rounded-lg bg-base-200/40 border border-base-300/40">
            <Icon icon={item.icon} class="size-3.5 text-primary/50 shrink-0 mt-0.5" />
            <p class="text-xs text-base-content/55 leading-relaxed">{item.tip}</p>
          </div>
        {/each}
      </div>
    </section>

    <!-- ── Profile Management ────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:archive" class="size-4 text-primary shrink-0" />
        <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Profile Management</h2>
      </div>
      <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden divide-y divide-base-300/40">

        <div class="flex items-center gap-4 px-4 py-3">
          <div class="size-8 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center shrink-0">
            <Icon icon="ph:upload-simple" class="size-4 text-primary" />
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-sm font-semibold text-base-content">Export</p>
            <p class="text-xs text-base-content/45 mt-0.5 leading-relaxed">
              Save all settings, favorites, history and mod list to a
              <span class="font-mono bg-base-300/60 px-1 rounded text-base-content/60">.dchub</span> file (zstd-compressed, portable between machines).
            </p>
          </div>
          <button class="btn btn-sm btn-primary shrink-0 gap-1.5" onclick={onExport}>
            <Icon icon="ph:upload-simple" class="size-3.5" />
            Export
          </button>
        </div>

        <div class="flex items-center gap-4 px-4 py-3">
          <div class="size-8 rounded-lg bg-base-300/60 border border-base-300 flex items-center justify-center shrink-0">
            <Icon icon="ph:download-simple" class="size-4 text-base-content/50" />
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-sm font-semibold text-base-content">Import</p>
            <p class="text-xs text-base-content/45 mt-0.5 leading-relaxed">
              Restore a previously exported
              <span class="font-mono bg-base-300/60 px-1 rounded text-base-content/60">.dchub</span> file. Overwrites current profile and mod list.
            </p>
          </div>
          <button class="btn btn-sm btn-ghost shrink-0 gap-1.5" onclick={onImport}>
            <Icon icon="ph:download-simple" class="size-3.5" />
            Import
          </button>
        </div>

        <div class="flex items-center gap-4 px-4 py-3">
          <div class="size-8 rounded-lg bg-error/10 border border-error/20 flex items-center justify-center shrink-0">
            <Icon icon="ph:arrow-counter-clockwise" class="size-4 text-error/70" />
          </div>
          <div class="flex-1 min-w-0">
            <p class="text-sm font-semibold text-error/80">Reset to defaults</p>
            <p class="text-xs text-base-content/45 mt-0.5 leading-relaxed">
              Wipe profile back to factory defaults — clears all settings, favorites, history and launch options. Installed mods on disk are not affected.
            </p>
          </div>
          <button class="btn btn-sm btn-error btn-outline shrink-0 gap-1.5" onclick={onReset}>
            <Icon icon="ph:arrow-counter-clockwise" class="size-3.5" />
            Reset
          </button>
        </div>

      </div>
    </section>

    <!-- ── Built with ────────────────────────────────────────────────────── -->
    <section>
      <div class="flex items-center gap-2 mb-3">
        <Icon icon="ph:code" class="size-4 text-primary shrink-0" />
        <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">Built with</h2>
      </div>
      <div class="flex flex-wrap gap-2">
        {#each [
          { icon: 'simple-icons:tauri',   label: 'Tauri 2',     url: 'https://tauri.app',         color: 'text-sky-400'     },
          { icon: 'simple-icons:svelte',  label: 'Svelte 5',    url: 'https://svelte.dev',        color: 'text-orange-400'  },
          { icon: 'simple-icons:rust',    label: 'Rust',        url: 'https://www.rust-lang.org', color: 'text-orange-600'  },
          { icon: 'simple-icons:daisyui', label: 'DaisyUI 5',   url: 'https://daisyui.com',       color: 'text-violet-400'  },
          { icon: 'simple-icons:tailwindcss', label: 'Tailwind', url: 'https://tailwindcss.com', color: 'text-cyan-400'    },
        ] as tech}
          <button
            class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-base-200/60 border border-base-300/60 text-xs text-base-content/50 hover:bg-base-200 hover:border-base-300 hover:text-base-content/80 transition-all"
            onclick={() => openUrl(tech.url)}
          >
            <Icon icon={tech.icon} class="size-3.5 {tech.color}" />
            {tech.label}
          </button>
        {/each}
      </div>
    </section>

    <!-- ── Footer ────────────────────────────────────────────────────────── -->
    <div class="flex items-center justify-between pt-2 pb-4 border-t border-base-300/40 text-xs text-base-content/25">
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
