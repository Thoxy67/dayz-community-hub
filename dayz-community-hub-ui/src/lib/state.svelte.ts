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

import type { ConnectRequest, ConnectModInfo } from '$lib/components/ConnectModal.svelte';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';

// ── Pending server mod operation (for confirmation modal during connection) ──
export type PendingServerModOp = {
  mods: ConnectModInfo[];
  mode: 'update' | 'install';
  onConfirm: () => void;
} | null;

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

// ── Toast notification types ──────────────────────────────────────────────
export type ToastKind = 'info' | 'success' | 'error' | 'warning';
export type Toast = {
  id: number;
  message: string;
  kind: ToastKind;
  dismissAt: number; // Timestamp when auto-dismiss should fire
};

// ── Constants ────────────────────────────────────────────────────────────
export const MOD_UPDATES_TTL_MS = 5 * 60 * 1000;
export const AWAY_SERVERS_MS = 5 * 60 * 1000;
export const AWAY_MODS_MS = 10 * 60 * 1000;
export const STALE_SERVERS_MS = 10 * 60 * 1000; // 10 minutes for stale data warning
export const TOAST_MAX_VISIBLE = 3;
export const TOAST_DISMISS_MS = 5000; // Auto-dismiss after 5s (errors stay)

// ── Theme definitions ────────────────────────────────────────────────────
// Only base themes are defined here. All other themes are managed via
// theme-presets.ts and applied as 'custom' with injected CSS.
export type ThemeName = 'dark' | 'light' | 'custom';

export const THEMES: { id: ThemeName; label: string; icon: string; isLight?: boolean }[] = [
  { id: 'dark', label: 'Dark', icon: 'ph:moon' },
  { id: 'light', label: 'Light', icon: 'ph:sun', isLight: true },
  { id: 'custom', label: 'Custom', icon: 'ph:palette' },
];

class AppState {
  // ── Theme ───────────────────────────────────────────────────────────────
  theme = $state<ThemeName>('dark');

  // ── Window settings (independent from theme) ────────────────────────────
  windowRadius = $state('0');
  windowBorderSize = $state('0');
  windowBorderFocus = $state('oklch(45% 0.12 250)');
  windowBorderBlur = $state('oklch(25% 0.02 250)');

  // ── Global state ────────────────────────────────────────────────────────
  initialized = $state(false);
  initError = $state<string | null>(null);
  showWizard = $state(false);
  showExcludedIpsModal = $state(false);
  activeTab = $state<TabId>('servers');
  servers = $state<ServerDto[]>([]);
  dayzavrServers = $state<import('$lib/types').DayzavrServer[]>([]);
  dayzavrLoading = $state(false);
  /** Mod folder names fully installed under !Workshop (for Play-button gating). */
  dayzavrInstalledMods = $state<string[]>([]);
  /** Active DayZavr mod install/update progress, or null when idle. */
  dayzavrInstall = $state<{
    active: boolean;
    serverName: string;
    status: string;
    downloadedBytes: number;
    totalBytes: number;
    uploadedBytes: number;
    downloadMbps: number;
    uploadMbps: number;
    peersLive: number;
    peersConnecting: number;
    peersSeen: number;
    eta: string | null;
    done: boolean;
    error: string | null;
  } | null>(null);
  profile = $state<ProfileDto | null>(null);
  installedMods = $state<InstalledModDto[]>([]);
  articles = $state<ArticleDto[]>([]);
  stats = $state<AppStatsDto | null>(null);
  steamPlayers = $state<number | null>(null);
  // SvelteMap / SvelteSet are natively reactive, so .set / .add / .delete
  // trigger updates without having to allocate fresh `new Map(…)` /
  // `new Set(…)` on every mutation.  This was the dominant alloc cost
  // during full-scan ping waves (3 alloc-and-copy ops per RAF frame ×
  // hundreds of frames).
  pingCache = new SvelteMap<string, number>();
  pingPending = new SvelteSet<string>();
  pingTimeouts = new SvelteMap<string, number>();
  a2sFailures = new SvelteSet<string>();
  pingFlushPending = false;
  /** Tracks active background ping session for progress bar */
  pingSession = $state<{ active: boolean; total: number; completed: number } | null>(null);
  /** True when ping scanning is paused */
  pingPaused = $state(false);
  avatarUrl = $state<string | null>(null);
  offlineMissions = $state<string[]>([]);
  offlineStatus = $state('');
  offlineStatusKind = $state<'info' | 'success' | 'error' | 'warning'>('info');

