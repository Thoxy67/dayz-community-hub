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
    A2sDetailsDto,
    DzchConfig,
  } from '$lib/types';
  // PingResult is used by the ping-batch event listener below

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
  import ConnectModal, { type ConnectRequest } from '$lib/components/ConnectModal.svelte';
  import ExcludedIpsModal from '$lib/components/ExcludedIpsModal.svelte';

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
  let showExcludedIpsModal = $state(false);
  let activeTab = $state<TabId>('servers');
  let servers = $state<ServerDto[]>([]);
  let profile = $state<ProfileDto | null>(null);
  let installedMods = $state<InstalledModDto[]>([]);
  let articles = $state<ArticleDto[]>([]);
  let stats = $state<AppStatsDto | null>(null);
  let steamPlayers = $state<number | null>(null);
  let pingCache = $state<Map<string, number>>(new Map());
  // Dirty flag: ping results were written into the map but Svelte hasn't been
  // notified yet. A single rAF-scheduled flush does the notification.
  let pingFlushPending = false;
  let avatarUrl = $state<string | null>(null);
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
  let connectRequest = $state<ConnectRequest | null>(null);

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
  // Prefill payload for the Direct Connect tab (set by pressing D in server/fav/history tabs).
  let directConnectPrefill = $state<{ ip: string; port: number } | null>(null);
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
    log: [],
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
    { id: 'about' as TabId, label: 'About', icon: 'mdi:information', pushRight: true },
  ]);

  // ── Status helpers ─────────────────────────────────────────────────────────
  let statusTimeout: ReturnType<typeof setTimeout> | null = null;
  let modUpdateInterval: ReturnType<typeof setInterval> | null = null;

  // ── Away-time refresh thresholds ──────────────────────────────────────────
  const AWAY_SERVERS_MS  = 5  * 60 * 1000; // 5 min  → refresh server list + re-ping
  const AWAY_MODS_MS     = 10 * 60 * 1000; // 10 min → re-check mod updates
  let awayAt: number | null = null;         // timestamp when the window lost focus/visibility
  let titleGlitchTick = $state(0);          // increment to trigger TitleBar glitch animation

  function handleWindowHide() {
    awayAt = Date.now();
  }

  function handleWindowShow() {
    if (awayAt === null) return;
    const elapsed = Date.now() - awayAt;
    awayAt = null;
    // Always fire the title glitch on focus return
    titleGlitchTick += 1;
    if (elapsed >= AWAY_SERVERS_MS) {
      refreshServers();
    }
    if (elapsed >= AWAY_MODS_MS && profile?.steam_api_key) {
      checkModUpdates();
    }
  }

  function timeIcon(time: string | undefined): string {
    if (!time) return 'ph:sun-horizon';
    const h = parseInt(time.split(':')[0], 10);
    if (isNaN(h)) return 'ph:sun-horizon';
    if (h >= 5  && h < 7)  return 'ph:sun-horizon';
    if (h >= 7  && h < 19) return 'ph:sun';
    if (h >= 19 && h < 21) return 'ph:sun-horizon';
    return 'ph:moon';
  }

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

  // ── Launcher update checker (shared between TitleBar badge and AboutTab) ───
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
    | 'idle' | 'checking' | 'up_to_date' | 'available' | 'downloading' | 'done' | 'error';

  let updateState  = $state<UpdateState>('idle');
  let updateInfo   = $state<UpdateInfo | null>(null);
  let updateError  = $state('');
  let dlReceived   = $state(0);
  let dlTotal      = $state(0);
  let dlPercent    = $derived(dlTotal > 0 ? Math.round((dlReceived / dlTotal) * 100) : 0);

  async function checkForUpdate() {
    updateState = 'checking';
    updateError = '';
    try {
      const info = await invoke<UpdateInfo | null>('check_for_update');
      if (info) { updateInfo = info; updateState = 'available'; }
      else       { updateState = 'up_to_date'; }
    } catch (e) { updateError = String(e); updateState = 'error'; }
  }

  async function installUpdate() {
    updateState = 'downloading';
    dlReceived  = 0;
    dlTotal     = 0;
    updateError = '';
    const onEvent = new Channel<DownloadEvent>();
    onEvent.onmessage = (ev) => {
      if      (ev.event === 'Started')  { dlTotal = ev.data.contentLength ?? 0; }
      else if (ev.event === 'Progress') { dlReceived += ev.data.chunkLength; }
      else if (ev.event === 'Finished') { updateState = 'done'; }
    };
    try {
      await invoke('install_update', { onEvent });
    } catch (e) { updateError = String(e); updateState = 'error'; }
  }

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

  // ── Last-played banner A2S refresh ───────────────────────────────────────

  let bannerA2s = $state<A2sDetailsDto | null>(null);
  let bannerA2sLoading = $state(false);

  // Reset A2S cache whenever the last history entry changes (different server).
  $effect(() => {
    profile?.history?.[0]?.ip;
    bannerA2s = null;
  });

  async function refreshBannerA2s() {
    const lh = profile?.history?.[0] ?? null;
    if (!lh) return;
    const sv = servers.find(s => s.ip === lh.ip && (s.query_port === lh.port || s.game_port === lh.port));
    const queryPort = sv ? sv.query_port : lh.port;
    bannerA2sLoading = true;
    try {
      bannerA2s = await invoke<A2sDetailsDto>('query_a2s', { ip: lh.ip, port: queryPort });
    } catch {
      // silently ignore — stale data stays visible
    } finally {
      bannerA2sLoading = false;
    }
  }

  // ── Pinging ───────────────────────────────────────────────────────────────

  /** Ping a single server and update the reactive cache in-place.
   *  Uses a 10 s timeout (vs 5 s for background batch) so manual pings
   *  have a better chance of getting a reply from a slow server.
   *  On failure the key is removed from the cache so the cell shows TIMEOUT.
   *  Display layer treats any stored value >= 5000ms as TIMEOUT too.
   */
  async function pingSingle(ip: string, port: number) {
    const key = `${ip}:${port}`;
    try {
      const ms = await invoke<number>('ping_single', { ip, port, timeoutMs: 10_000 });
      pingCache.set(key, ms);
      pingCache = new Map(pingCache);
    } catch {
      // Remove stale entry so the cell shows OFFLINE instead of a stale value.
      pingCache.delete(key);
      pingCache = new Map(pingCache);
    }
  }

  function startPinging() {
    // Build a lookup: "ip:game_port" | "ip:query_port" → query_port
    // so we always ping (and cache) under the query_port key — the same key
    // all tabs use for pingCache lookups.
    const portMap = new Map<string, number>();
    for (const s of servers) {
      portMap.set(`${s.ip}:${s.game_port}`,  s.query_port);
      portMap.set(`${s.ip}:${s.query_port}`, s.query_port);
    }

    const seen = new Set<string>();
    const targets: string[] = [];

    function addTarget(ip: string, port: number) {
      const resolvedPort = portMap.get(`${ip}:${port}`) ?? port;
      const key = `${ip}:${resolvedPort}`;
      if (!seen.has(key)) {
        seen.add(key);
        targets.push(key);
      }
    }

    if (profile) {
      for (const f of profile.favorites) addTarget(f.ip, f.port);
      for (const h of profile.history)   addTarget(h.ip, h.port);
    }

    // Fire-and-forget: results arrive via 'ping-batch' events (see onMount listener)
    invoke('start_pinging', { targets }).catch(() => {});
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

      // 2b. Background mod update check — populates staleModCount for the titlebar badge.
      // Fire-and-forget: non-blocking, non-fatal. Requires Steam API key.
      if (profile?.steam_api_key) checkModUpdates();

      // 2c. Background launcher update check — populates titlebar badge.
      // Fire-and-forget: backend returns null on non-Windows, no-op there.
      checkForUpdate();

      // Drain any CLI args that arrived before profile was ready.
      if (pendingCliArgs) {
        const queued = pendingCliArgs;
        pendingCliArgs = null;
        handleCliArgs(queued);
      }

      // 3b. Fetch Steam avatar in the background — result stored locally,
      // no need to re-call loadStats() just to get the avatar. Requires Steam API key + Steam ID.
      if (profile?.steam_api_key && profile?.steam_id) {
        invoke<string | null>('fetch_steam_avatar').then((url) => { avatarUrl = url; }).catch(() => {});
      }

      // 3. Load servers from cache (already in backend state)
      serversLoading = true;
      const fromCache = result.from_cache;
      loadServers().then(() => {
        serversLoading = false;
        // 4. Start pinging now that we have servers + profile
        startPinging();
        // 5. Background refresh if cache was used — only after loadServers
        // completes to avoid a race where refreshServersBackground finishes
        // first and its result gets overwritten by the stale cached list.
        if (fromCache) {
          refreshServersBackground();
        }
      });
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
      if (installedMods.length === 0) loadMods().then(() => { if (profile?.steam_api_key) checkModUpdates(); });
      else if (profile?.steam_api_key) checkModUpdates();
    }
    if (id === 'news' && articles.length === 0) loadNews();
    if (id === 'offline' && offlineMissions.length === 0) loadOfflineMissions();
  }

  // ── Server connect flow ───────────────────────────────────────────────────
  async function connectToServer(server: ServerDto) {
    // Fetch full server details (pure in-memory lookup on backend — fast).
    // We always need this when mods are involved, so fetch upfront.
    let fullServer: ServerFullDto | null = null;
    if (server.mods_count > 0) {
      try {
        fullServer = await invoke<ServerFullDto>('get_server_details', { ip: server.ip, port: server.query_port });
      } catch { /* non-fatal */ }
    }

    // Compute missing mods locally from the already-loaded installedMods list —
    // avoids a filesystem scan on every Connect click (was: get_missing_mods IPC call).
    const installedIds = new Set(installedMods.map((m) => m.id));
    const missingIds: number[] = (fullServer?.mods ?? [])
      .map((m: ModDto) => m.steam_workshop_id)
      .filter((id: number) => !installedIds.has(id));

    if (missingIds.length > 0) {
      connectRequest = {
        serverName: server.name,
        kind: 'missing',
        modCount: missingIds.length,
        onConnect: (updateMods) => {
          if (updateMods) doInstallAndLaunch(server);
          else doLaunchDirect(server);
        },
      };
    } else if (server.mods_count > 0) {
      connectRequest = {
        serverName: server.name,
        kind: 'update',
        modCount: server.mods_count,
        onConnect: (updateMods) => {
          if (updateMods) doUpdateAndLaunch(server);
          else doLaunchDirect(server);
        },
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
   * <file.dzch> or dzch:// → parse config, switch to Direct Connect, auto-connect
   */
  async function handleCliArgs(args: CliArgs) {
    // If profile isn't loaded yet, queue and let doInitialize() drain it.
    if (!initialized) {
      pendingCliArgs = args;
      return;
    }

    if (args.open) {
      await handleDzchOpen(args.open);
    } else if (args.connect) {
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

  /**
   * Handle a `.dzch` file path or `dzch://` URL: parse the config, switch
   * to Direct Connect, and call connectDirect() which will prompt the user
   * to install/update missing mods before launching.
   */
  async function handleDzchOpen(raw: string) {
    try {
      let config: DzchConfig;
      if (raw.startsWith('dzch://')) {
        config = await invoke<DzchConfig>('parse_dzch_url', { url: raw });
      } else {
        // It's a file path
        config = await invoke<DzchConfig>('read_dzch_file', { path: raw });
      }

      selectTab('connect');
      await new Promise((r) => setTimeout(r, 150));
      connectDirect(
        config.ip,
        config.port,
        config.password ?? undefined,
      );
    } catch (e) {
      setStatus(`Failed to open .dzch config: ${e}`, 'error');
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
    startModOp(
      'install_server',
      { ip: server.ip, port: server.query_port },
      () => doLaunchDirect(server),
    );
  }

  async function doUpdateAndLaunch(server: ServerDto) {
    startModOp(
      'update_server',
      { ip: server.ip, port: server.query_port },
      () => doLaunchDirect(server),
    );
  }

  async function doLaunchDirectByAddress(ip: string, port: number, password?: string, extraArgs?: string[]) {
    setStatus(`Launching ${ip}:${port}…`, 'info');
    try {
      await invoke('launch_direct', {
        ip,
        gamePort: port,
        password: password ?? null,
        extraArgs: extraArgs ?? null,
      });
      setStatus('Waiting for Steam to open DayZ…', 'info');
    } catch (e) {
      setStatus(`Launch failed: ${e}`, 'error');
    }
  }

  async function connectDirect(ip: string, port: number, password?: string, extraArgs?: string[]) {
    // Mirror connectToServer: check for missing/outdated mods and use the same
    // steamcmd download path (update_selected) so profile credentials are reused.

    // Try to find the server in the list first (gives us mods + query port)
    const inList = servers.find(
      (s) => s.ip === ip && (s.query_port === port || s.game_port === port)
    );

    // Gather server mods — from the list entry or from any prior A2S query result
    // the DirectConnectTab already fetched (exposed via installedMods cross-ref).
    // We only act on mods we know about; if none are known we launch directly.
    let serverMods: ModDto[] = [];
    if (inList && inList.mods_count > 0) {
      try {
        const full = await invoke<ServerFullDto>('get_server_details', {
          ip,
          port: inList.query_port,
        });
        serverMods = full.mods;
      } catch { /* non-fatal */ }
    }

    if (serverMods.length === 0) {
      // No mod info available — launch directly (private server not in list)
      doLaunchDirectByAddress(ip, port, password, extraArgs);
      return;
    }

    const installedIds = new Set(installedMods.map((m) => m.id));
    const missingIds = serverMods
      .map((m) => m.steam_workshop_id)
      .filter((id) => !installedIds.has(id));

    const serverName = inList?.name ?? `${ip}:${port}`;

    // Build (id, name) maps for progress display
    const allModIds   = serverMods.map((m) => m.steam_workshop_id);
    const allModNames = serverMods.map((m) => m.name || String(m.steam_workshop_id));
    const missingNames = serverMods
      .filter((m) => !installedIds.has(m.steam_workshop_id))
      .map((m) => m.name || String(m.steam_workshop_id));

    if (missingIds.length > 0) {
      connectRequest = {
        serverName,
        kind: 'missing',
        modCount: missingIds.length,
        onConnect: (updateMods) => {
          if (updateMods) {
            startModOp(
              'update_selected',
              { modIds: missingIds, modNames: missingNames },
              () => doLaunchDirectByAddress(ip, port, password, extraArgs),
            );
          } else {
            doLaunchDirectByAddress(ip, port, password, extraArgs);
          }
        },
      };
    } else if (serverMods.length > 0) {
      connectRequest = {
        serverName,
        kind: 'update',
        modCount: serverMods.length,
        onConnect: (updateMods) => {
          if (updateMods) {
            startModOp(
              'update_selected',
              { modIds: allModIds, modNames: allModNames },
              () => doLaunchDirectByAddress(ip, port, password, extraArgs),
            );
          } else {
            doLaunchDirectByAddress(ip, port, password, extraArgs);
          }
        },
      };
    } else {
      doLaunchDirectByAddress(ip, port, password, extraArgs);
    }
  }

  // ── Open in Direct Connect ────────────────────────────────────────────────

  /** Switch to the Direct Connect tab with prefilled address + port and auto-query. */
  function openInDirectConnect(ip: string, queryPort: number) {
    directConnectPrefill = { ip, port: queryPort };
    selectTab('connect');
  }

  // ── Favorites ─────────────────────────────────────────────────────────────

  /** Append a favorite to local profile state without a round-trip. */
  function profileAddFavorite(name: string, ip: string, port: number, password?: string) {
    if (!profile) return;
    const existing = profile.favorites.find((f) => f.ip === ip && f.port === port);
    if (existing) {
      existing.name = name;
      if (password !== undefined) existing.password = password || null;
      profile.favorites = [...profile.favorites]; // trigger reactivity
    } else {
      profile.favorites = [...profile.favorites, { name, ip, port, password: password ?? null }];
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
        password: null,
      });
      setStatus(`Added ${server.name} to favorites`, 'success');
    } catch (e) {
      await loadProfile(); // Revert on error
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  async function addFavoriteDirect(name: string, ip: string, port: number, password?: string) {
    try {
      profileAddFavorite(name, ip, port, password);
      await invoke('add_favorite', { name, ip, port, password: password ?? null });
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

  // ── IP exclusion ──────────────────────────────────────────────────────────

  let excludedIpsSet = $derived(new Set<string>(profile?.excluded_ips ?? []));

  async function excludeIp(ip: string) {
    if (!profile) return;
    // Optimistic local update
    if (!profile.excluded_ips.includes(ip)) {
      profile.excluded_ips = [...profile.excluded_ips, ip];
    }
    try {
      await invoke('add_excluded_ip', { ip });
      setStatus(`${ip} excluded from server list`, 'info');
    } catch (e) {
      await loadProfile();
      setStatus(`Failed to exclude IP: ${e}`, 'error');
    }
  }

  async function unexcludeIp(ip: string) {
    if (!profile) return;
    // Optimistic local update
    profile.excluded_ips = profile.excluded_ips.filter((e) => e !== ip);
    try {
      await invoke('remove_excluded_ip', { ip });
      setStatus(`${ip} removed from exclusions`, 'success');
    } catch (e) {
      await loadProfile();
      setStatus(`Failed: ${e}`, 'error');
    }
  }

  // ── Profile export / import / reset ──────────────────────────────────────
  async function exportProfile(includeMods: boolean) {
    const path = await saveDialog({
      title: 'Export profile',
      defaultPath: 'dayz-community-hub-profile.dchub',
      filters: [{ name: 'DayZ Community Hub profile', extensions: ['dchub'] }],
    });
    if (!path) return;
    try {
      await invoke('export_profile', { path, includeMods });
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
      await invoke('add_favorite', { name: h.name, ip: h.ip, port: h.port, password: null });
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

  function deleteSelectedMods(ids: number[]) {
    if (ids.length === 0) return;
    const totalSize = installedMods
      .filter(m => ids.includes(m.id))
      .reduce((acc, m) => acc + m.size, 0);
    const sizeMb = (totalSize / 1024 / 1024);
    const sizeStr = sizeMb >= 1024
      ? `${(sizeMb / 1024).toFixed(1)} GB`
      : `${sizeMb.toFixed(1)} MB`;
    confirmDialog = {
      title: 'Delete Mods',
      message: `Delete ${ids.length} mod${ids.length > 1 ? 's' : ''}?\nTotal size: ${sizeStr}`,
      confirmLabel: `Delete ${ids.length}`,
      confirmVariant: 'error',
      onConfirm: async () => {
        try {
          await invoke('delete_mods_bulk', { modIds: ids });
          await loadMods();
          setStatus(`Deleted ${ids.length} mod${ids.length > 1 ? 's' : ''}`, 'success');
        } catch (e) {
          setStatus(`Delete failed: ${e}`, 'error');
        }
      },
    };
  }

  function updateSelectedMods(ids: number[]) {
    if (ids.length === 0) return;
    startModOp('update_selected', { modIds: ids });
  }

  function installMods(workshopIds: number[]) {
    if (workshopIds.length === 0) return;
    startModOp('install_manual', {
      modIds: workshopIds,
      modNames: workshopIds.map(String),
    });
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
      // Run reloads in parallel — avatar fetch only when API key + Steam ID are set.
      const tasks: Promise<unknown>[] = [loadProfile(), loadStats()];
      if (steamApiKey && steamId) {
        tasks.push(invoke<string | null>('fetch_steam_avatar').then((url) => { avatarUrl = url; }).catch(() => {}));
      } else {
        avatarUrl = null; // Clear stale avatar if keys were removed
      }
      await Promise.all(tasks);
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

  function removeOfflineMode() {
    confirmDialog = {
      title: 'Remove Offline Mode',
      message: 'Delete all DayZCommunityOfflineMode mission folders?\nThis cannot be undone. You can reinstall with "Install / Update".',
      confirmLabel: 'Remove all',
      confirmVariant: 'error',
      onConfirm: async () => {
        try {
          offlineLoading = true;
          offlineStatus = 'Removing offline mode…';
          offlineStatusKind = 'info';
          const n = await invoke<number>('remove_offline_mode');
          await loadOfflineMissions();
          offlineStatus = n > 0 ? `Removed ${n} mission folder${n > 1 ? 's' : ''}` : 'Nothing to remove';
          offlineStatusKind = 'success';
        } catch (e) {
          offlineStatus = `Remove failed: ${e}`;
          offlineStatusKind = 'error';
          offlineLoading = false;
        }
      },
    };
  }

  function clearOfflineSaves() {
    confirmDialog = {
      title: 'Clear Saves',
      message: 'Delete all offline save data (storage_-1/) for every mission?\nThis wipes loot, player state and world state. The missions themselves are kept.',
      confirmLabel: 'Clear saves',
      confirmVariant: 'warning',
      onConfirm: async () => {
        try {
          const n = await invoke<number>('clear_offline_saves');
          offlineStatus = n > 0 ? `Cleared saves for ${n} mission${n > 1 ? 's' : ''}` : 'No saves found';
          offlineStatusKind = 'success';
        } catch (e) {
          offlineStatus = `Clear failed: ${e}`;
          offlineStatusKind = 'error';
        }
      },
    };
  }

  // ── Mod operation progress via Channel ────────────────────────────────────
  function startModOp(
    opType: string,
    args: Record<string, unknown>,
    onSuccess?: () => void,
  ) {
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
      log: [],
    };

    const onProgress = new Channel<ModProgressEvent>();
    onProgress.onmessage = (payload) => {
      switch (payload.kind) {
        case 'shutting_down_steam':
          modOp.phase = 'shutting_down';
          modOp.currentName = 'Closing Steam…';
          break;
        case 'steam_guard_mobile_required':
          modOp.phase = 'steam_guard_mobile';
          break;
        case 'password_required':
          modOp.phase = 'password_required';
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
        case 'log_line':
          if (payload.log_line) {
            modOp.log = [...modOp.log, payload.log_line];
          }
          break;
        case 'finished':
          modOp.phase = 'finished';
          modOp.ok = payload.ok;
          modOp.failed = payload.failed;
          modOp.hint = payload.hint;
          if (!payload.hint && payload.failed === 0) {
            setStatus(`Mods: ${payload.ok} updated successfully`, 'success');
            // Fire the post-success callback (e.g. launch the game)
            onSuccess?.();
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
    // checkModUpdates scans the filesystem and returns a full enriched mod list,
    // so it subsumes loadMods() — no need to call both.
    // Without a Steam API key we still need to reload the mod list from disk.
    if (profile?.steam_api_key) checkModUpdates(true);
    else loadMods();
    loadStats();
  }

  async function sendSteamcmdPassword(password: string) {
    try {
      await invoke('send_steamcmd_input', { input: password });
    } catch (e) {
      setStatus(`Failed to send password: ${e}`, 'error');
    }
  }

  async function cancelModOperation() {
    try {
      await invoke('cancel_mod_operation');
    } catch (e) {
      console.warn('cancel_mod_operation:', e);
    }
    dismissModOp();
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
    // Skip shortcuts when the user is typing in an input/textarea
    const tag = (e.target as HTMLElement)?.tagName;
    const isInput = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';

    if (e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey) {
      // Ctrl+1…9 switches tabs
      const n = parseInt(e.key, 10);
      if (n >= 1 && n <= tabs.length) {
        e.preventDefault();
        selectTab(tabs[n - 1].id);
        return;
      }
      // Ctrl+R — refresh server list
      if (e.key === 'r' && !isInput) {
        e.preventDefault();
        refreshServers();
        return;
      }
      // Ctrl+U — update mods that need updating
      if (e.key === 'u' && !isInput) {
        e.preventDefault();
        if (staleModCount > 0) {
          updateStaleMods();
        } else {
          setStatus('All mods are up to date', 'success');
        }
        return;
      }
      // Ctrl+L — launch latest played server (reconnect)
      if (e.key === 'l' && !isInput) {
        e.preventDefault();
        const last = profile?.history?.[0] ?? null;
        if (last) {
          connectByAddress(last.ip, last.port, last.name);
        } else {
          setStatus('No history entry to reconnect to', 'warning');
        }
        return;
      }
    }
  }

  onMount(() => {
    // Register global keyboard shortcuts on window so they work regardless
    // of which element has focus.
    window.addEventListener('keydown', handleGlobalKeydown);
    cleanupFns.push(() => window.removeEventListener('keydown', handleGlobalKeydown));

    // Away-time detection — visibilitychange is the most reliable signal in
    // Tauri/WebKit (covers both window switching and OS-level focus loss).
    // window blur/focus acts as a fallback for cases the visibility API misses.
    const onVisibilityChange = () =>
      document.hidden ? handleWindowHide() : handleWindowShow();
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('blur',  handleWindowHide);
    window.addEventListener('focus', handleWindowShow);
    cleanupFns.push(() => {
      document.removeEventListener('visibilitychange', onVisibilityChange);
      window.removeEventListener('blur',  handleWindowHide);
      window.removeEventListener('focus', handleWindowShow);
    });
    const saved = localStorage.getItem('theme') as 'light' | 'dark' | null;
    if (saved) theme = saved;

    doInitialize();

    // Ping results arrive in batches from the Rust background task.
    // We write directly into the Map on every event (cheap), but only trigger
    // Svelte reactivity once per animation frame — no matter how many events
    // arrive in the same frame, the UI re-renders exactly once.
    listen<PingResult[]>('ping-batch', ({ payload }) => {
      for (const r of payload) {
        pingCache.set(`${r.ip}:${r.port}`, r.ms);
      }
      if (!pingFlushPending) {
        pingFlushPending = true;
        requestAnimationFrame(() => {
          pingCache = new Map(pingCache);
          pingFlushPending = false;
        });
      }
    }).then((fn) => cleanupFns.push(fn));

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

    // Periodic mod update check — every 30 minutes, only when a Steam API key is set.
    modUpdateInterval = setInterval(() => {
      if (profile?.steam_api_key) {
        checkModUpdates();
      }
    }, 30 * 60 * 1000);

    return () => {
      cleanupFns.forEach((fn) => fn());
      if (statusTimeout) clearTimeout(statusTimeout);
      if (modUpdateInterval) clearInterval(modUpdateInterval);
    };
  });
</script>

<div class="flex flex-col h-screen w-screen overflow-hidden bg-base-100 text-base-content" data-theme={theme}>
  <TitleBar
    {stats} {avatarUrl} {steamPlayers} {theme} {profile}
    {staleModCount}
    {updateState}
    glitchTick={titleGlitchTick}
    onToggleTheme={toggleTheme}
    onSaveSettings={saveProfileSettings}
    onUnexcludeIp={unexcludeIp}
    onOpenExcludedIps={() => { showExcludedIpsModal = true; }}
    onUpdateMods={() => { selectTab('mods'); updateStaleMods(); }}
    onGoToUpdate={() => selectTab('about')}
  />
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
        {@const _players = bannerA2s?.players ?? _lhServer.players}
        {@const _maxPlayers = bannerA2s?.max_players ?? _lhServer.max_players}
        <span class="w-px h-3.5 bg-base-300 shrink-0"></span>
        <!-- Players — click to refresh via A2S -->
        <button
          class="flex items-center gap-1 shrink-0 tabular-nums cursor-pointer hover:opacity-70 transition-opacity
                 {_players >= _maxPlayers ? 'text-error' : _players > _maxPlayers / 2 ? 'text-warning' : 'text-success'}"
          onclick={refreshBannerA2s}
          title="Click to refresh player count"
          disabled={bannerA2sLoading}
        >
          {#if bannerA2sLoading}
            <span class="loading loading-spinner" style="width:10px;height:10px;"></span>
          {:else}
            <Icon icon="ph:users" class="size-3 shrink-0" />
          {/if}
          {_players}/{_maxPlayers}
        </button>
        <!-- Map -->
        <span class="flex items-center gap-1 shrink-0 text-teal-400/80">
          <Icon icon="ph:map-trifold" class="size-3 shrink-0" />
          {_lhServer.map}
        </span>
        <!-- Ping — click to re-ping -->
        <button
          class="flex items-center gap-1 shrink-0 tabular-nums font-mono cursor-pointer hover:opacity-70 transition-opacity
                 {_lhPing !== undefined
                   ? (_lhPing < 50 ? 'text-success' : _lhPing < 100 ? 'text-warning' : 'text-error')
                   : 'text-base-content/30'}"
          onclick={() => pingSingle(_lhServer.ip, _lhServer.query_port)}
          title="Click to ping"
        >
          <Icon icon="ph:wave-triangle" class="size-3 shrink-0" />
          {_lhPing !== undefined ? `${_lhPing}ms` : '—'}
        </button>
        <!-- In-game time (from server list, not A2S) -->
        {#if _lhServer.time}
          <span class="flex items-center gap-1 shrink-0 text-base-content/50 tabular-nums font-mono">
            <Icon icon={timeIcon(_lhServer.time)} class="size-3 shrink-0" />
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
        excludedIps={excludedIpsSet}
        loading={serversLoading}
        bind:filter={serversFilter}
        bmApiKey={profile?.battlemetrics_api_key ?? null}
        onConnect={connectToServer}
        onAddFavorite={addFavorite}
        onRemoveFavorite={(s) => removeFavoriteQuick(s.ip, s.query_port)}
        onRefresh={refreshServers}
        onPing={pingSingle}
        onExcludeIp={excludeIp}
        onUnexcludeIp={unexcludeIp}
        onManageExcluded={() => { showExcludedIpsModal = true; }}
        onDirectConnect={openInDirectConnect}
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
        onPing={pingSingle}
        onDirectConnect={openInDirectConnect}
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
        onPing={pingSingle}
        onDirectConnect={openInDirectConnect}
      />
    {:else if activeTab === 'mods'}
      <ModsTab
        mods={installedMods}
        loading={modsLoading}
        checking={modsChecking}
        staleCount={staleModCount}
        onRefresh={() => loadMods().then(() => { if (profile?.steam_api_key) checkModUpdates(true); })}
        onCheckUpdates={() => checkModUpdates(true)}
        steamApiKey={profile?.steam_api_key ?? ''}
        onDelete={deleteMod}
        onToggleManaged={toggleModManaged}
        onUpdate={updateMod}
        onUpdateAll={updateAllMods}
        onUpdateStale={updateStaleMods}
        onCleanup={cleanupMods}
        onOpenWorkshopDir={() => invoke('open_workshop_dir').catch(() => {})}
        onOpenModDir={(mod) => invoke('open_mod_dir', { modId: mod.id }).catch(() => {})}
        onDeleteSelected={deleteSelectedMods}
        onUpdateSelected={updateSelectedMods}
        onInstallMods={installMods}
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
         favoriteList={profile?.favorites ?? []}
         onConnect={connectDirect}
         onAddFavorite={addFavoriteDirect}
         prefill={directConnectPrefill}
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
        onRemoveOfflineMode={removeOfflineMode}
        onClearSaves={clearOfflineSaves}
        onOpenMissionDir={(mission) => invoke('open_mission_dir', { mission }).catch(() => {})}
      />
    {:else if activeTab === 'about'}
      <AboutTab
        onExport={exportProfile}
        onImport={importProfile}
        onReset={resetProfile}
        {updateState}
        {updateInfo}
        {updateError}
        {dlPercent}
        {dlReceived}
        {dlTotal}
        onCheckForUpdate={checkForUpdate}
        onInstallUpdate={installUpdate}
      />
    {/if}
  </div>

  <ConfirmModal dialog={confirmDialog} onClose={() => (confirmDialog = null)} />
  <ConnectModal request={connectRequest} onClose={() => (connectRequest = null)} />
  <ProgressModal
    modOp={modOp}
    onDismiss={dismissModOp}
    onSendPassword={sendSteamcmdPassword}
    steamLogin={profile?.steam_login}
    onDontTrust={cancelModOperation}
    onCancel={cancelModOperation}
  />

  {#if showExcludedIpsModal}
    <ExcludedIpsModal
      excludedIps={profile?.excluded_ips ?? []}
      onUnexclude={unexcludeIp}
      onClose={() => { showExcludedIpsModal = false; }}
    />
  {/if}

  {#if showWizard}
    <SetupWizard onDone={async () => {
      showWizard = false;
      await Promise.all([loadProfile(), loadStats()]);
      // Fetch avatar only when the user provided both keys during setup.
      if (profile?.steam_api_key && profile?.steam_id) {
        invoke<string | null>('fetch_steam_avatar').then((url) => { avatarUrl = url; }).catch(() => {});
      }
    }} />
  {/if}
</div>
