<script lang="ts">
  import type { AppStatsDto, ProfileDto } from '$lib/types';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { open as openDialog } from '@tauri-apps/plugin-dialog';
  import Icon from '@iconify/svelte';

  interface Props {
    stats: AppStatsDto | null;
    steamPlayers: number | null;
    theme: 'light' | 'dark';
    profile: ProfileDto | null;
    onToggleTheme: () => void;
    onSaveSettings: (
      player: string | null,
      steamLogin: string | null,
      steamPassword: string | null,
      steamRoot: string | null,
      steamcmdEnabled: boolean,
      steamApiKey: string | null,
      steamId: string | null,
    ) => void;
  }

  let { stats, steamPlayers, theme, profile, onToggleTheme, onSaveSettings }: Props = $props();

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
  let modalOpen     = $state(false);
  let playerName    = $state('');
  let steamLogin    = $state('');
  let steamPassword = $state('');
  let steamRoot     = $state('');
  let steamApiKey   = $state('');
  let steamId       = $state('');
  let showPassword  = $state(false);
  let showApiKey    = $state(false);

  function openModal() {
    playerName    = profile?.player ?? '';
    steamLogin    = profile?.steam_login ?? '';
    steamPassword = profile?.steam_password ?? '';
    steamRoot     = profile?.steam_root ?? '';
    steamApiKey   = profile?.steam_api_key ?? '';
    steamId       = profile?.steam_id ?? '';
    showPassword  = false;
    showApiKey    = false;
    modalOpen     = true;
  }

  function closeModal() { modalOpen = false; }

  function handleOk() {
    onSaveSettings(
      playerName.trim() || null,
      steamLogin.trim() || null,
      steamPassword || null,
      steamRoot.trim() || null,
      profile?.steamcmd_enabled ?? true,
      steamApiKey.trim() || null,
      steamId.trim() || null,
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

  function fmt(n: number | null | undefined): string {
    return n == null ? '—' : n.toLocaleString();
  }
</script>

<!-- ── Title bar ──────────────────────────────────────────────────────────── -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="h-9 flex items-center bg-base-200 border-b border-base-300 flex-shrink-0 select-none"
  onmousedown={onTitlebarMousedown}
>

  <!-- Left: identity -->
  <div class="flex items-center gap-2 px-4 pr-4 border-r border-base-300">
    <img src="/icon.svg" class="w-5 h-5" alt="icon" />
    <span class="text-sm font-semibold text-base-content tracking-tight">DayZ Community Hub</span>
  </div>

  <!-- Center: live stats -->
  <div class="flex items-center gap-5 px-4 text-xs text-base-content/60">
    <span class="flex items-center gap-1.5">
      <Icon icon="mdi:server-network" class="size-3.5 text-base-content/40" />
      <span class="tabular-nums font-medium text-base-content/80">{fmt(stats?.server_count)}</span>
      <span>servers</span>
    </span>
    <span class="flex items-center gap-1.5">
      <Icon icon="mdi:controller" class="size-3.5 text-base-content/40" />
      <span class="tabular-nums font-medium text-base-content/80">{fmt(stats?.total_players)}</span>
      <span>in-game</span>
    </span>
    <span class="flex items-center gap-1.5">
      <Icon icon="mdi:steam" class="size-3.5 text-base-content/40" />
      <span class="tabular-nums font-medium text-base-content/80">{fmt(steamPlayers)}</span>
      <span>on Steam</span>
    </span>
  </div>

  <!-- Spacer -->
  <div class="flex-1"></div>

  <!-- Right: user + theme + window controls -->
  <div class="flex items-center gap-1 text-xs">

    {#if stats && !stats.has_steamcmd}
      <span class="flex items-center gap-1 text-warning mr-2">
        <Icon icon="ph:warning" class="size-3.5" />
        <span>SteamCMD not found</span>
      </span>
    {/if}

    <!-- User chip -->
    <button
      class="flex items-center gap-1.5 px-2 py-1 rounded hover:bg-base-300 text-base-content/70 hover:text-primary transition-colors border-r border-base-300 mr-1"
      onclick={openModal}
      title="Edit account settings"
    >
      {#if stats?.avatar_url}
        <img
          src={stats.avatar_url}
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
          {#if stats?.avatar_url}
            <img src={stats.avatar_url} alt="Steam avatar" class="w-full h-full object-cover" />
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
              </div>
            </div>
            <!-- Steam root -->
            <div class="flex items-center gap-3 px-3 py-2.5">
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

        <!-- ── Section: Avatar ────────────────────────────────────────────── -->
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

      </div>

      <!-- ── Footer ────────────────────────────────────────────────────────── -->
      <div class="flex items-center justify-between px-5 py-3 border-t border-base-300 bg-base-200 flex-shrink-0">
        <button class="btn btn-ghost btn-sm text-base-content/60" onclick={closeModal}>
          Cancel
        </button>
        <button class="btn btn-primary btn-sm gap-1.5" onclick={handleOk}>
          <Icon icon="ph:check" class="size-3.5" />
          Save changes
        </button>
      </div>

    </div>
  </div>
{/if}
