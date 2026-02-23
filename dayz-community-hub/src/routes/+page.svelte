<script lang="ts">
  import '../app.css';
  import { invoke, Channel } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { openUrl as shellOpen } from '@tauri-apps/plugin-opener';
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
  } from '$lib/types';

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
  let modsLoading = $state(false);
  let modsChecking = $state(false);
  let newsLoading = $state(false);
  let offlineLoading = $state(false);

  let confirmDialog = $state<ConfirmDialog | null>(null);
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

  async function checkModUpdates() {
    modsChecking = true;
    try {
      installedMods = await invoke<InstalledModDto[]>('check_mod_updates');
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
      pingCache = new Map(pingCache).set(`${msg.ip}:${msg.port}`, msg.ms);
    };
    invoke('start_pinging', { targets, onResult }).catch(() => {});
  }

  // ── Initialization ─────────────────────────────────────────────────────────
  async function doInitialize() {
    try {
      // 1. Init backend (creates DayzCtl, loads cache)
      const result = await invoke<{ server_count: number; from_cache: boolean }>('initialize');
      initialized = true;

      // 2. Load profile + stats + steam players + mods in parallel (fast, no big data)
      await Promise.all([loadProfile(), loadStats(), loadSteamPlayers(), loadMods()]);

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
    try {
      const freshServers = await invoke<ServerDto[]>('refresh_servers');
      servers = freshServers;
      loadStats(); // Update stats with fresh data
    } catch {
      // Non-fatal — we already have cached data
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

  async function doLaunchDirect(server: ServerDto) {
    setStatus(`Launching ${server.name}…`, 'info');
    try {
      await invoke('setup_mod_symlinks', { ip: server.ip, port: server.query_port }).catch(() => {});
      await invoke('launch_server', { ip: server.ip, port: server.query_port, password: null });
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

  function connectDirect(ip: string, port: number, password?: string) {
    setStatus(`Connecting to ${ip}:${port}…`, 'info');
    invoke('launch_direct', { ip, gamePort: port, password: password ?? null })
      .catch((e) => setStatus(`Launch failed: ${e}`, 'error'));
  }

  // ── Favorites ─────────────────────────────────────────────────────────────
  async function addFavorite(server: ServerDto) {
    try {
      await invoke('add_favorite', {
        name: server.name,
        ip: server.ip,
        port: server.query_port,
      });
      await loadProfile();
      setStatus(`Added ${server.name} to favorites`, 'success');
    } catch (e) {
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  async function addFavoriteDirect(name: string, ip: string, port: number) {
    try {
      await invoke('add_favorite', { name, ip, port });
      await loadProfile();
      setStatus(`Added ${name} to favorites`, 'success');
    } catch (e) {
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  function removeFavorite(fav: FavoriteDto) {
    confirmDialog = {
      title: 'Remove Favorite',
      message: `Remove '${fav.name}' from favorites?`,
      onConfirm: async () => {
        await invoke('remove_favorite', { ip: fav.ip, port: fav.port });
        await loadProfile();
        setStatus('Removed from favorites', 'success');
      },
    };
  }

  /** Instant (no confirm dialog) remove — used by toggle stars in server/history lists. */
  async function removeFavoriteQuick(ip: string, port: number) {
    try {
      await invoke('remove_favorite', { ip, port });
      await loadProfile();
      setStatus('Removed from favorites', 'success');
    } catch (e) {
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  // ── Profile export / import / reset ──────────────────────────────────────
  async function exportProfile() {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const path = await save({
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
    const { open } = await import('@tauri-apps/plugin-dialog');
    const selected = await open({
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
      title: 'Reset Profile',
      message: 'Reset all settings, favorites, history and launch options to defaults? Installed mods on disk are not affected. The app will restart to apply the changes.',
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
        await invoke('remove_history_entry', { ip: h.ip, port: h.port });
        await loadProfile();
        setStatus('Removed from history', 'success');
      },
    };
  }

  function clearHistory() {
    confirmDialog = {
      title: 'Clear History',
      message: `Clear all ${profile?.history.length ?? 0} history entries?`,
      onConfirm: async () => {
        await invoke('clear_history');
        await loadProfile();
        setStatus('History cleared', 'success');
      },
    };
  }

  async function addFavoriteFromHistory(h: HistoryDto) {
    try {
      await invoke('add_favorite', { name: h.name, ip: h.ip, port: h.port });
      await loadProfile();
      setStatus(`Added ${h.name} to favorites`, 'success');
    } catch (e) {
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
    try {
      await invoke('toggle_launch_option', { key });
      await loadProfile();
    } catch (e) {
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  async function setOptionValue(key: string, value: string | null) {
    try {
      await invoke('set_launch_option_value', { key, value });
      await loadProfile();
    } catch (e) {
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
    loadMods().then(() => checkModUpdates());
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

    return () => {
      cleanupFns.forEach((fn) => fn());
    };
  });
</script>

<div class="flex flex-col h-screen w-screen overflow-hidden bg-base-100 text-base-content" data-theme={theme}>
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
        onConnect={connectByAddress}
        onRemove={removeFavorite}
      />
    {:else if activeTab === 'history'}
      <HistoryTab
        history={profile?.history ?? []}
        {servers}
        {pingCache}
        favorites={favoritesSet}
        onConnect={connectByAddress}
        onAddFavorite={addFavoriteFromHistory}
        onRemoveFavorite={(h) => removeFavoriteQuick(h.ip, h.port)}
        onRemove={removeHistoryEntry}
        onClearAll={clearHistory}
      />
    {:else if activeTab === 'mods'}
      <ModsTab
        mods={installedMods}
        loading={modsLoading}
        checking={modsChecking}
        staleCount={staleModCount}
        onRefresh={() => loadMods().then(() => checkModUpdates())}
        onCheckUpdates={checkModUpdates}
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
</div>
