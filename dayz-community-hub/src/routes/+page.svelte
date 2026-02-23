<script lang="ts">
  import '../app.css';
  import { invoke, Channel } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { openUrl as shellOpen } from '@tauri-apps/plugin-opener';
  import { save as saveDialog, open as openDialog } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';

  import type {
    ServerDto,
    ServerFullDto,
    ModDto,
    ProfileDto,
    InstalledModDto,
    ArticleDto,
    AppStatsDto,
    FavoriteDto,
    HistoryDto,
    ModProgressEvent,
    PingResult,
    TabId,
    ConfirmDialog,
    ModOpState,
    ServersFilterState,
    CliArgs,
  } from '$lib/types';

  import Icon from '@iconify/svelte';
  import TitleBar from '$lib/components/TitleBar.svelte';
  import TabBar from '$lib/components/TabBar.svelte';
  import ServersTab from '$lib/components/ServersTab.svelte';
  import FavoritesTab from '$lib/components/FavoritesTab.svelte';
  import HistoryTab from '$lib/components/HistoryTab.svelte';
  import ModsTab from '$lib/components/ModsTab.svelte';
  import NewsTab from '$lib/components/NewsTab.svelte';
  import DirectConnectTab from '$lib/components/DirectConnectTab.svelte';
  import OptionsTab from '$lib/components/OptionsTab.svelte';
  import OfflineTab from '$lib/components/OfflineTab.svelte';
  import AboutTab from '$lib/components/AboutTab.svelte';
  import SetupWizard from '$lib/components/SetupWizard.svelte';
  import ConfirmModal from '$lib/components/ConfirmModal.svelte';
  import ProgressModal from '$lib/components/ProgressModal.svelte';

  // ── Theme ─────────────────────────────────────────────────────────────────
  let theme = $state<'light' | 'dark'>('dark');

  function toggleTheme() {
    theme = theme === 'dark' ? 'light' : 'dark';
    localStorage.setItem('theme', theme);
  }

  // ── Global state ──────────────────────────────────────────────────────────
  let initialized = $state(false);
  let initError = $state<string | null>(null);
  let showWizard = $state(false);
  let activeTab = $state<TabId>('servers');
  let servers = $state<ServerDto[]>([]);
  let profile = $state<ProfileDto | null>(null);
  let installedMods = $state<InstalledModDto[]>([]);
  let articles = $state<ArticleDto[]>([]);
  let stats = $state<AppStatsDto | null>(null);
  let steamPlayers = $state<number | null>(null);
  let pingCache = $state<Map<string, number>>(new Map());
  let offlineMissions = $state<string[]>([]);
  let offlineStatus = $state('');
  let offlineStatusKind = $state<'info' | 'success' | 'error' | 'warning'>('info');

  let serversLoading = $state(false);
  let serversRefreshing = $state(false); // guard: prevent concurrent refresh_servers calls
  let modsLoading = $state(false);
  let modsChecking = $state(false);
  // Timestamp of the last successful checkModUpdates call (ms since epoch, 0 = never).
  // Used to throttle the Steam Workshop API call to at most once per 5 minutes.
  let modUpdatesLastChecked = $state(0);
  const MOD_UPDATES_TTL_MS = 5 * 60 * 1000;
  let newsLoading = $state(false);
  let offlineLoading = $state(false);

  let confirmDialog = $state<ConfirmDialog | null>(null);

  // ── Persistent filter state (survives tab switches) ───────────────────────
  let serversFilter = $state<ServersFilterState>({
    searchQuery: '',
    filterMap: '',
    filterMods: 'both',
    filterFirstPerson: 'both',
    filterPassword: 'both',
    filterBE: 'both',
    sortCol: 'none',
    sortAsc: true,
  });
  let optionsSearch = $state('');
  // Quick-connect banner: most recent history entry (null once dismissed).
  let quickConnectDismissed = $state(false);
  let lastHistoryEntry = $derived(
    !quickConnectDismissed ? (profile?.history?.[0] ?? null) : null
  );
  // CLI args received before initialization completed — drained by doInitialize().
  let pendingCliArgs = $state<CliArgs | null>(null);
  let statusMessage = $state('');
  let statusKind = $state<'info' | 'success' | 'error' | 'warning'>('info');

  let modOp = $state<ModOpState>({
    active: false,
    phase: 'downloading',
    current: 0,
    total: 0,
    currentName: '',
    completed: [],
    ok: 0,
    failed: 0,
    hint: null,
  });

  // ── Derived helpers ───────────────────────────────────────────────────────
  let favoritesSet = $derived(
    new Set(profile?.favorites.map((f) => `${f.ip}:${f.port}`) ?? [])
  );

  let tabs = $derived([
    { id: 'servers' as TabId, label: 'Servers', count: servers.length },
    { id: 'favorites' as TabId, label: 'Favorites', count: profile?.favorites.length ?? 0 },
    { id: 'history' as TabId, label: 'History', count: profile?.history.length ?? 0 },
    { id: 'mods' as TabId, label: 'Mods', count: installedMods.length },
    { id: 'news' as TabId, label: 'News' },
    { id: 'connect' as TabId, label: 'Connect' },
    { id: 'options' as TabId, label: 'Options' },
    { id: 'offline' as TabId, label: 'Offline' },
    { id: 'about' as TabId, label: 'About' },
  ]);

  // ── Status helpers ─────────────────────────────────────────────────────────
  let statusTimeout: ReturnType<typeof setTimeout> | null = null;

  function setStatus(msg: string, kind: 'info' | 'success' | 'error' | 'warning' = 'info') {
    statusMessage = msg;
    statusKind = kind;
    if (statusTimeout) clearTimeout(statusTimeout);
    if (kind !== 'error') {
      statusTimeout = setTimeout(() => { statusMessage = ''; }, 5000);
    }
  }

  // ── Data loading ──────────────────────────────────────────────────────────
  async function loadServers() {
    serversLoading = true;
    try {
      servers = await invoke<ServerDto[]>('get_servers');
    } catch (e) {
      setStatus(`Failed to load servers: ${e}`, 'error');
    } finally {
      serversLoading = false;
    }
  }

  async function refreshServers() {
    if (serversRefreshing) return;
    serversRefreshing = true;
    serversLoading = true;
    setStatus('Refreshing server list…', 'info');
    try {
      servers = await invoke<ServerDto[]>('refresh_servers');
      setStatus(`Loaded ${servers.length} servers`, 'success');
      startPinging();
    } catch (e) {
      setStatus(`Refresh failed: ${e}`, 'error');
    } finally {
      serversLoading = false;
      serversRefreshing = false;
    }
  }

  async function loadProfile() {
    try {
      profile = await invoke<ProfileDto>('get_profile');
    } catch (e) {
      setStatus(`Failed to load profile: ${e}`, 'error');
    }
  }

  async function loadMods() {
    modsLoading = true;
    try {
      installedMods = await invoke<InstalledModDto[]>('get_installed_mods');
    } catch (e) {
      setStatus(`Failed to load mods: ${e}`, 'error');
    } finally {
      modsLoading = false;
    }
  }

  async function checkModUpdates(force = false) {
    const now = Date.now();
    if (!force && now - modUpdatesLastChecked < MOD_UPDATES_TTL_MS) return;
    modsChecking = true;
    try {
      installedMods = await invoke<InstalledModDto[]>('check_mod_updates');
      modUpdatesLastChecked = Date.now();
    } catch (e) {
      setStatus(`Update check failed: ${e}`, 'error');
    } finally {
      modsChecking = false;
    }
  }

  let staleModCount = $derived(installedMods.filter(m => m.update_available).length);

  async function loadNews() {
    if (articles.length > 0) return;
    newsLoading = true;
    try {
      articles = await invoke<ArticleDto[]>('fetch_news');
    } catch (e) {
      setStatus(`News fetch failed: ${e}`, 'error');
    } finally {
      newsLoading = false;
    }
  }

  async function loadStats() {
    try {
      stats = await invoke<AppStatsDto>('get_app_stats');
    } catch { /* non-fatal */ }
  }

  async function loadSteamPlayers() {
    try {
      steamPlayers = await invoke<number>('fetch_steam_player_count');
    } catch { /* non-fatal */ }
  }

  async function loadOfflineMissions() {
    offlineLoading = true;
    offlineStatus = '';
    try {
      offlineMissions = await invoke<string[]>('get_offline_missions');
      if (offlineMissions.length === 0) {
        offlineStatus = "No missions found. Click 'Install / Update' to download.";
        offlineStatusKind = 'warning';
      } else {
        offlineStatus = `${offlineMissions.length} mission(s) available`;
        offlineStatusKind = 'success';
      }
    } catch (e) {
      offlineStatus = String(e);
      offlineStatusKind = 'error';
      offlineMissions = [];
    } finally {
      offlineLoading = false;
    }
  }

  // ── Pinging via Channel ───────────────────────────────────────────────────
  function startPinging() {
    // Collect favorites + history endpoints as explicit targets
    const targets: string[] = [];
    if (profile) {
      for (const f of profile.favorites) {
        targets.push(`${f.ip}:${f.port}`);
      }
      for (const h of profile.history) {
        targets.push(`${h.ip}:${h.port}`);
      }
    }

    const onResult = new Channel<PingResult>();
    onResult.onmessage = (msg) => {
      // Mutate in-place: Svelte 5 tracks Map mutations reactively, so we
      // avoid creating a new Map object on every ping result (which would
      // trigger a full re-render of all server rows for each of ~300 pings).
      pingCache.set(`${msg.ip}:${msg.port}`, msg.ms);
    };
    invoke('start_pinging', { targets, onResult }).catch(() => {});
  }

  // ── Initialization ─────────────────────────────────────────────────────────
  async function doInitialize() {
    try {
      // 1. Init backend (creates DayzCtl, loads cache)
      const result = await invoke<{ server_count: number; from_cache: boolean; is_first_launch: boolean }>('initialize');
      initialized = true;
      if (result.is_first_launch) showWizard = true;

      // 2. Load profile + stats + steam players + mods in parallel (fast, no big data)
      await Promise.all([loadProfile(), loadStats(), loadSteamPlayers(), loadMods()]);

      // Drain any CLI args that arrived before profile was ready.
      if (pendingCliArgs) {
        const queued = pendingCliArgs;
        pendingCliArgs = null;
        handleCliArgs(queued);
      }

      // 3b. Fetch Steam avatar in the background (non-blocking)
      invoke('fetch_steam_avatar').then(() => loadStats()).catch(() => {});

      // 3. Load servers from cache (already in backend state)
      serversLoading = true;
      loadServers().then(() => {
        serversLoading = false;
        // 4. Start pinging now that we have servers + profile
        startPinging();
      });

      // 5. Background refresh if cache was used
      if (result.from_cache) {
        // Refresh in background — don't await, don't block UI
        refreshServersBackground();
      }
    } catch (e) {
      initError = String(e);
      setStatus(`Initialization failed: ${e}`, 'error');
    }
  }

  /** Silent background refresh — updates servers without blocking UI */
  async function refreshServersBackground() {
    if (serversRefreshing) return;
    serversRefreshing = true;
    try {
      const freshServers = await invoke<ServerDto[]>('refresh_servers');
      servers = freshServers;
      loadStats(); // Update stats with fresh data
    } catch {
      // Non-fatal — we already have cached data
    } finally {
      serversRefreshing = false;
    }
  }

  // ── Tab switching ─────────────────────────────────────────────────────────
  function selectTab(id: TabId) {
    activeTab = id;
    if (id === 'mods') {
      if (installedMods.length === 0) loadMods().then(() => checkModUpdates());
      else checkModUpdates();
    }
    if (id === 'news' && articles.length === 0) loadNews();
    if (id === 'offline' && offlineMissions.length === 0) loadOfflineMissions();
  }

  // ── Server connect flow ───────────────────────────────────────────────────
  async function connectToServer(server: ServerDto) {
    let missingIds: number[] = [];
    try {
      missingIds = await invoke<number[]>('get_missing_mods', {
        ip: server.ip,
        port: server.query_port,
      });
    } catch { /* non-fatal */ }

    if (missingIds.length > 0) {
      // Need full server details to get mod names
      let fullServer: ServerFullDto | null = null;
      try {
        fullServer = await invoke<ServerFullDto>('get_server_details', { ip: server.ip, port: server.query_port });
      } catch { /* non-fatal */ }

      const modNames = missingIds
        .map((id) => fullServer?.mods.find((m: ModDto) => m.steam_workshop_id === id)?.name ?? String(id))
        .slice(0, 10)
        .join('\n');
      confirmDialog = {
        title: `Install ${missingIds.length} missing mod${missingIds.length > 1 ? 's' : ''}?`,
        message: `${modNames}${missingIds.length > 10 ? `\n…and ${missingIds.length - 10} more` : ''}`,
        confirmLabel: 'Install & connect',
        confirmVariant: 'success',
        declineLabel: 'Connect without mods',
        declineVariant: 'warning',
        onConfirm: () => doInstallAndLaunch(server),
        onDecline: () => doLaunchDirect(server),
        onCancel: () => {},
      };
    } else if (server.mods_count > 0) {
      confirmDialog = {
        title: 'Update mods before connecting?',
        message: `${server.name}\n${server.mods_count} mod(s) installed.`,
        confirmLabel: 'Update & connect',
        confirmVariant: 'success',
        declineLabel: 'Connect without updating',
        declineVariant: 'warning',
        onConfirm: () => doUpdateAndLaunch(server),
        onDecline: () => doLaunchDirect(server),
        onCancel: () => {},
      };
    } else {
      doLaunchDirect(server);
    }
  }

  /** Connect to a server by ip:port directly (for favorites/history that may not be in the server list) */
  function connectByAddress(ip: string, port: number, name: string) {
    // Try to find in server list first
    const server = servers.find(
      (s) => s.ip === ip && (s.query_port === port || s.game_port === port)
    );
    if (server) {
      connectToServer(server);
    } else {
      // Direct connect fallback
      connectDirect(ip, port);
    }
  }

  /**
   * Act on parsed CLI args — called on startup (get_cli_args) and when a
   * second instance fires the "cli-args" event.
   *
   * --connect <ip[:port]>  → switch to Direct Connect tab, pre-fill, and connect
   * --reconnect            → reconnect to the last history entry
   */
  async function handleCliArgs(args: CliArgs) {
    // If profile isn't loaded yet, queue and let doInitialize() drain it.
    if (!initialized) {
      pendingCliArgs = args;
      return;
    }
    if (args.connect) {
      // Parse optional port from "ip:port" form
      const raw = args.connect.trim();
      const lastColon = raw.lastIndexOf(':');
      let ip = raw;
      let port = 2302;
      if (lastColon !== -1) {
        const maybPort = parseInt(raw.slice(lastColon + 1), 10);
        if (!isNaN(maybPort)) {
          ip = raw.slice(0, lastColon);
          port = maybPort;
        }
      }
      selectTab('connect');
      // Small delay so the tab renders before we trigger the connect
      await new Promise((r) => setTimeout(r, 150));
      connectDirect(ip, port);
    } else if (args.reconnect) {
      const last = profile?.history?.[0] ?? null;
      if (last) {
        connectByAddress(last.ip, last.port, last.name);
      } else {
        setStatus('No history entry to reconnect to', 'warning');
      }
    }
  }

  async function doLaunchDirect(server: ServerDto) {
    setStatus(`Launching ${server.name}…`, 'info');
    try {
      await invoke('setup_mod_symlinks', { ip: server.ip, port: server.query_port }).catch(() => {});
      await invoke('launch_server', { ip: server.ip, port: server.query_port, password: null });
      setStatus('Waiting for Steam to open DayZ…', 'info');
    } catch (e) {
      setStatus(`Launch failed: ${e}`, 'error');
    }
  }

  async function doInstallAndLaunch(server: ServerDto) {
    startModOp('install_server', { ip: server.ip, port: server.query_port });
  }

  async function doUpdateAndLaunch(server: ServerDto) {
    startModOp('update_server', { ip: server.ip, port: server.query_port });
  }

  async function connectDirect(ip: string, port: number, password?: string) {
    setStatus(`Connecting to ${ip}:${port}…`, 'info');
    try {
      await invoke('launch_direct', { ip, gamePort: port, password: password ?? null });
      setStatus('Waiting for Steam to open DayZ…', 'info');
    } catch (e) {
      setStatus(`Launch failed: ${e}`, 'error');
    }
  }

  // ── Favorites ─────────────────────────────────────────────────────────────

  /** Append a favorite to local profile state without a round-trip. */
  function profileAddFavorite(name: string, ip: string, port: number) {
    if (!profile) return;
    // Avoid duplicates
    if (!profile.favorites.some((f) => f.ip === ip && f.port === port)) {
      profile.favorites = [...profile.favorites, { name, ip, port }];
    }
  }

  /** Remove a favorite from local profile state without a round-trip. */
  function profileRemoveFavorite(ip: string, port: number) {
    if (!profile) return;
    profile.favorites = profile.favorites.filter((f) => !(f.ip === ip && f.port === port));
  }

  async function addFavorite(server: ServerDto) {
    try {
      profileAddFavorite(server.name, server.ip, server.query_port);
      await invoke('add_favorite', {
        name: server.name,
        ip: server.ip,
        port: server.query_port,
      });
      setStatus(`Added ${server.name} to favorites`, 'success');
    } catch (e) {
      await loadProfile(); // Revert on error
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  async function addFavoriteDirect(name: string, ip: string, port: number) {
    try {
      profileAddFavorite(name, ip, port);
      await invoke('add_favorite', { name, ip, port });
      setStatus(`Added ${name} to favorites`, 'success');
    } catch (e) {
      await loadProfile();
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  function removeFavorite(fav: FavoriteDto) {
    confirmDialog = {
      title: 'Remove Favorite',
      message: `Remove '${fav.name}' from favorites?`,
      onConfirm: async () => {
        try {
          profileRemoveFavorite(fav.ip, fav.port);
          await invoke('remove_favorite', { ip: fav.ip, port: fav.port });
          setStatus('Removed from favorites', 'success');
        } catch (e) {
          await loadProfile();
          setStatus(`Failed: ${e}`, 'error');
        }
      },
    };
  }

  /** Instant (no confirm dialog) remove — used by toggle stars in server/history lists. */
  async function removeFavoriteQuick(ip: string, port: number) {
    try {
      profileRemoveFavorite(ip, port);
      await invoke('remove_favorite', { ip, port });
      setStatus('Removed from favorites', 'success');
    } catch (e) {
      await loadProfile();
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  // ── Profile export / import / reset ──────────────────────────────────────
  async function exportProfile() {
    const path = await saveDialog({
      title: 'Export profile',
      defaultPath: 'dayz-community-hub-profile.dchub',
      filters: [{ name: 'DayZ Community Hub profile', extensions: ['dchub'] }],
    });
    if (!path) return;
    try {
      await invoke('export_profile', { path });
      setStatus('Profile exported successfully', 'success');
    } catch (e) {
      setStatus(`Export failed: ${e}`, 'error');
    }
  }

  async function importProfile() {
    const selected = await openDialog({
      title: 'Import profile',
      multiple: false,
      filters: [{ name: 'DayZ Community Hub profile', extensions: ['dchub'] }],
    });
    if (!selected) return;
    const path = typeof selected === 'string' ? selected : selected[0];
    confirmDialog = {
      title: 'Import Profile',
      message: 'This will overwrite your current profile, favorites, history and launch options. The app will restart to apply the changes. Continue?',
      onConfirm: async () => {
        try {
          await invoke('import_profile', { path });
          await invoke('restart_app');
        } catch (e) {
          setStatus(`Import failed: ${e}`, 'error');
        }
      },
    };
  }

  function resetProfile() {
    confirmDialog = {
      title: 'Reset to Factory Defaults',
      message: 'This will delete all settings, favorites, history, cached server lists, and launch options — as if the app was never launched. Mods downloaded to your Steam workshop folder are not affected. The app will restart and the setup wizard will reappear.',
      onConfirm: async () => {
        try {
          await invoke('reset_profile');
          await invoke('restart_app');
        } catch (e) {
          setStatus(`Reset failed: ${e}`, 'error');
        }
      },
    };
  }

  // ── History ───────────────────────────────────────────────────────────────
  function removeHistoryEntry(h: HistoryDto) {
    confirmDialog = {
      title: 'Remove Entry',
      message: `Remove '${h.name}' from history?`,
      onConfirm: async () => {
        try {
          if (profile) profile.history = profile.history.filter((e) => !(e.ip === h.ip && e.port === h.port));
          await invoke('remove_history_entry', { ip: h.ip, port: h.port });
          setStatus('Removed from history', 'success');
        } catch (e) {
          await loadProfile();
          setStatus(`Failed: ${e}`, 'error');
        }
      },
    };
  }

  function clearHistory() {
    confirmDialog = {
      title: 'Clear History',
      message: `Clear all ${profile?.history.length ?? 0} history entries?`,
      onConfirm: async () => {
        try {
          if (profile) profile.history = [];
          await invoke('clear_history');
          setStatus('History cleared', 'success');
        } catch (e) {
          await loadProfile();
          setStatus(`Failed: ${e}`, 'error');
        }
      },
    };
  }

  async function addFavoriteFromHistory(h: HistoryDto) {
    try {
      profileAddFavorite(h.name, h.ip, h.port);
      await invoke('add_favorite', { name: h.name, ip: h.ip, port: h.port });
      setStatus(`Added ${h.name} to favorites`, 'success');
    } catch (e) {
      await loadProfile();
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  // ── Mods ──────────────────────────────────────────────────────────────────
  function deleteMod(mod_item: InstalledModDto) {
    confirmDialog = {
      title: 'Delete Mod',
      message: `Delete '${mod_item.name}' (${mod_item.id})?\nSize: ${mod_item.size_human}`,
      onConfirm: async () => {
        await invoke('delete_mod', { modId: mod_item.id });
        await loadMods();
        setStatus(`Deleted ${mod_item.name}`, 'success');
      },
    };
  }

  async function toggleModManaged(mod_item: InstalledModDto) {
    try {
      const managed = await invoke<boolean>('toggle_mod_managed', { modId: mod_item.id });
      await loadMods();
      setStatus(`${mod_item.name} marked as ${managed ? 'managed' : 'unmanaged'}`, 'success');
    } catch (e) {
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  function updateMod(mod_item: InstalledModDto) {
    startModOp('update_one', { modId: mod_item.id, modName: mod_item.name });
  }

  function updateAllMods() {
    startModOp('update_all', {});
  }

  function updateStaleMods() {
    startModOp('update_stale', {});
  }

  function cleanupMods() {
    confirmDialog = {
      title: 'Cleanup Mods',
      message: 'Remove all managed mods and symlinks?',
      onConfirm: async () => {
        try {
          const result = await invoke<string>('cleanup_mods');
          await loadMods();
          setStatus(result, 'success');
        } catch (e) {
          setStatus(`Cleanup failed: ${e}`, 'error');
        }
      },
    };
  }

  // ── Options / profile settings ────────────────────────────────────────────
  async function saveProfileSettings(
    player: string | null,
    steamLogin: string | null,
    steamPassword: string | null,
    steamRoot: string | null,
    steamcmdEnabled: boolean,
    steamcmdPath: string | null,
    steamApiKey: string | null,
    steamId: string | null,
    battlemetricsApiKey: string | null,
  ) {
    try {
      await invoke('save_profile_settings', {
        player,
        steamLogin,
        steamPassword,
        steamRoot,
        steamcmdEnabled,
        steamcmdPath,
        steamApiKey,
        steamId,
        battlemetricsApiKey,
      });
      await loadProfile();
      // Re-fetch avatar whenever settings are saved (credentials may have changed)
      await invoke('fetch_steam_avatar').catch(() => {});
      await loadStats();
      setStatus('Settings saved', 'success');
    } catch (e) {
      setStatus(`Failed to save settings: ${e}`, 'error');
    }
  }

  async function toggleOption(key: string) {
    // Optimistically flip the option in local state — backend returns the new bool
    const prev = profile?.options.find((o) => o.key === key)?.enabled;
    if (profile) {
      profile.options = profile.options.map((o) =>
        o.key === key ? { ...o, enabled: !o.enabled } : o
      );
    }
    try {
      await invoke<boolean>('toggle_launch_option', { key });
    } catch (e) {
      // Revert on failure
      if (profile) {
        profile.options = profile.options.map((o) =>
          o.key === key ? { ...o, enabled: prev ?? o.enabled } : o
        );
      }
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  async function setOptionValue(key: string, value: string | null) {
    const prevOpt = profile?.options.find((o) => o.key === key);
    if (profile) {
      profile.options = profile.options.map((o) =>
        o.key === key ? { ...o, value, enabled: value !== null ? true : o.enabled } : o
      );
    }
    try {
      await invoke('set_launch_option_value', { key, value });
    } catch (e) {
      // Revert on failure
      if (profile && prevOpt) {
        profile.options = profile.options.map((o) =>
          o.key === key ? { ...prevOpt } : o
        );
      }
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  // ── Offline mode ──────────────────────────────────────────────────────────
  async function updateOfflineMode() {
    offlineLoading = true;
    offlineStatus = 'Downloading DayZCommunityOfflineMode…';
    offlineStatusKind = 'info';
    await invoke('update_offline_mode').catch((e) => {
      offlineStatus = String(e);
      offlineStatusKind = 'error';
      offlineLoading = false;
    });
  }

  async function launchOfflineMission(mission: string) {
    setStatus(`Launching offline: ${mission}`, 'info');
    invoke('launch_offline_mission', { mission }).catch((e) =>
      setStatus(`Launch failed: ${e}`, 'error')
    );
  }

  // ── Mod operation progress via Channel ────────────────────────────────────
  function startModOp(opType: string, args: Record<string, unknown>) {
    modOp = {
      active: true,
      phase: 'downloading',
      current: 0,
      total: 0,
      currentName: 'Preparing…',
      completed: [],
      ok: 0,
      failed: 0,
      hint: null,
    };

    const onProgress = new Channel<ModProgressEvent>();
    onProgress.onmessage = (payload) => {
      switch (payload.kind) {
        case 'shutting_down_steam':
          modOp.phase = 'shutting_down';
          modOp.currentName = 'Closing Steam…';
          break;
        case 'starting':
          modOp.phase = 'downloading';
          modOp.current = payload.current;
          modOp.total = payload.total;
          modOp.currentName = payload.name;
          break;
        case 'done':
          modOp.current = payload.current;
          modOp.total = payload.total;
          modOp.completed = [...modOp.completed, { id: payload.mod_id, name: payload.name, ok: true }];
          break;
        case 'failed':
          modOp.current = payload.current;
          modOp.total = payload.total;
          modOp.completed = [...modOp.completed, { id: payload.mod_id, name: payload.name, ok: false }];
          break;
        case 'finished':
          modOp.phase = 'finished';
          modOp.ok = payload.ok;
          modOp.failed = payload.failed;
          modOp.hint = payload.hint;
          if (!payload.hint && payload.failed === 0) {
            setStatus(`Mods: ${payload.ok} updated successfully`, 'success');
          } else if (payload.failed > 0) {
            setStatus(`Mods: ${payload.ok} OK, ${payload.failed} failed`, 'warning');
          }
          break;
      }
    };

    invoke('start_mod_operation', { opType, ...args, onProgress }).catch((e) => {
      setStatus(`Mod operation failed: ${e}`, 'error');
      modOp.active = false;
    });
  }

  function dismissModOp() {
    modOp.active = false;
    // Force a fresh update check after a mod operation completes.
    loadMods().then(() => checkModUpdates(true));
    loadStats();
  }

  // ── News & misc ───────────────────────────────────────────────────────────
  function openUrl(url: string) {
    shellOpen(url).catch(() => {
      window.open(url, '_blank');
    });
  }

  // ── Event listeners ───────────────────────────────────────────────────────
  let cleanupFns: Array<() => void> = [];

  function handleGlobalKeydown(e: KeyboardEvent) {
    // Ctrl+1…9 switches tabs
    if (e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey) {
      const n = parseInt(e.key, 10);
      if (n >= 1 && n <= tabs.length) {
        e.preventDefault();
        selectTab(tabs[n - 1].id);
      }
    }
  }

  onMount(() => {
    const saved = localStorage.getItem('theme') as 'light' | 'dark' | null;
    if (saved) theme = saved;

    doInitialize();

    listen<string>('launch-done', ({ payload }) => {
      setStatus(`Launched: ${payload}`, 'success');
      loadProfile();
    }).then((fn) => cleanupFns.push(fn));

    listen<string>('launch-error', ({ payload }) => {
      setStatus(`Launch error: ${payload}`, 'error');
    }).then((fn) => cleanupFns.push(fn));

    listen('offline-mode-updated', () => {
      offlineLoading = false;
      offlineStatus = 'Offline mode updated successfully';
      offlineStatusKind = 'success';
      loadOfflineMissions();
    }).then((fn) => cleanupFns.push(fn));

    listen<string>('offline-mode-error', ({ payload }) => {
      offlineLoading = false;
      offlineStatus = `Update failed: ${payload}`;
      offlineStatusKind = 'error';
    }).then((fn) => cleanupFns.push(fn));

    // CLI args — read once at startup, then listen for a second instance.
    invoke<CliArgs>('get_cli_args').then((args) => {
      if (args.connect || args.reconnect) handleCliArgs(args);
    });
    listen<CliArgs>('cli-args', ({ payload }) => {
      handleCliArgs(payload);
    }).then((fn) => cleanupFns.push(fn));

    return () => {
      cleanupFns.forEach((fn) => fn());
      if (statusTimeout) clearTimeout(statusTimeout);
    };
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="flex flex-col h-screen w-screen overflow-hidden bg-base-100 text-base-content" data-theme={theme} onkeydown={handleGlobalKeydown}>
  <TitleBar {stats} {steamPlayers} {theme} {profile} onToggleTheme={toggleTheme} onSaveSettings={saveProfileSettings} />
  <TabBar {activeTab} {tabs} onSelect={selectTab} />

  {#if statusMessage}
    <div
      class="px-3 py-1 text-xs flex-shrink-0 border-b border-base-300
             {statusKind === 'error' ? 'bg-error/15 text-error' :
              statusKind === 'success' ? 'bg-success/15 text-success' :
              statusKind === 'warning' ? 'bg-warning/15 text-warning' :
              'bg-info/10 text-info'}"
    >
      {statusMessage}
    </div>
  {/if}

  <!-- Quick-connect banner: last played server, shown on Servers tab -->
  {#if lastHistoryEntry && activeTab === 'servers' && initialized}
    {@const _lh = lastHistoryEntry}
    {@const _lhServer = servers.find(s => s.ip === _lh.ip && (s.query_port === _lh.port || s.game_port === _lh.port))}
    {@const _lhPing = _lhServer
      ? (pingCache.get(`${_lhServer.ip}:${_lhServer.query_port}`) ?? pingCache.get(`${_lh.ip}:${_lh.port}`))
      : pingCache.get(`${_lh.ip}:${_lh.port}`)}
    <div class="flex items-center gap-3 px-3 py-1.5 bg-base-200 border-b border-base-300 flex-shrink-0 text-xs">
      <!-- Icon + label -->
      <div class="flex items-center gap-1.5 shrink-0 text-base-content/40">
        <Icon icon="ph:clock-counter-clockwise" class="size-3.5" />
        <span class="uppercase tracking-wide" style="font-size:10px;">Last played</span>
      </div>

      <!-- Server name -->
      <span class="font-semibold text-base-content/85 truncate">{_lh.name}</span>

      <!-- Live stats — only if server is in the list -->
      {#if _lhServer}
        <span class="w-px h-3.5 bg-base-300 shrink-0"></span>
        <!-- Players -->
        <span class="flex items-center gap-1 shrink-0 tabular-nums
                     {_lhServer.players >= _lhServer.max_players ? 'text-error' : _lhServer.players > _lhServer.max_players / 2 ? 'text-warning' : 'text-success'}">
          <Icon icon="ph:users" class="size-3 shrink-0" />
          {_lhServer.players}/{_lhServer.max_players}
        </span>
        <!-- Map -->
        <span class="flex items-center gap-1 shrink-0 text-teal-400/80">
          <Icon icon="ph:map-trifold" class="size-3 shrink-0" />
          {_lhServer.map}
        </span>
        <!-- Ping -->
        {#if _lhPing !== undefined}
          <span class="flex items-center gap-1 shrink-0 tabular-nums font-mono
                       {_lhPing < 50 ? 'text-success' : _lhPing < 100 ? 'text-warning' : 'text-error'}">
            <Icon icon="ph:wave-triangle" class="size-3 shrink-0" />
            {_lhPing}ms
          </span>
        {/if}
        <!-- In-game time -->
        {#if _lhServer.time}
          <span class="flex items-center gap-1 shrink-0 text-base-content/50 tabular-nums font-mono">
            <Icon icon="ph:sun-horizon" class="size-3 shrink-0" />
            {_lhServer.time}
          </span>
        {/if}
      {:else}
        <span class="shrink-0 text-warning/70" style="font-size:10px;" title="Server not found in current list — may be offline">OFFLINE</span>
      {/if}

      <!-- Time -->
      <span class="text-base-content/30 shrink-0 ml-auto" title={new Date(_lh.ts * 1000).toLocaleString()}>
        {_lh.relative_time}
      </span>

      <!-- Reconnect -->
      <button
        class="btn btn-xs btn-primary h-6 min-h-0 px-2.5 shrink-0 gap-1"
        onclick={() => connectByAddress(_lh.ip, _lh.port, _lh.name)}
      >
        <Icon icon="ph:arrow-right" class="size-3" />
        Reconnect
      </button>

      <!-- Dismiss -->
      <button
        class="size-5 flex items-center justify-center rounded text-base-content/30 hover:text-base-content/70 hover:bg-base-300 transition-colors shrink-0"
        onclick={() => (quickConnectDismissed = true)}
        title="Dismiss"
      >
        <Icon icon="ph:x" class="size-3" />
      </button>
    </div>
  {/if}

  <div class="flex-1 overflow-hidden">
    {#if !initialized}
      <div class="flex flex-col items-center justify-center h-full gap-4">
        {#if initError}
          <div class="text-error text-lg font-semibold">Initialization Failed</div>
          <div class="text-error/70 text-sm max-w-md text-center">{initError}</div>
          <button class="btn btn-primary btn-sm" onclick={() => window.location.reload()}>Retry</button>
        {:else}
          <span class="loading loading-spinner loading-lg text-primary"></span>
          <div class="text-base-content/60 text-sm">Initializing…</div>
        {/if}
      </div>
    {:else if activeTab === 'servers'}
      <ServersTab
        {servers}
        {pingCache}
        {installedMods}
        favorites={favoritesSet}
        loading={serversLoading}
        bind:filter={serversFilter}
        bmApiKey={profile?.battlemetrics_api_key ?? null}
        onConnect={connectToServer}
        onAddFavorite={addFavorite}
        onRemoveFavorite={(s) => removeFavoriteQuick(s.ip, s.query_port)}
        onRefresh={refreshServers}
      />
    {:else if activeTab === 'favorites'}
      <FavoritesTab
        favorites={profile?.favorites ?? []}
        {servers}
        {pingCache}
        bmApiKey={profile?.battlemetrics_api_key ?? null}
        onConnect={connectByAddress}
        onRemove={removeFavorite}
        onGoToServers={() => selectTab('servers')}
      />
    {:else if activeTab === 'history'}
      <HistoryTab
        history={profile?.history ?? []}
        {servers}
        {pingCache}
        favorites={favoritesSet}
        bmApiKey={profile?.battlemetrics_api_key ?? null}
        onConnect={connectByAddress}
        onAddFavorite={addFavoriteFromHistory}
        onRemoveFavorite={(h) => removeFavoriteQuick(h.ip, h.port)}
        onRemove={removeHistoryEntry}
        onClearAll={clearHistory}
        onGoToServers={() => selectTab('servers')}
      />
    {:else if activeTab === 'mods'}
      <ModsTab
        mods={installedMods}
        loading={modsLoading}
        checking={modsChecking}
        staleCount={staleModCount}
        onRefresh={() => loadMods().then(() => checkModUpdates(true))}
        onCheckUpdates={() => checkModUpdates(true)}
        onDelete={deleteMod}
        onToggleManaged={toggleModManaged}
        onUpdate={updateMod}
        onUpdateAll={updateAllMods}
        onUpdateStale={updateStaleMods}
        onCleanup={cleanupMods}
      />
    {:else if activeTab === 'news'}
      <NewsTab
        {articles}
        loading={newsLoading}
        onRefresh={() => { articles = []; loadNews(); }}
        onOpenUrl={openUrl}
      />
    {:else if activeTab === 'connect'}
       <DirectConnectTab
         {servers}
         {installedMods}
         favorites={favoritesSet}
         onConnect={connectDirect}
         onAddFavorite={addFavoriteDirect}
       />
    {:else if activeTab === 'options'}
      <OptionsTab
        options={profile?.options ?? []}
        bind:search={optionsSearch}
        onToggle={toggleOption}
        onSetValue={setOptionValue}
      />
    {:else if activeTab === 'offline'}
      <OfflineTab
        missions={offlineMissions}
        loading={offlineLoading}
        status={offlineStatus}
        statusKind={offlineStatusKind}
        onRefresh={loadOfflineMissions}
        onUpdate={updateOfflineMode}
        onLaunch={launchOfflineMission}
      />
    {:else if activeTab === 'about'}
      <AboutTab
        onExport={exportProfile}
        onImport={importProfile}
        onReset={resetProfile}
      />
    {/if}
  </div>

  <ConfirmModal dialog={confirmDialog} onClose={() => (confirmDialog = null)} />
  <ProgressModal state={modOp} onDismiss={dismissModOp} />

  {#if showWizard}
    <SetupWizard onDone={async () => {
      showWizard = false;
      await loadProfile();
      // Re-fetch avatar with the newly saved API key + Steam ID, then refresh stats
      invoke('fetch_steam_avatar').then(() => loadStats()).catch(() => {});
      await loadStats();
    }} />
  {/if}
</div>
