<script lang="ts">
  import Icon from '@iconify/svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { onDestroy } from 'svelte';

  interface Props {
    onDone: () => void;
  }

  let { onDone }: Props = $props();

  // ── Steps ──────────────────────────────────────────────────────────────────
  type Step = 'welcome' | 'identity' | 'steam' | 'done';
  const STEPS: Step[] = ['welcome', 'identity', 'steam', 'done'];

  let currentStep = $state<Step>('welcome');
  let saving = $state(false);
  let saveError = $state('');

  // ── Form fields ────────────────────────────────────────────────────────────
  let playerName   = $state('');
  let steamRoot    = $state('');
  let steamLogin   = $state('');
  let steamPass    = $state('');
  let showPass     = $state(false);
  let steamApiKey         = $state('');
  let steamId             = $state('');
  let steamcmdPath        = $state('');
  let battlemetricsApiKey = $state('');

  // ── SteamCMD detection state ────────────────────────────────────────────────
  type SteamcmdStatus = { found: boolean; path: string | null; platform: string };
  let steamcmdStatus   = $state<SteamcmdStatus | null>(null);
  let detectingCmd     = $state(false);
  let downloadingCmd   = $state(false);
  let downloadError    = $state('');
  let rescanInterval: ReturnType<typeof setInterval> | null = null;

  // ── Navigation ─────────────────────────────────────────────────────────────
  function stepIndex(s: Step) { return STEPS.indexOf(s); }
  let currentIndex = $derived(stepIndex(currentStep));

  async function next() {
    const idx = stepIndex(currentStep);
    if (idx < STEPS.length - 1) {
      const nextStep = STEPS[idx + 1];
      currentStep = nextStep;
      if (nextStep === 'steam') {
        await detectSteamcmd();
      }
    }
  }

  function back() {
    const idx = stepIndex(currentStep);
    if (idx > 0) {
      currentStep = STEPS[idx - 1];
      stopRescan();
    }
  }

  // ── SteamCMD detection ─────────────────────────────────────────────────────
  async function detectSteamcmd() {
    detectingCmd = true;
    downloadError = '';
    try {
      const status = await invoke<SteamcmdStatus>('detect_steamcmd');
      steamcmdStatus = status;
      if (status.found && status.path) {
        steamcmdPath = status.path;
        stopRescan();
      } else if (status.platform === 'linux') {
        startRescan();
      }
    } catch (e) {
      steamcmdStatus = { found: false, path: null, platform: 'linux' };
    } finally {
      detectingCmd = false;
    }
  }

  function startRescan() {
    stopRescan();
    rescanInterval = setInterval(async () => {
      try {
        const status = await invoke<SteamcmdStatus>('detect_steamcmd');
        steamcmdStatus = status;
        if (status.found && status.path) {
          steamcmdPath = status.path;
          stopRescan();
        }
      } catch { /* ignore */ }
    }, 5000);
  }

  function stopRescan() {
    if (rescanInterval !== null) {
      clearInterval(rescanInterval);
      rescanInterval = null;
    }
  }

  onDestroy(stopRescan);

  // ── Windows: download SteamCMD ─────────────────────────────────────────────
  async function downloadSteamcmd() {
    downloadingCmd = true;
    downloadError = '';
    try {
      const path = await invoke<string>('download_steamcmd_windows');
      steamcmdPath = path;
      steamcmdStatus = { found: true, path, platform: 'windows' };
    } catch (e) {
      downloadError = String(e);
    } finally {
      downloadingCmd = false;
    }
  }

  // ── Browse helpers ─────────────────────────────────────────────────────────
  async function browseSteamRoot() {
    const selected = await openDialog({ directory: true, multiple: false, title: 'Select Steam root folder' });
    if (selected) steamRoot = typeof selected === 'string' ? selected : selected[0];
  }

  async function browseSteamcmd() {
    const selected = await openDialog({ multiple: false, title: 'Select steamcmd binary' });
    if (selected) {
      steamcmdPath = typeof selected === 'string' ? selected : selected[0];
      steamcmdStatus = { found: true, path: steamcmdPath, platform: steamcmdStatus?.platform ?? 'linux' };
      stopRescan();
    }
  }

  // ── Import profile ─────────────────────────────────────────────────────────
  let importing = $state(false);
  let importError = $state('');

  async function importProfile() {
    importing = true;
    importError = '';
    try {
      const selected = await openDialog({
        multiple: false,
        title: 'Import DayZ Community Hub profile',
        filters: [{ name: 'DayZ Community Hub profile', extensions: ['dchub'] }],
      });
      if (!selected) { importing = false; return; }
      const path = typeof selected === 'string' ? selected : selected[0];
      await invoke('import_profile', { path });
      // Profile loaded — skip wizard and launch directly
      onDone();
    } catch (e) {
      importError = String(e);
      importing = false;
    }
  }

  // ── Save & finish ──────────────────────────────────────────────────────────
  async function finish() {
    saving = true;
    saveError = '';
    try {
      await invoke('save_profile_settings', {
        player:                playerName.trim()            || null,
        steamLogin:            steamLogin.trim()            || null,
        steamPassword:         steamPass                    || null,
        steamRoot:             steamRoot.trim()             || null,
        steamcmdPath:          steamcmdPath.trim()          || null,
        steamApiKey:           steamApiKey.trim()           || null,
        steamId:               steamId.trim()               || null,
        battlemetricsApiKey:   battlemetricsApiKey.trim()   || null,
        steamcmdEnabled: true,
      });
      onDone();
    } catch (e) {
      saveError = String(e);
      saving = false;
    }
  }

  // ── Derived helpers ────────────────────────────────────────────────────────
  let isWindows = $derived(steamcmdStatus?.platform === 'windows');
  let isLinux   = $derived(steamcmdStatus?.platform === 'linux');
  let cmdFound  = $derived(steamcmdStatus?.found === true);
