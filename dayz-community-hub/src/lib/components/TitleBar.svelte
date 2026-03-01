<script lang="ts">
  import type { AppStatsDto, ProfileDto } from '$lib/types';
  import { type ThemeName, THEMES } from '$lib/state.svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { open as openDialog, ask } from '@tauri-apps/plugin-dialog';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '@iconify/svelte';
  import GlitchText from '$lib/components/GlitchText.svelte';
  import CssEditor from '$lib/components/CssEditor.svelte';

  type UpdateState = 'idle' | 'checking' | 'up_to_date' | 'available' | 'downloading' | 'done' | 'error';

  interface Props {
    stats: AppStatsDto | null;
    avatarUrl: string | null;
    steamPlayers: number | null;
    theme: ThemeName;
    profile: ProfileDto | null;
    staleModCount?: number;
    updateState?: UpdateState;
    /** Increment to imperatively trigger the title glitch animation */
    glitchTick?: number;
    onSetTheme: (theme: ThemeName) => void;
    onUpdateMods?: () => void;
    onGoToUpdate?: () => void;
    onSaveSettings: (
      player: string | null,
      steamLogin: string | null,
      steamPassword: string | null,
      steamRoot: string | null,
      steamcmdEnabled: boolean,
      steamcmdPath: string | null,
      steamApiKey: string | null,
      steamId: string | null,
      battlemetricsApiKey: string | null,
      userLocation: [number, number] | null,
    ) => void;
    onUnexcludeIp: (ip: string) => void;
    onOpenExcludedIps: () => void;
  }

  let {
    stats,
    avatarUrl,
    steamPlayers,
    theme,
    profile,
    staleModCount = 0,
    updateState = 'idle',
    glitchTick = 0,
    onSetTheme,
    onSaveSettings,
    onUnexcludeIp,
    onOpenExcludedIps,
    onUpdateMods,
    onGoToUpdate,
  }: Props = $props();

  // ── Theme dropdown state ─────────────────────────────────────────────────
  let themeDropdownOpen = $state(false);
  let customThemeModalOpen = $state(false);
  let customCss = $state('');

  // Default custom theme CSS template
  const customCssTemplate = `/* Custom Theme */
[data-theme="custom"] {
  /* Base surfaces */
  --color-base-100: oklch(15% 0.01 0);
  --color-base-200: oklch(20% 0.01 0);
  --color-base-300: oklch(28% 0.015 0);
  --color-base-content: oklch(90% 0.01 0);

  /* Primary color */
  --color-primary: oklch(65% 0.20 255);
  --color-primary-content: oklch(10% 0.01 255);

  /* Secondary color */
  --color-secondary: oklch(60% 0.16 290);
  --color-secondary-content: oklch(95% 0.01 290);

  /* Accent color */
  --color-accent: oklch(65% 0.16 180);
  --color-accent-content: oklch(10% 0.01 180);

  /* Neutral */
  --color-neutral: oklch(20% 0.01 0);
  --color-neutral-content: oklch(90% 0.01 0);

  /* Status colors */
  --color-info: oklch(60% 0.16 230);
  --color-success: oklch(60% 0.18 145);
  --color-warning: oklch(70% 0.18 75);
  --color-error: oklch(60% 0.22 25);

  /* Border radius */
  --radius-btn: 0.375rem;
  --radius-box: 0.5rem;
  --radius-badge: 1rem;
}`;

  function selectTheme(t: ThemeName) {
    onSetTheme(t);
    themeDropdownOpen = false;
  }

  function openCustomThemeModal() {
    // Load saved custom CSS or use template
    const saved = localStorage.getItem('custom-theme-css');
    customCss = saved || customCssTemplate;
    // Switch to custom theme when opening editor
    onSetTheme('custom' as ThemeName);
    customThemeModalOpen = true;
    themeDropdownOpen = false;
  }

  function closeCustomThemeModal() {
    customThemeModalOpen = false;
  }

  function applyCustomTheme(css: string) {
    let styleEl = document.getElementById('custom-theme-style');
    if (!styleEl) {
      styleEl = document.createElement('style');
      styleEl.id = 'custom-theme-style';
      document.head.appendChild(styleEl);
    }
    styleEl.textContent = css;
  }

  // Load custom theme CSS on mount
  $effect(() => {
    const saved = localStorage.getItem('custom-theme-css');
    if (saved) {
      customCss = saved;
    }
  });

  // Apply and save custom CSS in real-time as it changes
  $effect(() => {
    applyCustomTheme(customCss);
    // Save to localStorage on every change
    if (customCss) {
      localStorage.setItem('custom-theme-css', customCss);
    }
  });

  function handleThemeKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      themeDropdownOpen = false;
    }
  }

  function handleCustomModalKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      closeCustomThemeModal();
    }
  }

  $effect(() => {
    if (themeDropdownOpen) {
      const handleClickOutside = (e: MouseEvent) => {
        const target = e.target as HTMLElement;
        if (!target.closest('.theme-dropdown')) {
          themeDropdownOpen = false;
        }
      };
      document.addEventListener('click', handleClickOutside);
      return () => document.removeEventListener('click', handleClickOutside);
    }
  });

  const currentTheme = $derived(THEMES.find((t) => t.id === theme) ?? THEMES[0]);

  // ── Window controls ────────────────────────────────────────────────────────
  const win = getCurrentWindow();
  const minimize = () => win.minimize();
  const toggleMaximize = () => win.toggleMaximize();
  const close = () => win.close();

  function onTitlebarMousedown(e: MouseEvent) {
    // Only drag on primary button, and not when clicking interactive children
    if (e.buttons !== 1) return;
    const target = e.target as HTMLElement;
    if (target.closest('button, input, a, [data-no-drag]')) return;
    if (e.detail === 2) {
      toggleMaximize();
    } else {
      win.startDragging();
    }
  }

  // ── Account modal state ────────────────────────────────────────────────────
  let logoHovered = $state(false);

  let modalOpen = $state(false);
  let playerName = $state('');
  let steamLogin = $state('');
  let steamPassword = $state('');
  let steamRoot = $state('');
  let steamApiKey = $state('');
  let steamId = $state('');
  let steamcmdPath = $state('');
  let battlemetricsApiKey = $state('');
  let userLocation = $state<[number, number] | null>(null);
  let detectingLocation = $state(false);
  let showPassword = $state(false);
  let showApiKey = $state(false);
  let showBmKey = $state(false);

  function openModal() {
    playerName = profile?.player ?? '';
    steamLogin = profile?.steam_login ?? '';
    steamPassword = profile?.steam_password ?? '';
    steamRoot = profile?.steam_root ?? '';
    steamcmdPath = profile?.steamcmd_path ?? '';
    steamApiKey = profile?.steam_api_key ?? '';
    steamId = profile?.steam_id ?? '';
    battlemetricsApiKey = profile?.battlemetrics_api_key ?? '';
    userLocation = profile?.user_location ?? null;
    manualLat = profile?.user_location ? profile.user_location[1].toFixed(4) : '';
    manualLon = profile?.user_location ? profile.user_location[0].toFixed(4) : '';
    locationError = '';
    detectedCity = '';
    detectedCountry = '';
    showPassword = false;
    showApiKey = false;
    showBmKey = false;
    modalOpen = true;
  }

  function closeModal() {
    modalOpen = false;
  }

  function handleOk() {
    onSaveSettings(
      playerName.trim() || null,
      steamLogin.trim() || null,
      steamPassword || null,
      steamRoot.trim() || null,
      profile?.steamcmd_enabled ?? true,
      steamcmdPath.trim() || null,
      steamApiKey.trim() || null,
      steamId.trim() || null,
      battlemetricsApiKey.trim() || null,
      userLocation,
    );
    closeModal();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') handleOk();
    if (e.key === 'Escape') closeModal();
  }

  async function browseSteamRoot() {
    const selected = await openDialog({
      directory: true,
      multiple: false,
      title: 'Select Steam root (steamapps folder)',
    });
    if (selected) steamRoot = selected as string;
  }

  async function clearSteamPassword() {
    const yes = await ask(
      'Remove the Steam password from profile.json? SteamCMD will fall back to cached credentials.',
      {
        title: 'Clear Steam password',
        kind: 'warning',
        okLabel: 'Remove',
        cancelLabel: 'Cancel',
      },
    );
    if (!yes) return;
    steamPassword = '';
    onSaveSettings(
      playerName.trim() || null,
      steamLogin.trim() || null,
      null,
      steamRoot.trim() || null,
      profile?.steamcmd_enabled ?? true,
      steamcmdPath.trim() || null,
      steamApiKey.trim() || null,
      steamId.trim() || null,
      battlemetricsApiKey.trim() || null,
      userLocation,
    );
  }

  let manualLat = $state('');
  let manualLon = $state('');
  let locationError = $state('');
  let detectedCity = $state('');
  let detectedCountry = $state('');

  async function detectLocationByIp() {
    detectingLocation = true;
    locationError = '';
    detectedCity = '';
    detectedCountry = '';
    try {
      const res = await fetch('http://ip-api.com/json/?fields=status,message,lat,lon,city,country,countryCode');
      const data = await res.json();
      if (data.status === 'success') {
        userLocation = [data.lon, data.lat];
        manualLat = data.lat.toFixed(4);
        manualLon = data.lon.toFixed(4);
        detectedCity = data.city || '';
        detectedCountry = data.country || '';
      } else {
        locationError = data.message || 'IP geolocation failed';
      }
    } catch (e) {
      locationError = 'Network error';
      console.error('IP geolocation failed:', e);
    } finally {
      detectingLocation = false;
    }
  }

  function applyManualLocation() {
    const lat = parseFloat(manualLat);
    const lon = parseFloat(manualLon);
    if (isNaN(lat) || isNaN(lon) || lat < -90 || lat > 90 || lon < -180 || lon > 180) {
      locationError = 'Invalid coordinates';
      return;
    }
    locationError = '';
    detectedCity = '';
    detectedCountry = '';
    userLocation = [lon, lat];
  }

  function clearLocation() {
    userLocation = null;
    manualLat = '';
    manualLon = '';
    locationError = '';
    detectedCity = '';
    detectedCountry = '';
  }

  function fmt(n: number | null | undefined): string {
    return n == null ? '—' : n.toLocaleString();
  }
