<script lang="ts">
  import Icon from '@iconify/svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { onDestroy, onMount } from 'svelte';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    onDone: () => void;
  }

  let { onDone }: Props = $props();

  // ── Steps ──────────────────────────────────────────────────────────────────
  // 1. welcome   — intro + import option
  // 2. steamcmd  — detect / install SteamCMD
  // 3. configure — Steam login, API keys, identity, BattleMetrics
  // 4. done      — summary, launch button
  type Step = 'welcome' | 'steamcmd' | 'configure' | 'done';
  const STEPS: Step[] = ['welcome', 'steamcmd', 'configure', 'done'];

  let currentStep = $state<Step>('welcome');
  let saving = $state(false);
  let saveError = $state('');

  // ── Form fields ────────────────────────────────────────────────────────────
  let playerName = $state('');
  let steamRoot = $state('');
  let steamLogin = $state('');
  let steamPass = $state('');
  let showPass = $state(false);
  let steamApiKey = $state('');
  let steamId = $state('');
  let steamcmdPath = $state('');
  let battlemetricsApiKey = $state('');

  // ── SteamCMD detection state ────────────────────────────────────────────────
  type SteamcmdStatus = { found: boolean; path: string | null; platform: string };
  let steamcmdStatus = $state<SteamcmdStatus | null>(null);
  let detectingCmd = $state(false);
  let downloadingCmd = $state(false);
  let downloadError = $state('');
  let unlistenSteamcmd: (() => void) | null = null;

  // ── Validation ─────────────────────────────────────────────────────────────
  let steamLoginValid = $derived(steamLogin.trim().length > 0);

  // ── Navigation ─────────────────────────────────────────────────────────────
  function stepIndex(s: Step) {
    return STEPS.indexOf(s);
  }
  let currentIndex = $derived(stepIndex(currentStep));

  async function next() {
    const idx = stepIndex(currentStep);
    if (idx < STEPS.length - 1) {
      const nextStep = STEPS[idx + 1];
      currentStep = nextStep;
      if (nextStep === 'steamcmd') {
        await detectSteamcmd();
      }
    }
  }

  function back() {
    const idx = stepIndex(currentStep);
    if (idx > 0) {
      currentStep = STEPS[idx - 1];
      // Keep rescan running when going back from configure to steamcmd
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
      startRescan();
    } finally {
      detectingCmd = false;
    }
  }

  function startRescan() {
    stopRescan();
    listen<SteamcmdStatus>('steamcmd-detected', ({ payload }) => {
      steamcmdStatus = payload;
      if (payload.found && payload.path) {
        steamcmdPath = payload.path;
        stopRescan();
      }
    }).then((fn) => {
      unlistenSteamcmd = fn;
    });
    invoke('watch_steamcmd').catch(() => {});
  }

  function stopRescan() {
    if (unlistenSteamcmd) {
      unlistenSteamcmd();
      unlistenSteamcmd = null;
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
    const selected = await openDialog({ directory: true, multiple: false, title: m.wizard_select_steam_root() });
    if (selected) steamRoot = typeof selected === 'string' ? selected : selected[0];
  }

  async function browseSteamcmd() {
    const selected = await openDialog({
      multiple: false,
      title: m.wizard_select_steamcmd(),
      filters: isWindows ? [{ name: m.wizard_steamcmd_filter(), extensions: ['exe'] }] : [],
    });
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
        title: m.wizard_import_dialog_title(),
        filters: [{ name: m.wizard_import_filter(), extensions: ['dchub'] }],
      });
      if (!selected) {
        importing = false;
        return;
      }
      const path = typeof selected === 'string' ? selected : selected[0];
      await invoke('import_profile', { path });
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
        player: playerName.trim() || null,
        steamLogin: steamLogin.trim() || null,
        steamPassword: steamPass || null,
        steamRoot: steamRoot.trim() || null,
        steamcmdPath: steamcmdPath.trim() || null,
        steamApiKey: steamApiKey.trim() || null,
        steamId: steamId.trim() || null,
        battlemetricsApiKey: battlemetricsApiKey.trim() || null,
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
  let isLinux = $derived(steamcmdStatus?.platform === 'linux');
  let cmdFound = $derived(steamcmdStatus?.found === true);