</script>

<!-- Full-screen overlay -->
<div class="fixed inset-0 z-50 bg-base-300/80 backdrop-blur-sm flex items-center justify-center p-4">
  <div class="w-full max-w-lg bg-base-100 rounded-2xl shadow-2xl border border-base-300 overflow-hidden flex flex-col">

    <!-- Progress bar -->
    <div class="h-1 bg-base-300">
      <div
        class="h-full bg-primary transition-all duration-500 ease-out"
        style="width: {((currentIndex) / (STEPS.length - 1)) * 100}%"
      ></div>
    </div>

    <!-- Step indicators -->
    <div class="flex items-center justify-center gap-2 pt-5 pb-1 px-6">
      {#each STEPS as step, i}
        <div class="flex items-center gap-2">
          <div class="size-6 rounded-full flex items-center justify-center text-xs font-bold transition-all
            {i < currentIndex ? 'bg-primary text-primary-content' :
             i === currentIndex ? 'bg-primary text-primary-content ring-2 ring-primary ring-offset-2 ring-offset-base-100' :
             'bg-base-300 text-base-content/40'}">
            {#if i < currentIndex}
              <Icon icon="ph:check-bold" class="size-3" />
            {:else}
              {i + 1}
            {/if}
          </div>
          {#if i < STEPS.length - 1}
            <div class="w-8 h-px {i < currentIndex ? 'bg-primary' : 'bg-base-300'} transition-colors"></div>
          {/if}
        </div>
      {/each}
    </div>

    <!-- Step content -->
    <div class="flex-1 px-8 py-6 overflow-y-auto">

      <!-- ── Welcome ───────────────────────────────────────────────────────── -->
      {#if currentStep === 'welcome'}
        <div class="text-center space-y-4">
          <img src="/icon.svg" alt="DayZ Community Hub" class="w-16 h-16 mx-auto" />
          <div>
            <h1 class="text-xl font-bold text-base-content tracking-tight">Welcome to DayZ Community Hub</h1>
            <p class="text-sm text-base-content/50 mt-1">A server browser and mod manager for DayZ Standalone</p>
          </div>
          <div class="bg-base-200/60 rounded-xl border border-base-300/60 text-left divide-y divide-base-300/50 mt-4">
            {#each [
              { icon: 'ph:magnifying-glass', text: 'Browse thousands of live servers' },
              { icon: 'ph:puzzle-piece', text: 'Manage and update your mods via SteamCMD' },
              { icon: 'ph:star', text: 'Keep favorites and connection history' },
              { icon: 'ph:rocket-launch', text: 'Launch DayZ directly with one click' },
            ] as item}
              <div class="flex items-center gap-3 px-4 py-2.5">
                <Icon icon={item.icon} class="size-4 text-primary shrink-0" />
                <span class="text-sm text-base-content/70">{item.text}</span>
              </div>
            {/each}
          </div>
          <p class="text-xs text-base-content/40 pt-2">
            This quick setup takes about 1 minute. You can change everything later in account settings.
          </p>
        </div>

      <!-- ── Identity ──────────────────────────────────────────────────────── -->
      {:else if currentStep === 'identity'}
        <div class="space-y-5">
          <div>
            <h2 class="text-base font-semibold text-base-content">Your identity</h2>
            <p class="text-xs text-base-content/50 mt-0.5">Used when launching DayZ and finding your Steam profile.</p>
          </div>

          <!-- Player name -->
          <div class="form-control">
            <label class="label py-0 pb-1.5" for="wiz-name">
              <span class="label-text text-xs flex items-center gap-1.5">
                <Icon icon="ph:game-controller" class="size-3.5 text-base-content/40" />
                In-game name
                <span class="text-base-content/30 ml-1">recommended</span>
              </span>
            </label>
            <input
              id="wiz-name"
              type="text"
              class="input input-bordered input-sm"
              placeholder="e.g. Survivor"
              bind:value={playerName}
            />
            <p class="label py-0 pt-1">
              <span class="label-text-alt text-base-content/40">Passed as <span class="font-mono">-name=</span> to DayZ.</span>
            </p>
          </div>

          <!-- Steam root -->
          <div class="form-control">
            <label class="label py-0 pb-1.5" for="wiz-steam-root">
              <span class="label-text text-xs flex items-center gap-1.5">
                <Icon icon="mdi:steam" class="size-3.5 text-base-content/40" />
                Steam root folder
                <span class="text-base-content/30 ml-1">recommended</span>
              </span>
            </label>
            <div class="flex gap-2">
              <input
                id="wiz-steam-root"
                type="text"
                class="input input-bordered input-sm flex-1 font-mono text-xs"
                placeholder="Auto-detect"
                bind:value={steamRoot}
              />
              <button class="btn btn-ghost btn-sm btn-square" onclick={browseSteamRoot} title="Browse">
                <Icon icon="ph:folder-open" class="size-4" />
              </button>
            </div>
            <p class="label py-0 pt-1">
              <span class="label-text-alt text-base-content/40">
                Linux: <span class="font-mono">~/.steam/steam</span> —
                Windows: <span class="font-mono">C:\Program Files (x86)\Steam</span>
              </span>
            </p>
          </div>
        </div>

      <!-- ── Steam ─────────────────────────────────────────────────────────── -->
      {:else if currentStep === 'steam'}
        <div class="space-y-5">
          <div>
            <h2 class="text-base font-semibold text-base-content">Steam &amp; SteamCMD</h2>
            <p class="text-xs text-base-content/50 mt-0.5">All optional — enables mod downloads, avatar, and update checks.</p>
          </div>

          <!-- ── SteamCMD detection banner ── -->
          <div class="rounded-xl border overflow-hidden
            {cmdFound ? 'border-success/40 bg-success/8' : 'border-warning/40 bg-warning/8'}">

            <!-- Header row -->
            <div class="flex items-center gap-2 px-4 py-2.5 border-b
              {cmdFound ? 'border-success/20' : 'border-warning/20'}">
              {#if detectingCmd}
                <span class="loading loading-spinner loading-xs text-base-content/40"></span>
                <span class="text-xs text-base-content/50">Detecting SteamCMD…</span>
              {:else if cmdFound}
                <Icon icon="ph:check-circle" class="size-4 text-success shrink-0" />
                <span class="text-xs font-semibold text-success">SteamCMD detected</span>
                <span class="ml-auto font-mono text-xs text-base-content/40 truncate max-w-[180px]">{steamcmdStatus?.path}</span>
              {:else}
                <Icon icon="ph:warning" class="size-4 text-warning shrink-0" />
                <span class="text-xs font-semibold text-warning">SteamCMD not found</span>
                <button
                  class="btn btn-ghost btn-xs ml-auto gap-1"
                  onclick={detectSteamcmd}
                  disabled={detectingCmd}
                >
                  <Icon icon="ph:arrow-clockwise" class="size-3.5" />
                  Rescan
                </button>
              {/if}
            </div>

            <!-- Windows: auto-download CTA -->
            {#if isWindows && !cmdFound}
              <div class="px-4 py-3 space-y-2">
                <p class="text-xs text-base-content/60">
                  SteamCMD will be downloaded from Valve and installed to
                  <span class="font-mono text-base-content/80">%APPDATA%\dayz_community_hub\steamcmd\</span>
                </p>
                {#if downloadError}
                  <div class="flex items-start gap-2 rounded-lg bg-error/10 border border-error/25 px-3 py-2 text-xs text-error">
                    <Icon icon="ph:warning-circle" class="size-4 shrink-0 mt-0.5" />
                    <span>{downloadError}</span>
                  </div>
                {/if}
                <button
                  class="btn btn-primary btn-sm w-full gap-2"
                  onclick={downloadSteamcmd}
                  disabled={downloadingCmd}
                >
                  {#if downloadingCmd}
                    <span class="loading loading-spinner loading-xs"></span>
                    Downloading…
                  {:else}
                    <Icon icon="ph:download-simple" class="size-4" />
                    Download SteamCMD
                  {/if}
                </button>
              </div>

            <!-- Linux: install instructions + live rescan -->
            {:else if isLinux && !cmdFound}
              <div class="px-4 py-3 space-y-2">
                <p class="text-xs text-base-content/60">Install SteamCMD using your package manager, then we'll detect it automatically.</p>
                <div class="bg-base-300/60 rounded-lg divide-y divide-base-300/50 text-xs font-mono">
                  {#each [
                    { label: 'Debian / Ubuntu', cmd: 'sudo apt install steamcmd' },
                    { label: 'Arch / Manjaro',  cmd: 'yay -S steamcmd' },
                    { label: 'Fedora / RHEL',   cmd: 'sudo dnf install steamcmd' },
                  ] as row}
                    <div class="flex items-center gap-2 px-3 py-2">
                      <span class="text-base-content/40 shrink-0 w-28">{row.label}</span>
                      <span class="text-primary">{row.cmd}</span>
                    </div>
                  {/each}
                </div>
                <div class="flex items-center gap-2 text-xs text-base-content/40">
                  <span class="loading loading-dots loading-xs"></span>
                  Scanning every 5 seconds…
                </div>
              </div>
            {/if}
          </div>

          <!-- Steam login -->
          <div class="bg-base-200/50 rounded-xl border border-base-300/60 p-4 space-y-3">
            <p class="text-xs font-semibold text-base-content/60 uppercase tracking-wide flex items-center gap-1.5">
              <Icon icon="mdi:steam" class="size-3.5" />
              SteamCMD login
            </p>
            <div class="grid grid-cols-2 gap-3">
              <div class="form-control">
                <label class="label py-0 pb-1" for="wiz-login">
                  <span class="label-text text-xs text-base-content/50">Username</span>
                </label>
                <input id="wiz-login" type="text" class="input input-bordered input-xs font-mono" placeholder="anonymous" bind:value={steamLogin} />
              </div>
              <div class="form-control">
                <label class="label py-0 pb-1" for="wiz-pass">
                  <span class="label-text text-xs text-base-content/50">Password</span>
                </label>
                <div class="relative">
                  <input
                    id="wiz-pass"
                    type={showPass ? 'text' : 'password'}
                    class="input input-bordered input-xs w-full pr-7"
                    placeholder="leave blank if anonymous"
                    bind:value={steamPass}
                  />
                  <button
                    type="button"
                    class="absolute right-2 top-1/2 -translate-y-1/2 text-base-content/40 hover:text-base-content"
                    onclick={() => (showPass = !showPass)}
                  >
                    <Icon icon={showPass ? 'ph:eye-slash' : 'ph:eye'} class="size-3.5" />
                  </button>
                </div>
              </div>
            </div>
            <div class="flex items-start gap-1.5 text-xs text-warning/70">
              <Icon icon="ph:warning" class="size-3 shrink-0 mt-0.5" />
              <span>Password stored in plaintext. Prefer anonymous login or cached credentials.</span>
            </div>
          </div>

          <!-- API key + Steam ID -->
          <div class="bg-base-200/50 rounded-xl border border-base-300/60 p-4 space-y-3">
            <p class="text-xs font-semibold text-base-content/60 uppercase tracking-wide flex items-center gap-1.5">
              <Icon icon="ph:identification-card" class="size-3.5" />
              Steam API Key &amp; ID
            </p>
            <div class="form-control">
              <label class="label py-0 pb-1" for="wiz-apikey">
                <span class="label-text text-xs text-base-content/50 flex items-center gap-1">
                  API Key
                  <button class="text-primary hover:underline ml-1" onclick={() => openUrl('https://steamcommunity.com/dev/apikey')}>steamcommunity.com/dev/apikey</button>
                </span>
              </label>
              <input id="wiz-apikey" type="text" class="input input-bordered input-xs font-mono" placeholder="32-character hex key" bind:value={steamApiKey} />
            </div>
            <div class="form-control">
              <label class="label py-0 pb-1" for="wiz-steamid">
                <span class="label-text text-xs text-base-content/50 flex items-center gap-1">
                  Steam ID (64-bit)
                  <button class="text-primary hover:underline ml-1" onclick={() => openUrl('https://steamdb.info/calculator/')}>steamdb.info/calculator</button>
                </span>
              </label>
              <input id="wiz-steamid" type="text" class="input input-bordered input-xs font-mono" placeholder="76561198…" bind:value={steamId} />
            </div>
          </div>

          <!-- BattleMetrics API token -->
          <div class="bg-base-200/50 rounded-xl border border-base-300/60 p-4 space-y-3">
            <div class="flex items-center justify-between">
              <p class="text-xs font-semibold text-base-content/60 uppercase tracking-wide flex items-center gap-1.5">
                <Icon icon="ph:chart-line-up" class="size-3.5" />
                BattleMetrics
                <span class="font-normal normal-case tracking-normal text-base-content/35 ml-1">optional</span>
              </p>
            </div>
            <p class="text-xs text-base-content/50 leading-relaxed">
              Adds server rankings, status, uptime %, and a 24 h player count graph to every server detail panel.
            </p>
            <div class="form-control">
              <label class="label py-0 pb-1" for="wiz-bmkey">
                <span class="label-text text-xs text-base-content/50 flex items-center gap-1">
                  Personal access token
                  <button class="text-primary hover:underline ml-1" onclick={() => openUrl('https://www.battlemetrics.com/developers')}>battlemetrics.com/developers</button>
                </span>
              </label>
              <input id="wiz-bmkey" type="password" class="input input-bordered input-xs font-mono" placeholder="eyJhbGci…" bind:value={battlemetricsApiKey} />
            </div>
          </div>

          <!-- SteamCMD path (manual override) -->
          <div class="form-control">
            <label class="label py-0 pb-1.5" for="wiz-steamcmd">
              <span class="label-text text-xs flex items-center gap-1.5">
                <Icon icon="ph:terminal-window" class="size-3.5 text-base-content/40" />
                SteamCMD path
                <span class="text-base-content/30 ml-1">override detected path</span>
              </span>
            </label>
            <div class="flex gap-2">
              <input
                id="wiz-steamcmd"
                type="text"
                class="input input-bordered input-sm flex-1 font-mono text-xs"
                placeholder={cmdFound ? (steamcmdStatus?.path ?? 'Auto-detected') : 'Not found — browse or install above'}
                bind:value={steamcmdPath}
              />
              <button class="btn btn-ghost btn-sm btn-square" onclick={browseSteamcmd} title="Browse">
                <Icon icon="ph:folder-open" class="size-4" />
              </button>
            </div>
          </div>
        </div>

      <!-- ── Done ──────────────────────────────────────────────────────────── -->
      {:else if currentStep === 'done'}
        <div class="text-center space-y-4 py-4">
          <div class="size-16 rounded-full bg-success/15 flex items-center justify-center mx-auto">
            <Icon icon="ph:check-circle" class="size-9 text-success" />
          </div>
          <div>
            <h2 class="text-lg font-bold text-base-content">You're all set!</h2>
            <p class="text-sm text-base-content/50 mt-1">Your profile has been saved. Let's go find some servers.</p>
          </div>
          <div class="bg-base-200/60 rounded-xl border border-base-300/60 text-left divide-y divide-base-300/50">
            <div class="flex items-center gap-3 px-4 py-2.5">
              <Icon icon="ph:info" class="size-4 text-base-content/40 shrink-0" />
              <span class="text-xs text-base-content/50">You can update any of these settings later by clicking your name in the title bar.</span>
            </div>
            <div class="flex items-center gap-3 px-4 py-2.5">
              <Icon icon="ph:book-open" class="size-4 text-base-content/40 shrink-0" />
              <span class="text-xs text-base-content/50">Check the <span class="font-semibold text-base-content/60">About</span> tab for tips and documentation.</span>
            </div>
          </div>
          {#if saveError}
            <div class="flex items-start gap-2 px-3 py-2 rounded-lg bg-error/10 border border-error/25 text-xs text-error text-left">
              <Icon icon="ph:warning-circle" class="size-4 shrink-0 mt-0.5" />
              <span>{saveError}</span>
            </div>
          {/if}
        </div>
      {/if}

    </div>

    <!-- Footer nav -->
    <div class="flex items-center justify-between px-8 py-4 border-t border-base-300 bg-base-200/40">

      <!-- Back / step label / import -->
      <div class="flex items-center gap-3">
        {#if currentStep !== 'welcome' && currentStep !== 'done'}
          <button class="btn btn-ghost btn-sm gap-1.5" onclick={back}>
            <Icon icon="ph:arrow-left" class="size-4" />
            Back
          </button>
        {/if}
        {#if currentStep === 'welcome'}
          <button
            class="btn btn-ghost btn-sm gap-1.5 text-base-content/50"
            onclick={importProfile}
            disabled={importing}
          >
            {#if importing}
              <span class="loading loading-spinner loading-xs"></span>
            {:else}
              <Icon icon="ph:upload-simple" class="size-4" />
            {/if}
            Import profile
          </button>
          {#if importError}
            <span class="text-xs text-error">{importError}</span>
          {/if}
        {:else}
          <span class="text-xs text-base-content/30 capitalize">{currentStep}</span>
        {/if}
      </div>

      <!-- Next / Finish -->
      <div class="flex items-center gap-2">
        {#if currentStep === 'welcome'}
          <button class="btn btn-primary btn-sm gap-1.5" onclick={next}>
            Get started
            <Icon icon="ph:arrow-right" class="size-4" />
          </button>

        {:else if currentStep === 'identity'}
          <button class="btn btn-ghost btn-sm text-base-content/40" onclick={next}>Skip</button>
          <button class="btn btn-primary btn-sm gap-1.5" onclick={next}>
            Next
            <Icon icon="ph:arrow-right" class="size-4" />
          </button>

        {:else if currentStep === 'steam'}
          <button class="btn btn-ghost btn-sm text-base-content/40" onclick={() => { currentStep = 'done'; finish(); }}>Skip</button>
          <button class="btn btn-primary btn-sm gap-1.5" onclick={() => { currentStep = 'done'; finish(); }}>
            Save &amp; continue
            <Icon icon="ph:arrow-right" class="size-4" />
          </button>

        {:else if currentStep === 'done'}
          <button class="btn btn-success btn-sm gap-1.5" onclick={onDone} disabled={saving}>
            {#if saving}
              <span class="loading loading-spinner loading-xs"></span>
            {:else}
              <Icon icon="ph:rocket-launch" class="size-4" />
            {/if}
            Launch app
          </button>
        {/if}
      </div>

    </div>
  </div>
</div>
