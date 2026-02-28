// ─── Central reactive state for the entire application ────────────────────────
// Wrapped in a class so that external modules can mutate $state fields
// through property access on the singleton (Svelte 5 exports $state as readonly
// when accessed via `import * as s`, but class fields remain writable).

import type {
  ServerDto,
  ProfileDto,
  InstalledModDto,
  ArticleDto,
  AppStatsDto,
  TabId,
  ConfirmDialog,
  ModOpState,
  ServersFilterState,
  CliArgs,
  A2sDetailsDto,
} from '$lib/types';

import type { ConnectRequest } from '$lib/components/ConnectModal.svelte';

// ── Types exported for action modules ────────────────────────────────────
export type UpdateInfo = {
  version: string;
  currentVersion: string;
  body: string | null;
  date: string | null;
};
export type DownloadEvent =
  | { event: 'Started'; data: { contentLength: number | null } }
  | { event: 'Progress'; data: { chunkLength: number } }
  | { event: 'Finished' };
export type UpdateState = 'idle' | 'checking' | 'up_to_date' | 'available' | 'downloading' | 'done' | 'error';

// ── Constants ────────────────────────────────────────────────────────────
export const MOD_UPDATES_TTL_MS = 5 * 60 * 1000;
export const AWAY_SERVERS_MS = 5 * 60 * 1000;
export const AWAY_MODS_MS = 10 * 60 * 1000;

class AppState {
  // ── Theme ───────────────────────────────────────────────────────────────
  theme = $state<'light' | 'dark'>('dark');

  // ── Global state ────────────────────────────────────────────────────────
  initialized = $state(false);
  initError = $state<string | null>(null);
  showWizard = $state(false);
  showExcludedIpsModal = $state(false);
  activeTab = $state<TabId>('servers');
  servers = $state<ServerDto[]>([]);
  profile = $state<ProfileDto | null>(null);
  installedMods = $state<InstalledModDto[]>([]);
  articles = $state<ArticleDto[]>([]);
  stats = $state<AppStatsDto | null>(null);
  steamPlayers = $state<number | null>(null);
  pingCache = $state<Map<string, number>>(new Map());
  pingFlushPending = false;
  avatarUrl = $state<string | null>(null);
  offlineMissions = $state<string[]>([]);
  offlineStatus = $state('');
  offlineStatusKind = $state<'info' | 'success' | 'error' | 'warning'>('info');

  serversLoading = $state(false);
  serversRefreshing = $state(false);
  modsLoading = $state(false);
  modsChecking = $state(false);
  modUpdatesLastChecked = $state(0);
  newsLoading = $state(false);
  offlineLoading = $state(false);

  confirmDialog = $state<ConfirmDialog | null>(null);
  connectRequest = $state<ConnectRequest | null>(null);

  // ── Persistent filter state ─────────────────────────────────────────────
  serversFilter = $state<ServersFilterState>({
    searchQuery: '',
    filterMap: '',
    filterMods: 'both',
    filterFirstPerson: 'both',
    filterPassword: 'both',
    filterBE: 'both',
    sortCol: 'none',
    sortAsc: true,
  });
  optionsSearch = $state('');
  quickConnectDismissed = $state(false);
  pendingCliArgs = $state<CliArgs | null>(null);
  directConnectPrefill = $state<{ ip: string; port: number; queryPort?: number } | null>(null);
  statusMessage = $state('');
  statusKind = $state<'info' | 'success' | 'error' | 'warning'>('info');

  modOp = $state<ModOpState>({
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

  // ── Last-played banner A2S ──────────────────────────────────────────────
  bannerA2s = $state<A2sDetailsDto | null>(null);
  bannerA2sLoading = $state(false);

  // ── Launcher updater ────────────────────────────────────────────────────
  updateState = $state<UpdateState>('idle');
  updateInfo = $state<UpdateInfo | null>(null);
  updateError = $state('');
  dlReceived = $state(0);
  dlTotal = $state(0);

  // ── Away-time ──────────────────────────────────────────────────────────
  awayAt: number | null = null;
  titleGlitchTick = $state(0);

  // ── Timers ─────────────────────────────────────────────────────────────
  statusTimeout: ReturnType<typeof setTimeout> | null = null;
  modUpdateInterval: ReturnType<typeof setInterval> | null = null;

  // ── Methods ────────────────────────────────────────────────────────────
  setStatus(msg: string, kind: 'info' | 'success' | 'error' | 'warning' = 'info') {
    this.statusMessage = msg;
    this.statusKind = kind;
    if (this.statusTimeout) clearTimeout(this.statusTimeout);
    if (kind !== 'error') {
      this.statusTimeout = setTimeout(() => {
        this.statusMessage = '';
      }, 5000);
    }
  }

  toggleTheme() {
    this.theme = this.theme === 'dark' ? 'light' : 'dark';
    localStorage.setItem('theme', this.theme);
  }

  loadTheme() {
    const saved = localStorage.getItem('theme') as 'light' | 'dark' | null;
    if (saved) this.theme = saved;
  }

  timeIcon(time: string | undefined): string {
    if (!time) return 'ph:sun-horizon';
    const h = parseInt(time.split(':')[0], 10);
    if (isNaN(h)) return 'ph:sun-horizon';
    if (h >= 5 && h < 7) return 'ph:sun-horizon';
    if (h >= 7 && h < 19) return 'ph:sun';
    if (h >= 19 && h < 21) return 'ph:sun-horizon';
    return 'ph:moon';
  }
}

// ── Singleton ────────────────────────────────────────────────────────────
export const app = new AppState();