</script>

<!-- Overlay starts below the titlebar (top: 36px = h-9) so the titlebar
     remains fully interactive (drag, minimize, maximize, close) during setup. -->
<div
  class="fixed inset-0 modal-backdrop-window z-50 bg-base-300/80 backdrop-blur-sm flex items-center justify-center p-4"
  style="top: 36px;"
>
  <div
    class="w-full max-w-lg bg-base-100 rounded-2xl shadow-2xl border border-base-300 overflow-hidden flex flex-col max-h-[calc(100vh-36px-2rem)]"
  >
    <!-- Progress bar -->
    <div class="h-1 bg-base-300 shrink-0">
      <div
        class="h-full bg-primary transition-all duration-500 ease-out"
        style="width: {(currentIndex / (STEPS.length - 1)) * 100}%"
      ></div>
    </div>

    <!-- Step indicators -->
    <div class="flex items-center justify-center gap-2 pt-5 pb-1 px-6 shrink-0">
      {#each STEPS as step, i}
        <div class="flex items-center gap-2">
          <div
            class="size-6 rounded-full flex items-center justify-center text-xs font-bold transition-all
            {i < currentIndex
              ? 'bg-primary text-primary-content'
              : i === currentIndex
                ? 'bg-primary text-primary-content ring-2 ring-primary ring-offset-2 ring-offset-base-100'
                : 'bg-base-300 text-base-content/40'}"
          >
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
    <div class="flex-1 px-8 py-6 overflow-y-auto min-h-0">
      <!-- ── Step 1: Welcome ─────────────────────────────────────────────── -->
      {#if currentStep === 'welcome'}
        <div class="text-center space-y-4">
          <img src="/icon.svg" alt="DayZ Community Hub" class="w-16 h-16 mx-auto" />
          <div>
            <h1 class="text-xl font-bold text-base-content tracking-tight">{m.wizard_welcome_title()}</h1>
            <p class="text-sm text-base-content/50 mt-1">{m.wizard_welcome_subtitle()}</p>
          </div>
          <div class="bg-base-200/60 rounded-xl border border-base-300/60 text-left divide-y divide-base-300/50 mt-4">
            {#each [{ icon: 'ph:magnifying-glass', textFn: () => m.wizard_welcome_feat1() }, { icon: 'ph:puzzle-piece', textFn: () => m.wizard_welcome_feat2() }, { icon: 'ph:star', textFn: () => m.wizard_welcome_feat3() }, { icon: 'ph:rocket-launch', textFn: () => m.wizard_welcome_feat4() }] as item}
              <div class="flex items-center gap-3 px-4 py-2.5">
                <Icon icon={item.icon} class="size-4 text-primary shrink-0" />
                <span class="text-sm text-base-content/70">{item.textFn()}</span>
              </div>
            {/each}
          </div>
          <p class="text-xs text-base-content/40 pt-2">
            {m.wizard_welcome_hint()}
          </p>
        </div>

        <!-- ── Step 2: SteamCMD ────────────────────────────────────────────── -->
      {:else if currentStep === 'steamcmd'}
        <div class="space-y-5">
          <div>
            <h2 class="text-base font-semibold text-base-content">{m.wizard_steamcmd_title()}</h2>
            <p class="text-xs text-base-content/50 mt-0.5">
              {m.wizard_steamcmd_desc()}
            </p>
          </div>

          <!-- Detection status banner -->
          <div
            class="rounded-xl border overflow-hidden
            {cmdFound ? 'border-success/40 bg-success/8' : 'border-base-300/60 bg-base-200/40'}"
          >
            <!-- Header row -->
            <div
              class="flex items-center gap-2 px-4 py-3
              {cmdFound ? 'border-b border-success/20' : ''}"
            >
              {#if detectingCmd}
                <span class="loading loading-ring loading-sm text-primary"></span>
                <span class="text-xs text-base-content/50">{m.wizard_steamcmd_detecting()}</span>
              {:else if cmdFound}
                <Icon icon="ph:check-circle" class="size-5 text-success shrink-0" />
                <div class="flex-1 min-w-0">
                  <p class="text-sm font-semibold text-success">{m.wizard_steamcmd_detected()}</p>
                  <p class="font-mono text-xs text-base-content/40 truncate mt-0.5">{steamcmdStatus?.path}</p>
                </div>
              {:else}
                <Icon icon="ph:warning-circle" class="size-5 text-warning shrink-0" />
                <span class="text-sm font-semibold text-warning">{m.wizard_steamcmd_not_found()}</span>
              {/if}
            </div>

            <!-- Platform-specific install instructions (only when not found) -->
            {#if !detectingCmd && !cmdFound}
              <!-- ── Linux: package manager commands ── -->
              {#if isLinux}
                <div class="px-4 py-3 space-y-3 border-t border-base-300/40">
                  <p class="text-xs text-base-content/60">{m.wizard_steamcmd_linux_hint()}</p>
                  <div class="rounded-lg border border-base-300/50 overflow-hidden divide-y divide-base-300/40">
                    {#each [{ icon: 'simple-icons:archlinux', label: 'Arch / Manjaro', cmd: 'yay -S steamcmd' }, { icon: 'simple-icons:debian', label: 'Debian / Ubuntu', cmd: 'sudo apt install steamcmd' }, { icon: 'simple-icons:fedora', label: 'Fedora / RHEL', cmd: 'sudo dnf install steamcmd' }] as row}
                      <div
                        class="flex items-center gap-2.5 px-3 py-2 bg-base-300/20 hover:bg-base-300/40 transition-colors"
                      >
                        <Icon icon={row.icon} class="size-3.5 text-base-content/30 shrink-0" />
                        <span class="text-xs text-base-content/50 shrink-0 w-28">{row.label}</span>
                        <code class="text-xs text-primary font-mono ml-auto">{row.cmd}</code>
                      </div>
                    {/each}
                  </div>
                  <div class="flex items-center gap-2 text-xs text-base-content/40 pt-1">
                    <span class="loading loading-ring loading-sm text-primary"></span>
                    <span>{m.wizard_steamcmd_linux_waiting()}</span>
                  </div>
                </div>

                <!-- ── Windows: auto-download or browse ── -->
              {:else if isWindows}
                <div class="px-4 py-3 space-y-3 border-t border-base-300/40">
                  <p class="text-xs text-base-content/60">
                    {m.wizard_steamcmd_win_hint()}
                  </p>

                  {#if downloadError}
                    <div
                      class="flex items-start gap-2 rounded-lg bg-error/10 border border-error/25 px-3 py-2 text-xs text-error"
                    >
                      <Icon icon="ph:warning-circle" class="size-4 shrink-0 mt-0.5" />
                      <span>{downloadError}</span>
                    </div>
                  {/if}

                  <!-- Install automatically -->
                  <button
                    class="btn btn-primary btn-sm w-full gap-2"
                    onclick={downloadSteamcmd}
                    disabled={downloadingCmd}
                  >
                    {#if downloadingCmd}
                      <span class="loading loading-spinner loading-xs"></span>
                      {m.wizard_steamcmd_downloading()}
                    {:else}
                      <Icon icon="ph:download-simple" class="size-4" />
                      {m.wizard_steamcmd_install_auto()}
                    {/if}
                  </button>
                  <p class="text-xs text-base-content/35 text-center">
                    {m.wizard_steamcmd_install_hint()}
                    <span class="font-mono text-base-content/50">%APPDATA%\dayz-community-hub\steamcmd\</span>
                  </p>

                  <div class="flex items-center gap-3">
                    <div class="flex-1 h-px bg-base-300/60"></div>
                    <span class="text-xs text-base-content/30 uppercase tracking-widest">{m.wizard_steamcmd_or()}</span>
                    <div class="flex-1 h-px bg-base-300/60"></div>
                  </div>

                  <!-- Browse for existing -->
                  <button class="btn btn-ghost btn-sm w-full gap-2" onclick={browseSteamcmd}>
                    <Icon icon="ph:folder-open" class="size-4" />
                    {m.wizard_steamcmd_browse()}
                  </button>
                </div>
              {/if}
            {/if}
          </div>

          <!-- Manual path override (always visible, collapsed style) -->
          {#if cmdFound}
            <div class="form-control">
              <label class="label py-0 pb-1.5" for="wiz-steamcmd">
                <span class="label-text text-xs flex items-center gap-1.5">
                  <Icon icon="ph:terminal-window" class="size-3.5 text-base-content/40" />
                  {m.wizard_steamcmd_path()}
                  <span class="text-base-content/30 ml-1">{m.wizard_steamcmd_override()}</span>
                </span>
              </label>
              <div class="flex gap-2">
                <input
                  id="wiz-steamcmd"
                  type="text"
                  class="input input-bordered input-sm flex-1 font-mono text-xs"
                  placeholder={steamcmdStatus?.path ?? m.wizard_auto_detect()}
                  bind:value={steamcmdPath}
                />
                <button class="btn btn-ghost btn-sm btn-square" onclick={browseSteamcmd} title={m.wizard_browse()}>
                  <Icon icon="ph:folder-open" class="size-4" />
                </button>
              </div>
            </div>
          {/if}

          <!-- Steam root folder -->
          <div class="form-control">
            <label class="label py-0 pb-1.5" for="wiz-steam-root">
              <span class="label-text text-xs flex items-center gap-1.5">
                <Icon icon="mdi:steam" class="size-3.5 text-base-content/40" />
                {m.wizard_steam_root()}
                <span class="text-base-content/30 ml-1">{m.wizard_optional()}</span>
              </span>
            </label>
            <div class="flex gap-2">
              <input
                id="wiz-steam-root"
                type="text"
                class="input input-bordered input-sm flex-1 font-mono text-xs"
                placeholder={m.wizard_auto_detect()}
                bind:value={steamRoot}
              />
              <button class="btn btn-ghost btn-sm btn-square" onclick={browseSteamRoot} title={m.wizard_browse()}>
                <Icon icon="ph:folder-open" class="size-4" />
              </button>
            </div>
            <p class="label py-0 pt-1">
              <span class="label-text-alt text-base-content/40">
                {#if isLinux}
                  {m.wizard_linux_path_hint({ path: '~/.steam/steam' })}
                {:else if isWindows}
                  {m.wizard_windows_path_hint({ path: 'C:\\Program Files (x86)\\Steam' })}
                {:else}
                  {m.wizard_leave_blank()}
                {/if}
              </span>
            </p>
          </div>
        </div>

        <!-- ── Step 3: Configure ───────────────────────────────────────────── -->
      {:else if currentStep === 'configure'}
        <div class="space-y-5">
          <div>
            <h2 class="text-base font-semibold text-base-content">{m.wizard_config_title()}</h2>
            <p class="text-xs text-base-content/50 mt-0.5">{m.wizard_config_desc()}</p>
          </div>

          <!-- Steam login -->
          <div class="bg-base-200/50 rounded-xl border border-base-300/60 p-4 space-y-3">
            <p class="text-xs font-semibold text-base-content/60 uppercase tracking-wide flex items-center gap-1.5">
              <Icon icon="mdi:steam" class="size-3.5" />
              {m.wizard_steamcmd_login()}
            </p>
            <div class="grid grid-cols-2 gap-3">
              <div class="form-control">
                <label class="label py-0 pb-1" for="wiz-login">
                  <span class="label-text text-xs text-base-content/50 flex items-center gap-1">
                    {m.wizard_username()}
                    <span class="text-error text-[10px]">*</span>
                  </span>
                </label>
                <input
                  id="wiz-login"
                  type="text"
                  class="input input-bordered input-xs font-mono {!steamLoginValid && steamLogin.length > 0
                    ? 'input-error'
                    : ''}"
                  placeholder={m.wizard_steam_username_placeholder()}
                  bind:value={steamLogin}
                />
              </div>
              <div class="form-control">
                <label class="label py-0 pb-1" for="wiz-pass">
                  <span class="label-text text-xs text-base-content/50 flex items-center gap-1">
                    {m.wizard_password()}
                    <span class="text-base-content/30 text-[10px]">{m.wizard_optional()}</span>
                  </span>
                </label>
                <div class="relative">
                  <input
                    id="wiz-pass"
                    type={showPass ? 'text' : 'password'}
                    class="input input-bordered input-xs w-full pr-7"
                    placeholder={m.wizard_password_cached()}
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
            {#if steamPass.length > 0}
              <div class="flex items-start gap-1.5 text-xs text-warning/70">
                <Icon icon="ph:warning" class="size-3 shrink-0 mt-0.5" />
                <span>{m.wizard_password_warning()}</span>
              </div>
            {/if}
            {#if !steamLoginValid}
              <div class="flex items-start gap-1.5 text-xs text-base-content/40">
                <Icon icon="ph:info" class="size-3 shrink-0 mt-0.5" />
                <span>{m.wizard_steam_required()}</span>
              </div>
            {/if}
          </div>

          <!-- Player name -->
          <div class="form-control">
            <label class="label py-0 pb-1.5" for="wiz-name">
              <span class="label-text text-xs flex items-center gap-1.5">
                <Icon icon="ph:game-controller" class="size-3.5 text-base-content/40" />
                {m.wizard_ingame_name()}
                <span class="text-base-content/30 ml-1">{m.wizard_optional()}</span>
              </span>
            </label>
            <input
              id="wiz-name"
              type="text"
              class="input input-bordered input-sm"
              placeholder={m.wizard_ingame_placeholder()}
              bind:value={playerName}
            />
            <p class="label py-0 pt-1">
              <span class="label-text-alt text-base-content/40">{m.wizard_ingame_hint({ flag: '-name=' })}</span>
            </p>
          </div>

          <!-- API key + Steam ID -->
          <div class="bg-base-200/50 rounded-xl border border-base-300/60 p-4 space-y-3">
            <p class="text-xs font-semibold text-base-content/60 uppercase tracking-wide flex items-center gap-1.5">
              <Icon icon="ph:identification-card" class="size-3.5" />
              {m.wizard_steam_api()}
              <span class="font-normal normal-case tracking-normal text-base-content/35 ml-1"
                >{m.wizard_optional()}</span
              >
            </p>
            <p class="text-xs text-base-content/40 leading-relaxed">
              {m.wizard_steam_api_desc()}
            </p>
            <div class="form-control">
              <label class="label py-0 pb-1" for="wiz-apikey">
                <span class="label-text text-xs text-base-content/50 flex items-center gap-1">
                  {m.wizard_api_key()}
                  <button
                    class="text-primary hover:underline ml-1"
                    onclick={() => openUrl('https://steamcommunity.com/dev/apikey')}
                    >steamcommunity.com/dev/apikey</button
                  >
                </span>
              </label>
              <input
                id="wiz-apikey"
                type="text"
                class="input input-bordered input-xs font-mono"
                placeholder={m.wizard_api_key_placeholder()}
                bind:value={steamApiKey}
              />
            </div>
            <div class="form-control">
              <label class="label py-0 pb-1" for="wiz-steamid">
                <span class="label-text text-xs text-base-content/50 flex items-center gap-1">
                  {m.wizard_steam_id_label()}
                  <button
                    class="text-primary hover:underline ml-1"
                    onclick={() => openUrl('https://steamdb.info/calculator/')}>steamdb.info/calculator</button
                  >
                </span>
              </label>
              <input
                id="wiz-steamid"
                type="text"
                class="input input-bordered input-xs font-mono"
                placeholder={m.wizard_steam_id_placeholder()}
                bind:value={steamId}
              />
            </div>
          </div>

          <!-- BattleMetrics API token -->
          <div class="bg-base-200/50 rounded-xl border border-base-300/60 p-4 space-y-3">
            <div class="flex items-center justify-between">
              <p class="text-xs font-semibold text-base-content/60 uppercase tracking-wide flex items-center gap-1.5">
                <Icon icon="ph:chart-line-up" class="size-3.5" />
                {m.wizard_bm_title()}
                <span class="font-normal normal-case tracking-normal text-base-content/35 ml-1"
                  >{m.wizard_optional()}</span
                >
              </p>
            </div>
            <p class="text-xs text-base-content/50 leading-relaxed">
              {m.wizard_bm_desc()}
            </p>
            <div class="form-control">
              <label class="label py-0 pb-1" for="wiz-bmkey">
                <span class="label-text text-xs text-base-content/50 flex items-center gap-1">
                  {m.wizard_bm_token()}
                  <button
                    class="text-primary hover:underline ml-1"
                    onclick={() => openUrl('https://www.battlemetrics.com/developers')}
                    >battlemetrics.com/developers</button
                  >
                </span>
              </label>
              <input
                id="wiz-bmkey"
                type="password"
                class="input input-bordered input-xs font-mono"
                placeholder={m.wizard_bm_token_placeholder()}
                bind:value={battlemetricsApiKey}
              />
            </div>
          </div>
        </div>

        <!-- ── Step 4: Done ────────────────────────────────────────────────── -->
      {:else if currentStep === 'done'}
        <div class="text-center space-y-4 py-4">
          <div class="size-16 rounded-full bg-success/15 flex items-center justify-center mx-auto">
            <Icon icon="ph:check-circle" class="size-9 text-success" />
          </div>
          <div>
            <h2 class="text-lg font-bold text-base-content">{m.wizard_done_title()}</h2>
            <p class="text-sm text-base-content/50 mt-1">
              {m.wizard_done_desc({ button: m.wizard_done_button() })}
            </p>
          </div>
          <div class="bg-base-200/60 rounded-xl border border-base-300/60 text-left divide-y divide-base-300/50">
            <div class="flex items-center gap-3 px-4 py-2.5">
              <Icon icon="ph:info" class="size-4 text-base-content/40 shrink-0" />
              <span class="text-xs text-base-content/50">{m.wizard_done_settings_hint()}</span>
            </div>
            <div class="flex items-center gap-3 px-4 py-2.5">
              <Icon icon="ph:book-open" class="size-4 text-base-content/40 shrink-0" />
              <span class="text-xs text-base-content/50">{m.wizard_done_about_hint({ tab: m.tab_about() })}</span>
            </div>
          </div>
          {#if saveError}
            <div
              class="flex items-start gap-2 px-3 py-2 rounded-lg bg-error/10 border border-error/25 text-xs text-error text-left"
            >
              <Icon icon="ph:warning-circle" class="size-4 shrink-0 mt-0.5" />
              <span>{saveError}</span>
            </div>
          {/if}
        </div>
      {/if}
    </div>

    <!-- Footer nav -->
    <div class="flex items-center justify-between px-8 py-4 border-t border-base-300 bg-base-200/40 shrink-0">
      <!-- Back / step label / import -->
      <div class="flex items-center gap-3">
        {#if currentStep !== 'welcome' && currentStep !== 'done'}
          <button class="btn btn-ghost btn-sm gap-1.5" onclick={back}>
            <Icon icon="ph:arrow-left" class="size-4" />
            {m.wizard_back()}
          </button>
        {/if}
        {#if currentStep === 'configure'}
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
            {m.wizard_import_profile()}
          </button>
          {#if importError}
            <span class="text-xs text-error">{importError}</span>
          {/if}
        {:else if currentStep !== 'welcome'}
          <span class="text-xs text-base-content/30 capitalize"
            >{currentStep === 'steamcmd' ? 'SteamCMD' : currentStep}</span
          >
        {/if}
      </div>

      <!-- Next / Finish -->
      <div class="flex items-center gap-2">
        {#if currentStep === 'welcome'}
          <button class="btn btn-primary btn-sm gap-1.5" onclick={next}>
            {m.wizard_get_started()}
            <Icon icon="ph:arrow-right" class="size-4" />
          </button>
        {:else if currentStep === 'steamcmd'}
          <button class="btn btn-primary btn-sm gap-1.5" onclick={next}>
            {m.wizard_next()}
            <Icon icon="ph:arrow-right" class="size-4" />
          </button>
        {:else if currentStep === 'configure'}
          <button
            class="btn btn-primary btn-sm gap-1.5"
            onclick={next}
            disabled={!steamLoginValid}
            title={!steamLoginValid ? m.wizard_steam_required() : ''}
          >
            {m.wizard_next()}
            <Icon icon="ph:arrow-right" class="size-4" />
          </button>
        {:else if currentStep === 'done'}
          <button class="btn btn-ghost btn-sm gap-1.5" onclick={back}>
            <Icon icon="ph:arrow-left" class="size-4" />
            {m.wizard_back()}
          </button>
          <button class="btn btn-success btn-sm gap-1.5" onclick={finish} disabled={saving}>
            {#if saving}
              <span class="loading loading-spinner loading-xs"></span>
            {:else}
              <Icon icon="ph:rocket-launch" class="size-4" />
            {/if}
            {m.wizard_launch()}
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>
