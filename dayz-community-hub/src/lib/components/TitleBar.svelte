<script lang="ts">
  import type { AppStatsDto, ProfileDto } from '$lib/types';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { open as openDialog, ask } from '@tauri-apps/plugin-dialog';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '@iconify/svelte';
  import GlitchText from '$lib/components/GlitchText.svelte';


  type UpdateState =
    | 'idle' | 'checking' | 'up_to_date' | 'available' | 'downloading' | 'done' | 'error';

  interface Props {
    stats: AppStatsDto | null;
    avatarUrl: string | null;
    steamPlayers: number | null;
    theme: 'light' | 'dark';
    profile: ProfileDto | null;
    staleModCount?: number;
    updateState?: UpdateState;
    /** Increment to imperatively trigger the title glitch animation */
    glitchTick?: number;
    onToggleTheme: () => void;
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
    ) => void;
    onUnexcludeIp: (ip: string) => void;
    onOpenExcludedIps: () => void;
  }

  let { stats, avatarUrl, steamPlayers, theme, profile, staleModCount = 0, updateState = 'idle', glitchTick = 0, onToggleTheme, onSaveSettings, onUnexcludeIp, onOpenExcludedIps, onUpdateMods, onGoToUpdate }: Props = $props();

  // ── Window controls ────────────────────────────────────────────────────────
  const win = getCurrentWindow();
  const minimize       = () => win.minimize();
  const toggleMaximize = () => win.toggleMaximize();
  const close          = () => win.close();

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

  let modalOpen            = $state(false);
  let playerName           = $state('');
  let steamLogin           = $state('');
  let steamPassword        = $state('');
  let steamRoot            = $state('');
  let steamApiKey          = $state('');
  let steamId              = $state('');
  let steamcmdPath         = $state('');
  let battlemetricsApiKey  = $state('');
  let showPassword         = $state(false);
  let showApiKey           = $state(false);
  let showBmKey            = $state(false);

  function openModal() {
    playerName           = profile?.player ?? '';
    steamLogin           = profile?.steam_login ?? '';
    steamPassword        = profile?.steam_password ?? '';
    steamRoot            = profile?.steam_root ?? '';
    steamcmdPath         = profile?.steamcmd_path ?? '';
    steamApiKey          = profile?.steam_api_key ?? '';
    steamId              = profile?.steam_id ?? '';
    battlemetricsApiKey  = profile?.battlemetrics_api_key ?? '';
    showPassword         = false;
    showApiKey           = false;
    showBmKey            = false;
    modalOpen            = true;
  }

  function closeModal() { modalOpen = false; }

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
    );
    closeModal();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') handleOk();
    if (e.key === 'Escape') closeModal();
  }

  async function browseSteamRoot() {
    const selected = await openDialog({ directory: true, multiple: false, title: 'Select Steam root (steamapps folder)' });
    if (selected) steamRoot = selected as string;
  }

  async function clearSteamPassword() {
    const yes = await ask('Remove the Steam password from profile.json? SteamCMD will fall back to cached credentials.', {
      title: 'Clear Steam password',
      kind: 'warning',
      okLabel: 'Remove',
      cancelLabel: 'Cancel',
    });
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
    );
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
  onmouseenter={() => logoHovered = true}
  onmouseleave={() => logoHovered = false}