</script>

<!-- ── Title bar ──────────────────────────────────────────────────────────── -->
<!-- Sits above modals (z-[1001] beats DaisyUI modal z-[1000]) so window
     controls and drag-to-move remain usable while any modal is open. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="h-9 flex items-center bg-base-200 border-b border-base-300 flex-shrink-0 select-none relative z-[1001]"
  onmousedown={onTitlebarMousedown}
  onmouseenter={() => (logoHovered = true)}
  onmouseleave={() => (logoHovered = false)}
>
  <!-- Left: identity — fixed width so glitch chars never shift adjacent elements -->
  <div
    class="flex items-center gap-2 px-4 pr-4 border-r border-base-300 shrink-0 overflow-hidden titlebar-identity"
    role="presentation"
  >
    <img src="/icon.svg" class="w-5 h-5 titlebar-logo" class:titlebar-logo--hovered={logoHovered} alt="icon" />
    <GlitchText
      text="DayZ Community Hub"
      class="text-sm font-semibold text-base-content tracking-tight font-mono whitespace-nowrap"
      externalTrigger={glitchTick}
    />
  </div>

  <!-- Center: live stats — absolutely centred so neither side affects its position -->
  <div
    class="absolute left-1/2 -translate-x-1/2 flex items-center gap-5 px-4 text-xs text-base-content/60 pointer-events-none"
  >
    <span class="flex items-center gap-1.5 pointer-events-auto" title="Servers">
      <Icon icon="mdi:server-network" class="size-3.5 text-accent-stat-server" />
      <span class="tabular-nums font-medium text-base-content/80">{fmt(stats?.server_count)}</span>
    </span>
    <span class="flex items-center gap-1.5 pointer-events-auto" title="Players in-game">
      <Icon icon="mdi:controller" class="size-3.5 text-accent-stat-players" />
      <span class="tabular-nums font-medium text-base-content/80">{fmt(stats?.total_players)}</span>
    </span>
    <span class="flex items-center gap-1.5 pointer-events-auto" title="Players on Steam">
      <Icon icon="mdi:steam" class="size-3.5 text-accent-stat-steam" />
      <span class="tabular-nums font-medium text-base-content/80">{fmt(steamPlayers)}</span>
    </span>
  </div>

  <!-- Spacer -->
  <div class="flex-1"></div>

  <!-- Right: user + theme + window controls -->
  <div class="flex items-center gap-1 text-xs">
    {#if stats && !stats.has_steamcmd}
      <button
        class="flex items-center gap-1 text-warning mr-2 hover:text-warning/80 transition-colors"
        title="SteamCMD not found — click to open settings and configure the path"
        onclick={openModal}
      >
        <Icon icon="ph:warning" class="size-3.5" />
        <span>SteamCMD not found</span>
      </button>
    {/if}

    <!-- Launcher update badge — only shown when an update is available -->
    {#if updateState === 'available'}
      <button
        class="flex items-center gap-1.5 px-2 py-1 rounded text-accent-update hover:opacity-80 hover:bg-base-300 transition-colors border-r border-base-300 mr-1 font-medium text-xs"
        onclick={onGoToUpdate}
        title="Launcher update available — click to view"
        data-no-drag
      >
        <Icon icon="line-md:downloading-loop" class="size-4" />
        Update available
      </button>
    {/if}

    <!-- Mod update badge — only shown when stale mods exist -->
    {#if staleModCount > 0}
      <button
        class="flex items-center gap-1.5 px-2 py-1 rounded text-accent-stale hover:opacity-80 hover:bg-base-300 transition-colors border-r border-base-300 mr-1 font-medium text-xs"
        onclick={onUpdateMods}
        title="Update {staleModCount} mod{staleModCount > 1 ? 's' : ''} — click to open Mods tab and start update"
        data-no-drag
      >
        <Icon icon="line-md:download-outline-loop" class="size-4" />
        Update {staleModCount} mod{staleModCount > 1 ? 's' : ''}
      </button>
    {/if}

    <!-- User chip -->
    <button
      class="flex items-center gap-1.5 px-2 py-1 rounded hover:bg-base-300 text-base-content/70 hover:text-primary transition-colors border-r border-base-300 mr-1"
      onclick={openModal}
      title="Edit account settings"
    >
      {#if avatarUrl}
        <img
          src={avatarUrl}
          alt="Steam avatar"
          class="size-5 rounded-full object-cover ring-1 ring-base-300 flex-shrink-0"
        />
      {:else}
        <Icon icon="ph:user-circle" class="size-3.5 text-base-content/40 flex-shrink-0" />
      {/if}
      {#if stats?.player_name}
        <span class="font-medium">{stats.player_name}</span>
        {#if stats.steam_login}
          <span class="text-base-content/40">({stats.steam_login})</span>
        {/if}
      {:else}
        <span class="italic text-base-content/40">Set up account</span>
      {/if}
      <Icon icon="ph:pencil-simple" class="size-3 text-base-content/30" />
    </button>

    <!-- Theme selector -->
    <div class="relative theme-dropdown" onkeydown={handleThemeKeydown}>
      <button
        class="inline-flex items-center justify-center gap-1.5 h-9 px-2 text-base-content/50 hover:bg-base-300 hover:text-base-content transition-colors"
        onclick={(e) => {
          e.stopPropagation();
          themeDropdownOpen = !themeDropdownOpen;
        }}
        title="Change theme"
      >
        <Icon icon={currentTheme.icon} class="size-4" />
        <Icon icon="ph:caret-down" class="size-3 opacity-50" />
      </button>

      {#if themeDropdownOpen}
        <div
          class="absolute right-0 top-full mt-1 w-44 bg-base-200 border border-base-300 rounded-lg shadow-xl z-50 py-1 overflow-hidden max-h-[420px] overflow-y-auto"
        >
          <!-- Dark themes section -->
          <div class="px-3 py-1.5 text-xs font-semibold text-base-content/40 uppercase tracking-wider">Dark</div>
          {#each THEMES.filter((t) => !t.isLight && !t.isMixed) as t}
            <button
              class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {t.id ===
              theme
                ? 'text-primary bg-base-300/50'
                : 'text-base-content/70'}"
              onclick={() => selectTheme(t.id)}
            >
              <Icon icon={t.icon} class="size-4 shrink-0" />
              <span class="flex-1">{t.label}</span>
              {#if t.id === theme}
                <Icon icon="ph:check" class="size-4 text-primary" />
              {/if}
            </button>
          {/each}

          <!-- Separator -->
          <div class="my-1 mx-2 border-t border-base-content/10"></div>

          <!-- Light themes section -->
          <div class="px-3 py-1.5 text-xs font-semibold text-base-content/40 uppercase tracking-wider">Light</div>
          {#each THEMES.filter((t) => t.isLight) as t}
            <button
              class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {t.id ===
              theme
                ? 'text-primary bg-base-300/50'
                : 'text-base-content/70'}"
              onclick={() => selectTheme(t.id)}
            >
              <Icon icon={t.icon} class="size-4 shrink-0" />
              <span class="flex-1">{t.label}</span>
              {#if t.id === theme}
                <Icon icon="ph:check" class="size-4 text-primary" />
              {/if}
            </button>
          {/each}

          <!-- Separator -->
          <div class="my-1 mx-2 border-t border-base-content/10"></div>

          <!-- Mixed themes section -->
          <div class="px-3 py-1.5 text-xs font-semibold text-base-content/40 uppercase tracking-wider">Mixed</div>
          {#each THEMES.filter((t) => t.isMixed) as t}
            <button
              class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {t.id ===
              theme
                ? 'text-primary bg-base-300/50'
                : 'text-base-content/70'}"
              onclick={() => selectTheme(t.id)}
            >
              <Icon icon={t.icon} class="size-4 shrink-0" />
              <span class="flex-1">{t.label}</span>
              {#if t.id === theme}
                <Icon icon="ph:check" class="size-4 text-primary" />
              {/if}
            </button>
          {/each}

          <!-- Separator -->
          <div class="my-1 mx-2 border-t border-base-content/10"></div>

          <!-- Custom theme button -->
          <button
            class="w-full flex items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-base-300 transition-colors {theme ===
            'custom'
              ? 'text-primary bg-base-300/50'
              : 'text-base-content/70'}"
            onclick={openCustomThemeModal}
          >
            <Icon icon="ph:palette" class="size-4 shrink-0" />
            <span class="flex-1">Custom</span>
            <Icon icon="ph:pencil-simple" class="size-3.5 text-base-content/40" />
          </button>
        </div>
      {/if}
    </div>

    <!-- Window controls -->
    <button
      class="inline-flex items-center justify-center w-10 h-9 text-base-content/45 hover:bg-base-300 hover:text-base-content transition-colors"
      onclick={minimize}
      title="Minimize"
    >
      <Icon icon="mdi:minus" class="size-3.5" />
    </button>
    <button
      class="inline-flex items-center justify-center w-10 h-9 text-base-content/45 hover:bg-base-300 hover:text-base-content transition-colors"
      onclick={toggleMaximize}
      title="Maximize"
    >
      <Icon icon="mdi:checkbox-blank-outline" class="size-3" />
    </button>
    <button
      class="inline-flex items-center justify-center w-10 h-9 text-base-content/45 hover:bg-red-600 hover:text-white transition-colors"
      onclick={close}
      title="Close"
    >
      <Icon icon="mdi:close" class="size-3.5" />
    </button>
  </div>
</div>

<!-- ── Account settings modal ─────────────────────────────────────────────── -->
{#if modalOpen}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
    style="top: 36px;"
    role="presentation"
    onclick={closeModal}
  >
    <div
      class="bg-base-100 rounded-xl shadow-2xl w-full max-w-md mx-4 flex flex-col overflow-hidden max-h-[90vh]"
      role="dialog"
      aria-modal="true"
      aria-label="Account settings"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={handleKeydown}
    >
      <!-- ── Header: identity preview ──────────────────────────────────────── -->
      <div class="flex items-center gap-3 px-5 py-4 bg-base-200 border-b border-base-300 flex-shrink-0">
        <!-- Avatar preview -->
        <div
          class="size-10 rounded-full bg-base-300 border border-base-300 overflow-hidden flex items-center justify-center flex-shrink-0"
        >
          {#if avatarUrl}
            <img src={avatarUrl} alt="Steam avatar" class="w-full h-full object-cover" />
          {:else}
            <Icon icon="ph:user" class="size-5 text-base-content/30" />
          {/if}
        </div>
        <div class="flex-1 min-w-0">
          <p class="text-sm font-semibold text-base-content leading-tight truncate">
            {playerName || 'Unnamed player'}
          </p>
          <p class="text-xs text-base-content/50 truncate">
            {steamLogin ? `Steam: ${steamLogin}` : 'No Steam account linked'}
          </p>
        </div>
        <button
          class="size-7 rounded flex items-center justify-center text-base-content/40 hover:bg-base-300 hover:text-base-content transition-colors flex-shrink-0"
          onclick={closeModal}
          title="Close"
        >
          <Icon icon="ph:x" class="size-3.5" />
        </button>
      </div>

      <!-- ── Scrollable body ───────────────────────────────────────────────── -->
      <div class="flex-1 overflow-y-auto p-5 space-y-5">
        <!-- ── Section: Identity ──────────────────────────────────────────── -->
        <div>
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:game-controller" class="size-3.5 text-primary" />
            <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wider">Identity</span>
          </div>
          <div class="bg-base-200/60 rounded-lg border border-base-300/60 overflow-hidden">
            <!-- In-game name -->
            <div class="flex items-center gap-3 px-3 py-2.5 border-b border-base-300/60">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-player">In-game name</label>
              <input
                id="field-player"
                type="text"
                class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none"
                placeholder="Your DayZ player name"
                autocomplete="nickname"
                bind:value={playerName}
              />
            </div>
          </div>
        </div>

        <!-- ── Section: Steam Login ───────────────────────────────────────── -->
        <div>
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="mdi:steam" class="size-3.5 text-primary" />
            <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wider">Steam Login</span>
            <span class="text-xs text-base-content/35 font-normal normal-case tracking-normal"
              >for SteamCMD mod updates</span
            >
          </div>
          <div class="bg-base-200/60 rounded-lg border border-base-300/60 overflow-hidden">
            <!-- Username -->
            <div class="flex items-center gap-3 px-3 py-2.5 border-b border-base-300/60">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-login">Username</label>
              <input
                id="field-login"
                type="text"
                class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none"
                placeholder="anonymous"
                autocomplete="username"
                bind:value={steamLogin}
              />
            </div>
            <!-- Password -->
            <div class="flex items-center gap-3 px-3 py-2.5 border-b border-base-300/60">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-password">Password</label>
              <div class="flex-1 flex items-center gap-1.5">
                {#if showPassword}
                  <input
                    id="field-password"
                    type="text"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder="Leave blank for cached credentials"
                    autocomplete="current-password"
                    bind:value={steamPassword}
                  />
                {:else}
                  <input
                    id="field-password"
                    type="password"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder="Leave blank for cached credentials"
                    autocomplete="current-password"
                    bind:value={steamPassword}
                  />
                {/if}
                <button
                  type="button"
                  class="text-base-content/30 hover:text-base-content transition-colors shrink-0"
                  onclick={() => (showPassword = !showPassword)}
                  title={showPassword ? 'Hide' : 'Show'}
                >
                  <Icon icon={showPassword ? 'ph:eye-slash' : 'ph:eye'} class="size-3.5" />
                </button>
                {#if steamPassword}
                  <button
                    type="button"
                    class="text-base-content/30 hover:text-error transition-colors shrink-0"
                    onclick={clearSteamPassword}
                    title="Clear password from profile"
                  >
                    <Icon icon="ph:x-circle" class="size-3.5" />
                  </button>
                {/if}
              </div>
            </div>
            <!-- Steam root -->
            <div class="flex items-center gap-3 px-3 py-2.5 border-b border-base-300/60">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-root">Steam root</label>
              <input
                id="field-root"
                type="text"
                class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                placeholder="~/.steam/steam"
                bind:value={steamRoot}
              />
              <button
                class="text-base-content/35 hover:text-primary transition-colors shrink-0"
                onclick={browseSteamRoot}
                title="Browse…"
              >
                <Icon icon="ph:folder-open" class="size-3.5" />
              </button>
            </div>
            <!-- SteamCMD path -->
            <div class="flex items-center gap-3 px-3 py-2.5">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-steamcmd">SteamCMD path</label>
              <input
                id="field-steamcmd"
                type="text"
                class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                placeholder="auto-detect"
                bind:value={steamcmdPath}
              />
              <button
                class="text-base-content/35 hover:text-primary transition-colors shrink-0"
                onclick={async () => {
                  const selected = await openDialog({ multiple: false, title: 'Select steamcmd binary' });
                  if (selected) steamcmdPath = selected as string;
                }}
                title="Browse…"
              >
                <Icon icon="ph:folder-open" class="size-3.5" />
              </button>
            </div>
          </div>
          <!-- Plaintext warning -->
          {#if steamPassword}
            <div class="flex items-start gap-2 mt-2 px-3 py-2 rounded-lg bg-warning/10 border border-warning/30">
              <Icon icon="ph:warning" class="size-3.5 text-warning shrink-0 mt-0.5" />
              <p class="text-xs text-warning leading-snug">
                Password is stored in plaintext in <span class="font-mono">profile.json</span>
              </p>
            </div>
          {/if}
        </div>

        <!-- ── Section: Steam API ─────────────────────────────────────────── -->
        <div>
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:identification-card" class="size-3.5 text-primary" />
            <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wider">Steam API</span>
            <span class="text-xs text-base-content/35 font-normal normal-case tracking-normal"
              >avatar in titlebar &amp; mod update checks</span
            >
          </div>
          <div class="bg-base-200/60 rounded-lg border border-base-300/60 overflow-hidden">
            <!-- API key -->
            <div class="flex items-center gap-3 px-3 py-2.5 border-b border-base-300/60">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-apikey">API key</label>
              <div class="flex-1 flex items-center gap-1.5">
                {#if showApiKey}
                  <input
                    id="field-apikey"
                    type="text"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder="steamcommunity.com/dev/apikey"
                    autocomplete="off"
                    bind:value={steamApiKey}
                  />
                {:else}
                  <input
                    id="field-apikey"
                    type="password"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder="steamcommunity.com/dev/apikey"
                    autocomplete="off"
                    bind:value={steamApiKey}
                  />
                {/if}
                <button
                  type="button"
                  class="text-base-content/30 hover:text-base-content transition-colors shrink-0"
                  onclick={() => (showApiKey = !showApiKey)}
                  title={showApiKey ? 'Hide' : 'Show'}
                >
                  <Icon icon={showApiKey ? 'ph:eye-slash' : 'ph:eye'} class="size-3.5" />
                </button>
              </div>
            </div>
            <!-- Steam ID -->
            <div class="flex items-center gap-3 px-3 py-2.5">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-steamid">Steam ID</label>
              <input
                id="field-steamid"
                type="text"
                class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none"
                placeholder="76561198000000000"
                bind:value={steamId}
              />
            </div>
          </div>
        </div>

        <!-- ── Section: BattleMetrics ─────────────────────────────────────── -->
        <div>
          <div class="flex items-center gap-2 mb-3">
            <Icon icon="ph:chart-line-up" class="size-3.5 text-primary" />
            <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wider">BattleMetrics</span>
            <span class="text-xs text-base-content/35 font-normal normal-case tracking-normal"
              >rankings, uptime &amp; distance</span
            >
          </div>
          <div class="bg-base-200/60 rounded-lg border border-base-300/60 overflow-hidden">
            <!-- API token row -->
            <div class="flex items-center gap-3 px-3 py-2.5 border-b border-base-300/40">
              <label class="text-xs text-base-content/55 w-24 shrink-0" for="field-bmkey">API token</label>
              <div class="flex-1 flex items-center gap-1.5">
                {#if showBmKey}
                  <input
                    id="field-bmkey"
                    type="text"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder="battlemetrics.com/developers"
                    autocomplete="off"
                    bind:value={battlemetricsApiKey}
                  />
                {:else}
                  <input
                    id="field-bmkey"
                    type="password"
                    class="flex-1 bg-transparent text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none min-w-0"
                    placeholder="battlemetrics.com/developers"
                    autocomplete="off"
                    bind:value={battlemetricsApiKey}
                  />
                {/if}
                <button
                  type="button"
                  class="text-base-content/30 hover:text-base-content transition-colors shrink-0"
                  onclick={() => (showBmKey = !showBmKey)}
                  title={showBmKey ? 'Hide' : 'Show'}
                >
                  <Icon icon={showBmKey ? 'ph:eye-slash' : 'ph:eye'} class="size-3.5" />
                </button>
              </div>
            </div>
            <!-- Location section -->
            <div class="px-3 py-3 space-y-2.5">
              <div class="flex items-center gap-2">
                <Icon icon="ph:map-pin" class="size-3.5 text-primary/70" />
                <span class="text-xs font-medium text-base-content/60">Your Location</span>
                <span class="text-xs text-base-content/30">for distance calculation</span>
              </div>

              {#if userLocation}
                <!-- Location set: show nice card -->
                <div class="flex items-center gap-3 px-3 py-2.5 rounded-lg bg-success/5 border border-success/20">
                  <div class="size-8 rounded-full bg-success/10 flex items-center justify-center shrink-0">
                    <Icon icon="ph:map-pin-fill" class="size-4 text-success" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <p class="text-sm font-medium text-base-content truncate">
                      {#if detectedCity || detectedCountry}
                        {detectedCity}{detectedCity && detectedCountry ? ', ' : ''}{detectedCountry}
                      {:else}
                        Location set
                      {/if}
                    </p>
                    <p class="text-xs text-base-content/40 font-mono">
                      {userLocation[1].toFixed(4)}, {userLocation[0].toFixed(4)}
                    </p>
                  </div>
                  <div class="flex items-center gap-1 shrink-0">
                    <button
                      type="button"
                      class="btn btn-ghost btn-xs btn-square text-base-content/40 hover:text-primary"
                      onclick={() => openUrl(`https://www.google.com/maps?q=${userLocation![1]},${userLocation![0]}`)}
                      title="Open in Google Maps"
                    >
                      <Icon icon="ph:map-trifold" class="size-4" />
                    </button>
                    <button
                      type="button"
                      class="btn btn-ghost btn-xs btn-square text-base-content/40 hover:text-error"
                      onclick={clearLocation}
                      title="Clear location"
                    >
                      <Icon icon="ph:trash" class="size-4" />
                    </button>
                  </div>
                </div>
              {:else}
                <!-- No location: show detection options -->
                <div class="flex items-center gap-2">
                  <button
                    type="button"
                    class="btn btn-sm btn-primary gap-1.5 flex-1"
                    onclick={detectLocationByIp}
                    disabled={detectingLocation}
                  >
                    {#if detectingLocation}
                      <span class="loading loading-spinner loading-xs"></span>
                      Detecting…
                    {:else}
                      <Icon icon="ph:crosshair" class="size-4" />
                      Auto-detect via IP
                    {/if}
                  </button>
                </div>
              {/if}

              <!-- Manual input (always visible, collapsed style) -->
              <div class="flex items-center gap-2 pt-1">
                <span class="text-xs text-base-content/35">Manual:</span>
                <input
                  type="text"
                  class="w-20 px-2 py-1 rounded bg-base-300/40 text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none border border-transparent focus:border-primary/50"
                  placeholder="Lat"
                  bind:value={manualLat}
                />
                <input
                  type="text"
                  class="w-20 px-2 py-1 rounded bg-base-300/40 text-xs font-mono text-base-content placeholder:text-base-content/25 outline-none border border-transparent focus:border-primary/50"
                  placeholder="Lon"
                  bind:value={manualLon}
                />
                <button
                  type="button"
                  class="btn btn-ghost btn-xs gap-1"
                  onclick={applyManualLocation}
                  disabled={!manualLat || !manualLon}
                >
                  <Icon icon="ph:check" class="size-3.5" />
                </button>
              </div>

              {#if locationError}
                <div class="flex items-center gap-1.5 px-2 py-1.5 rounded bg-error/10 text-error text-xs">
                  <Icon icon="ph:warning-circle" class="size-3.5 shrink-0" />
                  {locationError}
                </div>
              {/if}
            </div>
          </div>
          <p class="text-xs text-base-content/35 mt-1.5 px-1">
            Get a token at
            <button
              type="button"
              class="text-primary hover:underline"
              onclick={() => {
                openUrl('https://www.battlemetrics.com/developers');
              }}>battlemetrics.com/developers</button
            >
            · Location is used to calculate distance to servers.
          </p>
        </div>
      </div>

      <!-- ── Footer ─────────────────────────────────────────────────────────── -->
      <div class="flex items-center justify-between px-5 py-3 border-t border-base-300 bg-base-200 flex-shrink-0">
        <button class="btn btn-ghost btn-sm text-base-content/60" onclick={closeModal}> Cancel </button>
        <div class="flex items-center gap-2">
          <button
            class="btn btn-ghost btn-sm gap-1.5 text-base-content/50"
            onclick={() => {
              onOpenExcludedIps();
            }}
            title="Manage excluded IPs"
          >
            <Icon icon="ph:prohibit" class="size-3.5" />
            Excluded IPs
            {#if (profile?.excluded_ips?.length ?? 0) > 0}
              <span class="badge badge-xs badge-error/70 text-error font-mono">{profile!.excluded_ips!.length}</span>
            {/if}
          </button>
          <button class="btn btn-primary btn-sm gap-1.5" onclick={handleOk}>
            <Icon icon="ph:check" class="size-3.5" />
            Save changes
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- ── Custom theme modal ───────────────────────────────────────────────────── -->
{#if customThemeModalOpen}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
    style="top: 36px;"
    role="presentation"
    onclick={closeCustomThemeModal}
  >
    <div
      class="bg-base-100 rounded-xl shadow-2xl w-full max-w-2xl mx-4 flex flex-col overflow-hidden max-h-[85vh]"
      role="dialog"
      aria-modal="true"
      aria-label="Custom theme editor"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={handleCustomModalKeydown}
    >
      <!-- Header -->
      <div class="flex items-center justify-between px-5 py-4 bg-base-200 border-b border-base-300 flex-shrink-0">
        <div class="flex items-center gap-3">
          <Icon icon="ph:palette" class="size-5 text-primary" />
          <div>
            <h2 class="text-sm font-semibold text-base-content">Custom Theme Editor</h2>
            <p class="text-xs text-base-content/50">Define your own theme using CSS variables</p>
          </div>
        </div>
        <button
          class="size-7 rounded flex items-center justify-center text-base-content/40 hover:bg-base-300 hover:text-base-content transition-colors"
          onclick={closeCustomThemeModal}
          title="Close"
        >
          <Icon icon="ph:x" class="size-4" />
        </button>
      </div>

      <!-- Color reference -->
      <div class="px-5 py-3 bg-base-200/50 border-b border-base-300/50">
        <div class="flex items-center gap-2 mb-2">
          <Icon icon="ph:info" class="size-3.5 text-base-content/50" />
          <span class="text-xs font-medium text-base-content/60">Current theme colors</span>
        </div>
        <div class="flex flex-wrap gap-2">
          <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-base-100 border border-base-300">
            <div class="size-3 rounded-full bg-base-100 border border-base-300"></div>
            <span class="text-xs text-base-content/60">base-100</span>
          </div>
          <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-base-100 border border-base-300">
            <div class="size-3 rounded-full bg-base-200"></div>
            <span class="text-xs text-base-content/60">base-200</span>
          </div>
          <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-base-100 border border-base-300">
            <div class="size-3 rounded-full bg-base-300"></div>
            <span class="text-xs text-base-content/60">base-300</span>
          </div>
          <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-base-100 border border-base-300">
            <div class="size-3 rounded-full bg-primary"></div>
            <span class="text-xs text-base-content/60">primary</span>
          </div>
          <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-base-100 border border-base-300">
            <div class="size-3 rounded-full bg-secondary"></div>
            <span class="text-xs text-base-content/60">secondary</span>
          </div>
          <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-base-100 border border-base-300">
            <div class="size-3 rounded-full bg-accent"></div>
            <span class="text-xs text-base-content/60">accent</span>
          </div>
          <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-base-100 border border-base-300">
            <div class="size-3 rounded-full bg-success"></div>
            <span class="text-xs text-base-content/60">success</span>
          </div>
          <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-base-100 border border-base-300">
            <div class="size-3 rounded-full bg-warning"></div>
            <span class="text-xs text-base-content/60">warning</span>
          </div>
          <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-base-100 border border-base-300">
            <div class="size-3 rounded-full bg-error"></div>
            <span class="text-xs text-base-content/60">error</span>
          </div>
          <div class="flex items-center gap-1.5 px-2 py-1 rounded bg-base-100 border border-base-300">
            <div class="size-3 rounded-full bg-info"></div>
            <span class="text-xs text-base-content/60">info</span>
          </div>
        </div>
      </div>

      <!-- CSS Editor -->
      <div class="flex-1 min-h-0 p-4">
        <div class="h-full">
          <CssEditor
            value={customCss}
            onInput={(v) => (customCss = v)}
            placeholder="Enter your custom CSS..."
          />
        </div>
      </div>

      <!-- Footer -->
      <div class="flex items-center justify-between px-5 py-3 border-t border-base-300 bg-base-200 flex-shrink-0">
        <div class="flex items-center gap-2">
          <button
            class="btn btn-ghost btn-sm text-base-content/60"
            onclick={() => {
              customCss = customCssTemplate;
            }}
            title="Reset to default template"
          >
            <Icon icon="ph:arrow-counter-clockwise" class="size-3.5" />
            Reset
          </button>
        </div>
        <div class="flex items-center gap-2">
          <span class="text-xs text-base-content/40 italic">Auto-saved</span>
          <button class="btn btn-primary btn-sm gap-1.5" onclick={closeCustomThemeModal}>
            <Icon icon="ph:check" class="size-3.5" />
            Done
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .titlebar-logo {
    transition: transform 0.6s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .titlebar-logo--hovered {
    transform: rotate(15deg) scale(1.1);
  }
</style>
