<script lang="ts">
  import Icon from '@iconify/svelte';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { getVersion, getName } from '@tauri-apps/api/app';
  import { invoke } from '@tauri-apps/api/core';
  import { onMount, onDestroy } from 'svelte';
  import GlitchText from '$lib/components/GlitchText.svelte';
  import SuikaGame from '$lib/components/SuikaGame.svelte';
  import { APP_AUTHOR, REPO_URL, ISSUES_URL, RELEASES_URL, WIKI_URL, LICENSE_URL } from '$lib/constants';
  import * as m from '$lib/paraglide/messages.js';

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
          <p class="text-xs text-base-content/50 mt-0.5">{m.about_hero_tagline()}</p>
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
              title={m.about_source_repo()}
            >
              <Icon icon="catppuccin:forgejo" class="size-3" />{m.about_forgejo()}
            </button>
          </div>
        </div>
        <!-- Feature highlights inline on the right -->
        <div class="hidden lg:flex items-center gap-4 shrink-0 border-l border-base-300/40 pl-6">
          {#each [{ icon: 'mdi:server-network', labelFn: () => m.about_feat_browser(), color: 'text-sky-400' }, { icon: 'mdi:puzzle', labelFn: () => m.about_feat_mods(), color: 'text-fuchsia-400' }, { icon: 'ph:chart-line-up', labelFn: () => m.about_feat_bm(), color: 'text-emerald-400' }, { icon: 'ph:rocket-launch', labelFn: () => m.about_feat_launch(), color: 'text-orange-400' }] as feat}
            <div class="feat-icon flex flex-col items-center gap-1 text-center cursor-default">
              <Icon icon={feat.icon} class="size-5 {feat.color}" />
              <span class="text-xs text-base-content/50 font-medium whitespace-nowrap">{feat.labelFn()}</span>
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
              <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">{m.about_updates()}</h2>
            </div>
            <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden">
              {#if updateState === 'idle' || updateState === 'checking'}
                <div class="flex items-center gap-3 px-4 py-3.5">
                  <span class="loading loading-spinner loading-sm text-primary shrink-0"></span>
                  <span class="text-sm text-base-content/60">{m.about_updates_checking()}</span>
                </div>
              {:else if updateState === 'up_to_date'}
                <div class="flex items-center gap-3 px-4 py-3.5">
                  <div
                    class="size-8 rounded-lg bg-success/10 border border-success/20 flex items-center justify-center shrink-0"
                  >
                    <Icon icon="ph:check-circle" class="size-4 text-success" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-semibold text-base-content">{m.about_updates_up_to_date()}</p>
                    <p class="text-xs text-base-content/45 mt-0.5">{m.about_updates_latest({ version: appVersion })}</p>
                  </div>
                  <button class="btn btn-xs btn-ghost text-base-content/40 gap-1" onclick={onCheckForUpdate}>
                    <Icon icon="ph:arrows-clockwise" class="size-3.5" />{m.about_updates_recheck()}
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
                      {m.about_updates_available()} <span class="text-primary font-mono">v{updateInfo?.version}</span>
                    </p>
                    <p class="text-xs text-base-content/45 mt-0.5">
                      {m.about_updates_current()} <span class="font-mono">v{updateInfo?.currentVersion}</span>
                      {#if updateInfo?.date}
                        · {m.about_updates_released({ date: new Date(updateInfo.date).toLocaleDateString() })}{/if}
                    </p>
                    {#if updateInfo?.body}
                      <p class="text-xs text-base-content/50 mt-2 leading-relaxed line-clamp-3">{updateInfo.body}</p>
                    {/if}
                  </div>
                </div>
                <div class="flex items-center justify-end gap-2 px-4 py-2.5 bg-base-200/60">
                  <button class="btn btn-sm btn-primary gap-1.5" onclick={onInstallUpdate}>
                    <Icon icon="ph:download-simple" class="size-3.5" />{m.about_updates_install()}
                  </button>
                </div>
              {:else if updateState === 'downloading'}
                <div class="px-4 py-3.5 space-y-2.5">
                  <div class="flex items-center gap-2">
                    <span class="loading loading-spinner loading-sm text-primary shrink-0"></span>
                    <span class="text-sm font-semibold text-base-content"
                      >{m.about_updates_downloading({ version: updateInfo?.version ?? '…' })}</span
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
                    <p class="text-sm font-semibold text-base-content">{m.about_updates_done()}</p>
                    <p class="text-xs text-base-content/45 mt-0.5">
                      {m.about_updates_done_hint()}
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
                    <Icon icon="ph:arrows-clockwise" class="size-3.5" />{m.about_updates_retry()}
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
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">{m.about_quickstart()}</h2>
          </div>
          <div class="space-y-2">
            {#each [{ n: 1, icon: 'ph:user-gear', titleFn: () => m.about_qs_step1_title(), bodyFn: () => m.about_qs_step1_body() }, { n: 2, icon: 'ph:game-controller', titleFn: () => m.about_qs_step2_title(), bodyFn: () => m.about_qs_step2_body() }, { n: 3, icon: 'mdi:server-network', titleFn: () => m.about_qs_step3_title(), bodyFn: () => m.about_qs_step3_body() }] as step, i}
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
                    <Icon icon={step.icon} class="size-3.5 text-primary/70 shrink-0" />{step.titleFn()}
                  </p>
                  <p class="text-xs text-base-content/50 mt-0.5 leading-relaxed">{step.bodyFn()}</p>
                </div>
              </div>
            {/each}
          </div>
        </section>

        <!-- SteamCMD & Mods -->
        <section class="stagger-item" class:visible={mounted} style="transition-delay: 360ms">
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:terminal-window" class="size-4 text-primary shrink-0" />
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">{m.about_steamcmd()}</h2>
          </div>
          <div class="rounded-xl border border-base-300/60 bg-base-200/40 divide-y divide-base-300/40">
            <div class="px-4 py-3">
              <p class="text-xs font-semibold text-base-content/70 mb-1.5">{m.about_steamcmd_what()}</p>
              <p class="text-xs text-base-content/50 leading-relaxed">
                {m.about_steamcmd_desc({ notFound: m.about_steamcmd_not_found() })}
              </p>
              {#if platform === 'linux' || platform === ''}
                <div class="mt-2 space-y-1 text-xs font-mono">
                  {#each [{ labelFn: () => m.about_steamcmd_linux_debian(), cmd: 'sudo apt install steamcmd' }, { labelFn: () => m.about_steamcmd_linux_arch(), cmd: 'yay -S steamcmd' }, { labelFn: () => m.about_steamcmd_linux_fedora(), cmd: 'sudo dnf install steamcmd' }] as row}
                    <div class="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-base-300/40">
                      <Icon icon="simple-icons:linux" class="size-3.5 text-base-content/40 shrink-0" />
                      <span class="text-base-content/40 shrink-0 w-28">{row.labelFn()}</span>
                      <span class="text-primary ml-auto">{row.cmd}</span>
                    </div>
                  {/each}
                </div>
              {/if}
              {#if platform === 'windows'}
                <div class="mt-3 space-y-2">
                  <p class="text-xs text-base-content/50 leading-relaxed">
                    {m.about_steamcmd_win_desc()}
                    <span class="font-mono text-base-content/70">%APPDATA%\dayz-community-hub\steamcmd\</span>.
                  </p>
                  {#if cmdDownloadOk}
                    <div
                      class="flex items-center gap-2 px-3 py-2 rounded-lg bg-success/10 border border-success/25 text-xs text-success"
                    >
                      <Icon icon="ph:check-circle" class="size-4 shrink-0" />{m.about_steamcmd_installed()}
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
                        <span class="loading loading-spinner loading-xs"></span>{m.about_steamcmd_downloading()}
                      {:else}
                        <Icon icon="ph:download-simple" class="size-4" />{m.about_steamcmd_install()}
                      {/if}
                    </button>
                  {/if}
                </div>
              {/if}
            </div>
            <div class="px-4 py-3">
              <p class="text-xs font-semibold text-base-content/70 mb-2">{m.about_mod_workflow()}</p>
              <div class="space-y-2">
                {#each [() => m.about_mod_step1(), () => m.about_mod_step2(), () => m.about_mod_step3()] as stepFn, i}
                  <div class="flex items-start gap-3 text-xs text-base-content/50">
                    <span
                      class="size-4 rounded-full bg-base-300/80 text-base-content/50 text-xs font-bold flex items-center justify-center shrink-0 mt-0.5"
                      >{i + 1}</span
                    >
                    <span class="leading-relaxed">{stepFn()}</span>
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
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">{m.about_shortcuts()}</h2>
          </div>
          <div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <div>
              <p class="text-xs font-semibold text-base-content/35 uppercase tracking-widest mb-1 px-1">
                {m.about_shortcuts_global()}
              </p>
              <div class="rounded-xl border border-base-300/50 overflow-hidden">
                {#each [{ keys: ['Ctrl', '1 – 9'], descFn: () => m.about_shortcut_tab() }, { keys: ['Ctrl', 'R'], descFn: () => m.about_shortcut_refresh() }, { keys: ['Ctrl', 'U'], descFn: () => m.about_shortcut_update() }, { keys: ['Ctrl', 'L'], descFn: () => m.about_shortcut_reconnect() }] as row, i}
                  <div
                    class="flex items-center gap-2 px-3 py-1.5 {i % 2 === 0
                      ? 'bg-base-200/30'
                      : ''} border-b border-base-300/30 last:border-0"
                  >
                    <div class="flex items-center gap-1 shrink-0 w-28">
                      {#each row.keys as k}<kbd class="kbd kbd-xs">{k}</kbd>{/each}
                    </div>
                    <span class="text-xs text-base-content/55">{row.descFn()}</span>
                  </div>
                {/each}
              </div>
            </div>

            <div>
              <p class="text-xs font-semibold text-base-content/35 uppercase tracking-widest mb-1 px-1">
                {m.about_shortcuts_servers()}
              </p>
              <div class="rounded-xl border border-base-300/50 overflow-hidden">
                {#each [{ keys: ['↑', '↓'], descFn: () => m.about_shortcut_nav() }, { keys: ['Enter'], descFn: () => m.about_shortcut_connect() }, { keys: ['D'], descFn: () => m.about_shortcut_direct() }, { keys: ['F'], descFn: () => m.about_shortcut_fav() }, { keys: ['I'], descFn: () => m.about_shortcut_info() }, { keys: ['P'], descFn: () => m.about_shortcut_ping() }, { keys: ['Esc'], descFn: () => m.about_shortcut_close() }, { keys: ['Dbl-click'], descFn: () => m.about_shortcut_dblclick() }] as row, i}
                  <div
                    class="flex items-center gap-2 px-3 py-1.5 {i % 2 === 0
                      ? 'bg-base-200/30'
                      : ''} border-b border-base-300/30 last:border-0"
                  >
                    <div class="flex items-center gap-1 shrink-0 w-28">
                      {#each row.keys as k}<kbd class="kbd kbd-xs">{k}</kbd>{/each}
                    </div>
                    <span class="text-xs text-base-content/55">{row.descFn()}</span>
                  </div>
                {/each}
              </div>
            </div>

            <div>
              <p class="text-xs font-semibold text-base-content/35 uppercase tracking-widest mb-1 px-1">
                {m.about_shortcuts_mods()}
              </p>
              <div class="rounded-xl border border-base-300/50 overflow-hidden">
                {#each [{ keys: ['↑', '↓'], descFn: () => m.about_shortcut_nav() }, { keys: ['Space'], descFn: () => m.about_shortcut_space() }, { keys: ['M'], descFn: () => m.about_shortcut_managed() }, { keys: ['Dbl-click'], descFn: () => m.about_shortcut_openfolder() }] as row, i}
                  <div
                    class="flex items-center gap-2 px-3 py-1.5 {i % 2 === 0
                      ? 'bg-base-200/30'
                      : ''} border-b border-base-300/30 last:border-0"
                  >
                    <div class="flex items-center gap-1 shrink-0 w-28">
                      {#each row.keys as k}<kbd class="kbd kbd-xs">{k}</kbd>{/each}
                    </div>
                    <span class="text-xs text-base-content/55">{row.descFn()}</span>
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
              {m.about_sharing()}
            </h2>
          </div>
          <div class="rounded-xl border border-base-300/60 bg-base-200/40 divide-y divide-base-300/40">
            <!-- How it works -->
            <div class="px-4 py-3">
              <p class="text-xs font-semibold text-base-content/70 mb-1.5">{m.about_sharing_how()}</p>
              <p class="text-xs text-base-content/50 leading-relaxed">
                {m.about_sharing_desc({ url: 'dzch://', file: '.dzch' })}
              </p>
            </div>

            <!-- dzch:// URLs -->
            <div class="px-4 py-3">
              <div class="flex items-center gap-2 mb-2">
                <Icon icon="ph:link" class="size-3.5 text-indigo-400 shrink-0" />
                <p class="text-xs font-semibold text-base-content/70">{m.about_sharing_urls()}</p>
              </div>
              <p class="text-xs text-base-content/50 leading-relaxed mb-2">
                {m.about_sharing_urls_desc()}
              </p>
              <div class="space-y-1.5 text-xs font-mono">
                <div class="px-3 py-1.5 rounded-lg bg-base-300/40">
                  <span class="text-base-content/35 select-none">{m.about_sharing_basic()} </span><span
                    class="text-primary break-all">dzch://192.168.1.1:2302</span
                  >
                </div>
                <div class="px-3 py-1.5 rounded-lg bg-base-300/40">
                  <span class="text-base-content/35 select-none">{m.about_sharing_with_mods()} </span><span
                    class="text-primary break-all">dzch://192.168.1.1:2302?mods=123456,789012</span
                  >
                </div>
                <div class="px-3 py-1.5 rounded-lg bg-base-300/40">
                  <span class="text-base-content/35 select-none">{m.about_sharing_full()} </span><span
                    class="text-primary break-all"
                    >dzch://192.168.1.1:2302?qport=27016&name=My%20Server&password=secret&mods=123456,789012</span
                  >
                </div>
              </div>
              <div class="mt-2 space-y-1">
                {#each [{ param: 'qport', descFn: () => m.about_sharing_param_qport() }, { param: 'name', descFn: () => m.about_sharing_param_name() }, { param: 'password', descFn: () => m.about_sharing_param_password() }, { param: 'mods', descFn: () => m.about_sharing_param_mods() }] as row}
                  <div class="flex items-start gap-2 text-xs">
                    <span class="font-mono text-primary/80 shrink-0 w-20 text-right">{row.param}</span>
                    <span class="text-base-content/45">{row.descFn()}</span>
                  </div>
                {/each}
              </div>
            </div>

            <!-- .dzch files -->
            <div class="px-4 py-3">
              <div class="flex items-center gap-2 mb-2">
                <Icon icon="ph:file-text" class="size-3.5 text-emerald-400 shrink-0" />
                <p class="text-xs font-semibold text-base-content/70">{m.about_sharing_files()}</p>
              </div>
              <p class="text-xs text-base-content/50 leading-relaxed mb-2">
                {m.about_sharing_files_desc({ button: m.about_sharing_files_button() })}
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
              <p class="text-xs font-semibold text-base-content/70 mb-2">{m.about_sharing_from_dc()}</p>
              <div class="space-y-2">
                {#each [() => m.about_sharing_dc_step1(), () => m.about_sharing_dc_step2(), () => m.about_sharing_dc_step3()] as stepFn, i}
                  <div class="flex items-start gap-3 text-xs text-base-content/50">
                    <span
                      class="size-4 rounded-full bg-base-300/80 text-base-content/50 text-xs font-bold flex items-center justify-center shrink-0 mt-0.5"
                      >{i + 1}</span
                    >
                    <span class="leading-relaxed">{stepFn()}</span>
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
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">{m.about_profile()}</h2>
          </div>
          <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden divide-y divide-base-300/40">
            <div class="flex items-center gap-4 px-4 py-3">
              <div
                class="size-8 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center shrink-0"
              >
                <Icon icon="ph:upload-simple" class="size-4 text-primary" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-semibold text-base-content">{m.about_profile_export()}</p>
                <p class="text-xs text-base-content/45 mt-0.5 leading-relaxed">
                  {m.about_profile_export_desc({ file: '.dchub' })}
                </p>
                <label class="flex items-center gap-2 mt-2 cursor-pointer select-none">
                  <input
                    type="checkbox"
                    class="checkbox checkbox-xs checkbox-primary"
                    bind:checked={exportIncludeMods}
                  />
                  <span class="text-xs text-base-content/60">{m.about_profile_include_mods()}</span>
                </label>
              </div>
              <button class="btn btn-sm btn-primary shrink-0 gap-1.5" onclick={() => onExport(exportIncludeMods)}>
                <Icon icon="ph:upload-simple" class="size-3.5" />{m.about_profile_export()}
              </button>
            </div>
            <div class="flex items-center gap-4 px-4 py-3">
              <div
                class="size-8 rounded-lg bg-base-300/60 border border-base-300 flex items-center justify-center shrink-0"
              >
                <Icon icon="ph:download-simple" class="size-4 text-base-content/50" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-semibold text-base-content">{m.about_profile_import()}</p>
                <p class="text-xs text-base-content/45 mt-0.5 leading-relaxed">
                  {m.about_profile_import_desc({ file: '.dchub' })}
                </p>
              </div>
              <button class="btn btn-sm btn-ghost shrink-0 gap-1.5" onclick={onImport}>
                <Icon icon="ph:download-simple" class="size-3.5" />{m.about_profile_import()}
              </button>
            </div>
            <div class="flex items-center gap-4 px-4 py-3">
              <div
                class="size-8 rounded-lg bg-error/10 border border-error/20 flex items-center justify-center shrink-0"
              >
                <Icon icon="ph:arrow-counter-clockwise" class="size-4 text-error/70" />
              </div>
              <div class="flex-1 min-w-0">
                <p class="text-sm font-semibold text-error/80">{m.about_profile_reset()}</p>
                <p class="text-xs text-base-content/45 mt-0.5 leading-relaxed">
                  {m.about_profile_reset_desc()}
                </p>
              </div>
              <button class="btn btn-sm btn-error btn-outline shrink-0 gap-1.5" onclick={onReset}>
                <Icon icon="ph:arrow-counter-clockwise" class="size-3.5" />{m.about_profile_reset()}
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
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">{m.about_tabs()}</h2>
          </div>
          <div class="rounded-xl border border-base-300/50 overflow-hidden divide-y divide-base-300/30">
            {#each [{ icon: 'mdi:server-network', labelFn: () => m.about_tab_servers(), color: 'text-sky-400', descFn: () => m.about_tab_servers_desc() }, { icon: 'ph:star', labelFn: () => m.about_tab_favorites(), color: 'text-yellow-400', descFn: () => m.about_tab_favorites_desc() }, { icon: 'ph:clock-clockwise', labelFn: () => m.about_tab_history(), color: 'text-teal-400', descFn: () => m.about_tab_history_desc() }, { icon: 'mdi:puzzle', labelFn: () => m.about_tab_mods(), color: 'text-fuchsia-400', descFn: () => m.about_tab_mods_desc() }, { icon: 'ph:newspaper', labelFn: () => m.about_tab_news(), color: 'text-rose-400', descFn: () => m.about_tab_news_desc() }, { icon: 'ph:plugs-connected', labelFn: () => m.about_tab_connect(), color: 'text-indigo-400', descFn: () => m.about_tab_connect_desc() }, { icon: 'ph:sliders', labelFn: () => m.about_tab_options(), color: 'text-orange-400', descFn: () => m.about_tab_options_desc() }, { icon: 'ph:mountains', labelFn: () => m.about_tab_offline(), color: 'text-emerald-400', descFn: () => m.about_tab_offline_desc() }] as tab, i}
              <div
                class="flex items-start gap-2.5 px-3 py-2.5 {i % 2 === 0
                  ? 'bg-base-200/20'
                  : ''} hover:bg-base-200/40 transition-colors"
              >
                <Icon icon={tab.icon} class="size-3.5 {tab.color} shrink-0 mt-0.5" />
                <div class="flex-1 min-w-0">
                  <span class="text-xs font-semibold text-base-content/80">{tab.labelFn()}</span>
                  <span class="text-xs text-base-content/40 leading-snug block mt-0.5">{tab.descFn()}</span>
                </div>
              </div>
            {/each}
          </div>
        </section>

        <!-- Optional API Keys -->
        <section class="stagger-item" class:visible={mounted} style="transition-delay: 240ms">
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:key" class="size-4 text-primary shrink-0" />
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">{m.about_api_keys()}</h2>
          </div>
          <div class="space-y-2">
            <!-- Steam -->
            <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden">
              <div class="flex items-center gap-2.5 px-3 py-2.5 border-b border-base-300/40 bg-base-200/60">
                <Icon icon="ph:identification-card" class="size-3.5 text-sky-400 shrink-0" />
                <div class="flex-1 min-w-0">
                  <p class="text-xs font-semibold text-base-content">{m.about_api_steam()}</p>
                  <p class="text-xs text-base-content/40">{m.about_api_steam_hint()}</p>
                </div>
              </div>
              <div class="px-3 py-2.5 space-y-1.5">
                <div class="flex items-start gap-1.5 text-xs text-base-content/50">
                  <Icon icon="ph:user-circle" class="size-3 text-sky-400/60 shrink-0 mt-0.5" />
                  <span
                    ><span class="font-semibold text-base-content/65">{m.about_api_steam_avatar()}</span> — {m.about_api_steam_avatar_desc()}</span
                  >
                </div>
                <div class="flex items-start gap-1.5 text-xs text-base-content/50">
                  <Icon icon="mdi:puzzle-outline" class="size-3 text-sky-400/60 shrink-0 mt-0.5" />
                  <span
                    ><span class="font-semibold text-base-content/65">{m.about_api_steam_mods()}</span> — {m.about_api_steam_mods_desc()}</span
                  >
                </div>
                <div class="flex flex-wrap gap-1.5 pt-0.5">
                  <button
                    class="inline-flex items-center gap-1 px-2 py-1 rounded-lg bg-base-300/50 border border-base-300 text-xs text-primary hover:bg-primary/10 hover:border-primary/30 transition-all"
                    onclick={() => openUrl('https://steamcommunity.com/dev/apikey')}
                  >
                    <Icon icon="mdi:steam" class="size-3" />{m.about_api_steam_key()}
                  </button>
                  <button
                    class="inline-flex items-center gap-1 px-2 py-1 rounded-lg bg-base-300/50 border border-base-300 text-xs text-primary hover:bg-primary/10 hover:border-primary/30 transition-all"
                    onclick={() => openUrl('https://steamdb.info/calculator/')}
                  >
                    <Icon icon="ph:calculator" class="size-3" />{m.about_api_steam_id()}
                  </button>
                </div>
              </div>
            </div>
            <!-- BattleMetrics -->
            <div class="rounded-xl border border-base-300/60 bg-base-200/40 overflow-hidden">
              <div class="flex items-center gap-2.5 px-3 py-2.5 border-b border-base-300/40 bg-base-200/60">
                <Icon icon="ph:chart-line-up" class="size-3.5 text-emerald-400 shrink-0" />
                <div class="flex-1 min-w-0">
                  <p class="text-xs font-semibold text-base-content">{m.about_api_bm()}</p>
                  <p class="text-xs text-base-content/40">{m.about_api_bm_hint()}</p>
                </div>
              </div>
              <div class="px-3 py-2.5 space-y-1.5">
                <div class="grid grid-cols-2 gap-1.5">
                  {#each [{ icon: 'ph:trophy', color: 'text-yellow-400/70', labelFn: () => m.about_api_bm_rank() }, { icon: 'ph:globe', color: 'text-sky-400/70', labelFn: () => m.about_api_bm_country() }, { icon: 'ph:map-pin', color: 'text-rose-400/70', labelFn: () => m.about_api_bm_distance() }, { icon: 'ph:clock', color: 'text-teal-400/70', labelFn: () => m.about_api_bm_uptime() }, { icon: 'ph:chart-line-up', color: 'text-emerald-400/70', labelFn: () => m.about_api_bm_graph() }] as feat}
                    <div
                      class="flex items-center gap-1.5 px-2 py-1.5 rounded-lg bg-base-300/30 border border-base-300/40"
                    >
                      <Icon icon={feat.icon} class="size-3 {feat.color} shrink-0" />
                      <span class="text-xs text-base-content/60">{feat.labelFn()}</span>
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
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">{m.about_built_with()}</h2>
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
            <h2 class="text-xs font-semibold text-base-content/60 uppercase tracking-widest">{m.about_links()}</h2>
          </div>
          <div class="flex flex-wrap gap-2">
            {#each [{ icon: 'ph:git-branch', labelFn: () => m.about_link_releases(), url: RELEASES_URL, color: 'text-emerald-400' }, { icon: 'ph:bug', labelFn: () => m.about_link_bug(), url: ISSUES_URL, color: 'text-orange-400' }, { icon: 'ph:book-open', labelFn: () => m.about_link_wiki(), url: WIKI_URL, color: 'text-sky-400' }, { icon: 'ph:scroll', labelFn: () => m.about_link_license(), url: LICENSE_URL, color: 'text-violet-400' }] as link}
              <button
                class="tech-badge inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-base-200/60 border border-base-300/60 text-xs text-base-content/50 hover:bg-base-200 hover:border-base-300 hover:text-base-content/80"
                onclick={() => openUrl(link.url)}
              >
                <Icon icon={link.icon} class="{link.color} size-3.5" />{link.labelFn()}
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
        {m.about_made_with()}
        <Icon icon="ph:heart-fill" class="size-3 text-error/40" />
        {m.about_by()}
        {AUTHOR}
        <button
          class="hover:text-primary transition-colors ml-1"
          onclick={() => openUrl(REPO_URL)}
          title={m.about_source_repo()}
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