  serversLoading = $state(false);
  serversRefreshing = $state(false);
  serversLastRefreshed = $state(0); // Timestamp of last server refresh
  modsLoading = $state(false);
  modsChecking = $state(false);
  modUpdatesLastChecked = $state(0);
  newsLoading = $state(false);
  offlineLoading = $state(false);

  confirmDialog = $state<ConfirmDialog | null>(null);
  connectRequest = $state<ConnectRequest | null>(null);
  pendingServerModOp = $state<PendingServerModOp>(null);

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
  historyAutoOpenDetail = $state(false);
  pendingCliArgs = $state<CliArgs | null>(null);
  directConnectPrefill = $state<{ ip: string; port: number; queryPort?: number } | null>(null);
  // Legacy single status (kept for backwards compatibility)
  statusMessage = $state('');
  statusKind = $state<'info' | 'success' | 'error' | 'warning'>('info');
  // ── Toast notification queue ──────────────────────────────────────────────
  toasts = $state<Toast[]>([]);
  private _toastIdCounter = 0;

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

  /** Add a toast notification to the queue (max 3 visible, FIFO) */
  addToast(message: string, kind: ToastKind = 'info'): number {
    const id = ++this._toastIdCounter;
    const dismissAt = kind === 'error' ? Infinity : Date.now() + TOAST_DISMISS_MS;
    const toast: Toast = { id, message, kind, dismissAt };

    // Add to queue, limit to max visible
    this.toasts = [...this.toasts, toast].slice(-TOAST_MAX_VISIBLE);

    // Schedule auto-dismiss for non-error toasts
    if (kind !== 'error') {
      setTimeout(() => this.dismissToast(id), TOAST_DISMISS_MS);
    }

    return id;
  }

  /** Dismiss a toast by ID */
  dismissToast(id: number): void {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }

  /** Clear all toasts */
  clearToasts(): void {
    this.toasts = [];
  }

  /** Legacy setStatus — now adds a toast instead of replacing */
  setStatus(msg: string, kind: 'info' | 'success' | 'error' | 'warning' = 'info') {
    // Keep legacy single status for backwards compatibility
    this.statusMessage = msg;
    this.statusKind = kind;
    if (this.statusTimeout) clearTimeout(this.statusTimeout);
    if (kind !== 'error') {
      this.statusTimeout = setTimeout(() => {
        this.statusMessage = '';
      }, 5000);
    }
    // Also add as toast for the new stacking UI
    this.addToast(msg, kind);
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

  loadWindowSettings() {
    const saved = localStorage.getItem('window-settings');
    if (saved) {
      try {
        const parsed = JSON.parse(saved);
        if (parsed.windowRadius !== undefined) this.windowRadius = parsed.windowRadius;
        if (parsed.windowBorderSize !== undefined) this.windowBorderSize = parsed.windowBorderSize;
        if (parsed.windowBorderFocus !== undefined) this.windowBorderFocus = parsed.windowBorderFocus;
        if (parsed.windowBorderBlur !== undefined) this.windowBorderBlur = parsed.windowBorderBlur;
      } catch {
        // Invalid JSON, ignore
      }
    }
  }

  saveWindowSettings() {
    localStorage.setItem(
      'window-settings',
      JSON.stringify({
        windowRadius: this.windowRadius,
        windowBorderSize: this.windowBorderSize,
        windowBorderFocus: this.windowBorderFocus,
        windowBorderBlur: this.windowBorderBlur,
      }),
    );
  }

  // Check if this is the first run (no theme has been set yet)
  isFirstRun(): boolean {
    return !localStorage.getItem('theme') && !localStorage.getItem('active-preset-id');
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