>

  <!-- Left: identity — fixed width so glitch chars never shift adjacent elements -->
  <div
    class="flex items-center gap-2 px-4 pr-4 border-r border-base-300 shrink-0 overflow-hidden titlebar-identity"
    role="presentation"
  >
    <img
      src="/icon.svg"
      class="w-5 h-5 titlebar-logo"
      class:titlebar-logo--hovered={logoHovered}
      alt="icon"
    />
    <GlitchText
      text="DayZ Community Hub"
      class="text-sm font-semibold text-base-content tracking-tight font-mono whitespace-nowrap"
      externalTrigger={glitchTick}
    />
  </div>

  <!-- Center: live stats — absolutely centred so neither side affects its position -->
  <div class="absolute left-1/2 -translate-x-1/2 flex items-center gap-5 px-4 text-xs text-base-content/60 pointer-events-none">
    <span class="flex items-center gap-1.5 pointer-events-auto" title="Servers">
      <Icon icon="mdi:server-network" class="size-3.5 text-red-500 dark:text-red-400" />
      <span class="tabular-nums font-medium text-base-content/80">{fmt(stats?.server_count)}</span>
    </span>
    <span class="flex items-center gap-1.5 pointer-events-auto" title="Players in-game">
      <Icon icon="mdi:controller" class="size-3.5 text-green-500 dark:text-green-400" />
      <span class="tabular-nums font-medium text-base-content/80">{fmt(stats?.total_players)}</span>
    </span>
    <span class="flex items-center gap-1.5 pointer-events-auto" title="Players on Steam">
      <Icon icon="mdi:steam" class="size-3.5 text-sky-500 dark:text-sky-500" />
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
        class="flex items-center gap-1.5 px-2 py-1 rounded text-emerald-400 hover:text-emerald-300 hover:bg-base-300 transition-colors border-r border-base-300 mr-1 font-medium text-xs"
        onclick={onGoToUpdate}
        title="Launcher update available — click to view"
        data-no-drag
      >
        <Icon icon="line-md:downloading-loop" class="size-4 text-emerald-400" />
        Update available
      </button>
    {/if}

    <!-- Mod update badge — only shown when stale mods exist -->
    {#if staleModCount > 0}
      <button
        class="flex items-center gap-1.5 px-2 py-1 rounded text-yellow-400 hover:text-yellow-300 hover:bg-base-300 transition-colors border-r border-base-300 mr-1 font-medium text-xs"
        onclick={onUpdateMods}
        title="Update {staleModCount} mod{staleModCount > 1 ? 's' : ''} — click to open Mods tab and start update"
        data-no-drag
      >
        <Icon icon="line-md:download-outline-loop" class="size-4 text-yellow-400" />
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

    <!-- Theme toggle -->
    <button
      class="inline-flex items-center justify-center w-9 h-9 text-base-content/50 hover:bg-base-300 hover:text-base-content transition-colors"
      onclick={onToggleTheme}
      title={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
    >
      {#if theme === 'dark'}
        <Icon icon="ph:sun" class="size-4" />
      {:else}
        <Icon icon="ph:moon" class="size-4" />
      {/if}
    </button>

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
        <div class="size-10 rounded-full bg-base-300 border border-base-300 overflow-hidden flex items-center justify-center flex-shrink-0">
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
            <span class="text-xs text-base-content/35 font-normal normal-case tracking-normal">for SteamCMD mod updates</span>
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
            <span class="text-xs text-base-content/35 font-normal normal-case tracking-normal">avatar in titlebar &amp; mod update checks</span>
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
            <span class="text-xs text-base-content/35 font-normal normal-case tracking-normal">player history &amp; server rankings</span>
          </div>
          <div class="bg-base-200/60 rounded-lg border border-base-300/60 overflow-hidden">
            <div class="flex items-center gap-3 px-3 py-2.5">
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
          </div>
          <p class="text-xs text-base-content/35 mt-1.5 px-1">
            Get a personal access token at
            <button
              type="button"
              class="text-primary hover:underline"
              onclick={() => { openUrl('https://www.battlemetrics.com/developers'); }}
            >battlemetrics.com/developers</button>
          </p>
        </div>



      </div>

      <!-- ── Footer ─────────────────────────────────────────────────────────── -->
      <div class="flex items-center justify-between px-5 py-3 border-t border-base-300 bg-base-200 flex-shrink-0">
        <button class="btn btn-ghost btn-sm text-base-content/60" onclick={closeModal}>
          Cancel
        </button>
        <div class="flex items-center gap-2">
          <button
            class="btn btn-ghost btn-sm gap-1.5 text-base-content/50"
            onclick={() => { onOpenExcludedIps(); }}
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

<style>
  .titlebar-logo {
    transition: transform 0.6s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .titlebar-logo--hovered {
    transform: rotate(15deg) scale(1.1);
  }
</style>
