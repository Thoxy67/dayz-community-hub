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

// ── Theme definitions ────────────────────────────────────────────────────
export type ThemeName =
  | 'dark'
  | 'light'
  | 'midnight'
  | 'forest'
  | 'blood-moon'
  | 'military'
  | 'oled'
  | 'sepia'
  | 'ocean'
  | 'sand'
  | 'rose'
  | 'mint'
  | 'lavender'
  | 'catppuccin-latte'
  | 'twilight'
  | 'mocha'
  | 'catppuccin'
  | 'catppuccin-frappe'
  | 'kanagawa'
  | 'tokyonight'
  | 'tokyonight-storm'
  | 'custom';

export const THEMES: { id: ThemeName; label: string; icon: string; isLight?: boolean; isMixed?: boolean }[] = [
  // Dark themes
  { id: 'dark', label: 'Dark', icon: 'ph:moon' },
  { id: 'midnight', label: 'Midnight', icon: 'ph:moon-stars' },
  { id: 'forest', label: 'Forest', icon: 'ph:tree' },
  { id: 'blood-moon', label: 'Blood Moon', icon: 'ph:drop' },
  { id: 'military', label: 'Military', icon: 'ph:shield-chevron' },
  { id: 'oled', label: 'OLED Black', icon: 'ph:circle-half' },
  { id: 'sepia', label: 'Sepia', icon: 'ph:film-strip' },
  { id: 'catppuccin', label: 'Catppuccin Mocha', icon: 'ph:cat' },
  { id: 'kanagawa', label: 'Kanagawa', icon: 'game-icons:big-wave' },
  { id: 'tokyonight', label: 'Tokyo Night', icon: 'ph:city' },
  // Light themes
  { id: 'light', label: 'Light', icon: 'ph:sun', isLight: true },
  { id: 'ocean', label: 'Ocean', icon: 'ph:waves', isLight: true },
  { id: 'sand', label: 'Sand', icon: 'ph:sun-horizon', isLight: true },
  { id: 'rose', label: 'Rose', icon: 'ph:flower-lotus', isLight: true },
  { id: 'mint', label: 'Mint', icon: 'ph:leaf', isLight: true },
  { id: 'lavender', label: 'Lavender', icon: 'ph:butterfly', isLight: true },
  { id: 'catppuccin-latte', label: 'Catppuccin Latte', icon: 'ph:cat', isLight: true },
  // Mixed themes
  { id: 'twilight', label: 'Twilight', icon: 'ph:cloud-sun', isMixed: true },
  { id: 'mocha', label: 'Mocha', icon: 'ph:coffee', isMixed: true },
  { id: 'catppuccin-frappe', label: 'Catppuccin Frappé', icon: 'ph:cat', isMixed: true },
  { id: 'tokyonight-storm', label: 'Tokyo Night Storm', icon: 'ph:cloud-lightning', isMixed: true },
];

class AppState {
  // ── Theme ───────────────────────────────────────────────────────────────
  theme = $state<ThemeName>('dark');

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

  setTheme(name: ThemeName) {
    this.theme = name;
    localStorage.setItem('theme', name);
  }

  loadTheme() {
    const saved = localStorage.getItem('theme') as ThemeName | null;
    if (saved && THEMES.some((t) => t.id === saved)) {
      this.theme = saved;
    }
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
