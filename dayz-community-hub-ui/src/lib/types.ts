// ─── DTOs mirroring the Rust structs in src-tauri/src/lib.rs ─────────────────

export interface ModDto {
  name: string;
  steam_workshop_id: number;
}

/** Slim server DTO for list display (no mod details). */
export interface ServerDto {
  game_port: number;
  ip: string;
  query_port: number;
  name: string;
  map: string;
  players: number;
  max_players: number;
  environment: string; // "w" = Windows, "l" = Linux
  password: boolean;
  version: string;
  first_person_only: boolean;
  time: string;
  mods_count: number;
  vac: boolean;
  battl_eye: boolean | null;
  /** Bots reported by A2S_INFO, populated at runtime from ping results.
   * DayZ servers often pad this to fake a full server. undefined until pinged. */
  bots?: number;
}

/** Full server DTO with mod details (fetched on demand). */
export interface ServerFullDto extends ServerDto {
  mods: ModDto[];
}

export interface InstalledModDto {
  name: string;
  id: number;
  local_updated: number;
  size: number;
  size_human: string;
  managed: boolean;
  /** Remote time_updated from Steam Workshop API. null if not yet checked. */
  remote_updated: number | null;
  /** True when remote_updated > local_updated. */
  update_available: boolean;
}

export interface FavoriteDto {
  name: string;
  ip: string;
  port: number;
  /** Saved server join password — auto-filled in Direct Connect. */
  password: string | null;
}

export interface HistoryDto {
  name: string;
  ip: string;
  port: number;
  ts: number;
  relative_time: string;
}

export interface LaunchOptionDto {
  key: string;
  enabled: boolean;
  value: string | null;
  description: string;
}

export interface ProfileDto {
  steam_login: string | null;
  steam_password: string | null;
  steam_root: string | null;
  steamcmd_enabled: boolean;
  /** Explicit path to steamcmd binary (overrides auto-detection). */
  steamcmd_path: string | null;
  player: string | null;
  steam_api_key: string | null;
  steam_id: string | null;
  battlemetrics_api_key: string | null;
  /** User location [longitude, latitude] for distance calculation. */
  user_location: [number, number] | null;
  favorites: FavoriteDto[];
  history: HistoryDto[];
  options: LaunchOptionDto[];
  /** IPs excluded from the server browser. */
  excluded_ips: string[];
  /** Ping concurrency level (5-100). */
  ping_concurrency: number;
  /** Auto ping timeout in milliseconds (1000-5000). */
  ping_timeout_auto: number;
  /** Manual ping timeout in milliseconds (1000-30000). */
  ping_timeout_manual: number;
  /** Max consecutive timeouts before stopping auto-retry (0-5). */
  ping_max_retries: number;
  /** Whether to include favorites in auto ping scan. */
  ping_scan_favorites: boolean;
  /** Whether to include history in auto ping scan. */
  ping_scan_history: boolean;
  /** Whether to include all servers in auto ping scan. */
  ping_scan_servers: boolean;
}

/** BattleMetrics server info fetched on demand for the detail panel. */
export interface BattleMetricsDto {
  /** BattleMetrics server ID (used to build the BM page URL). */
  id: string;
  /** Server name from BattleMetrics. */
  name: string;
  /** Global rank (1 = most popular). null if not ranked. */
  rank: number | null;
  /** "online" | "offline" | "dead" */
  status: string;
  /** ISO 3166-1 alpha-2 country code, e.g. "DE". null if unknown. */
  country: string | null;
  /** Server coordinates [longitude, latitude]. null if unavailable. */
  location: [number, number] | null;
  /** Uptime % over last 30 days (0–100). null if not available. */
  uptime: number | null;
  /** Whether the server is private (password protected). */
  private: boolean | null;
  /** Whether this is an official server. */
  official: boolean | null;
  /** Whether third-person view is allowed. */
  third_person: boolean | null;
  /** Whether the server is modded. */
  modded: boolean | null;
  /** Query status: "valid", "invalid", etc. */
  query_status: string | null;
  /** Server's Steam ID. */
  server_steam_id: string | null;
  /** When the server was first seen on BattleMetrics (ISO 8601). */
  created_at: string | null;
  /** Player count data points for the last 24 h: [unix_secs, count] pairs. */
  player_history: [number, number][];
  /** Current player count from BattleMetrics. */
  players: number | null;
  /** Max players from BattleMetrics. */
  max_players: number | null;
}

export interface ArticleDto {
  title: string;
  slug: string;
  excerpt: string | null;
  content_text: string;
  content_html: string;
  date: string;
  url: string;
  image_url: string | null;
  category: string | null;
  author: string | null;
}

export interface A2sPlayerDto {
  name: string;
  score: number;
  duration: number;
}

export interface A2sRuleDto {
  name: string;
  value: string;
}

export interface A2sDetailsDto {
  server_name: string;
  game: string;
  players: number;
  max_players: number;
  /** Bots reported by A2S_INFO (DayZ servers often pad this to fake a full server). */
  bots: number;
  map: string;
  version: string;
  players_list: A2sPlayerDto[];
  /** Mods from server list (empty if server not found in list) */
  mods: ModDto[];
  /** Mod names from A2S rules (fallback for unlisted servers, no workshop IDs) */
  mods_from_a2s?: string[];
  /** Server rules/cvars from A2S rules query (non-mod entries) */
  rules?: A2sRuleDto[];
  /** Actual query port used */
  query_port: number;
  /** Server game port: from server list (priority) or A2S extended info. null if unavailable. */
  game_port: number | null;
}

/** Hardware specs used to recommend optimal launch options. */
export interface SystemSpecsDto {
  logical_cores: number;
  physical_cores: number;
  total_memory_mb: number;
}

export interface AppStatsDto {
  server_count: number;
  total_players: number;
  player_name: string | null;
  steam_login: string | null;
  has_steamcmd: boolean;
  // avatar_url removed — cached locally in frontend from fetch_steam_avatar
}

export interface ModProgressEvent {
  kind:
    | 'shutting_down_steam'
    | 'steam_guard_mobile_required'
    | 'password_required'
    | 'log_line'
    | 'starting'
    | 'done'
    | 'failed'
    | 'finished';
  current: number;
  total: number;
  mod_id: number;
  name: string;
  ok: number;
  failed: number;
  hint: string | null;
  log_line: string | null;
}

export interface PingResult {
  ip: string;
  port: number;
  ms: number;
  players?: number;
  max_players?: number;
  /** Bots reported by A2S_INFO (DayZ servers often pad this to fake a full server). */
  bots?: number;
  /** True when the A2S query failed (timeout or error). */
  failed?: boolean;
}

// ─── App-level UI state ────────────────────────────────────────────────────────

export type TabId = 'servers' | 'favorites' | 'history' | 'mods' | 'news' | 'connect' | 'options' | 'offline' | 'about';

export interface ConfirmDialog {
  title: string;
  message: string;
  onConfirm: () => void;
  /** If set, "No" executes this instead of just closing */
  onDecline?: () => void;
  /** If set, a third "Cancel" button appears that just closes without any action */
  onCancel?: () => void;
  /** Custom label for the confirm button */
  confirmLabel?: string;
  /** Custom label for the decline button */
  declineLabel?: string;
  /** DaisyUI color variant for the confirm button (default: 'warning') */
  confirmVariant?: 'warning' | 'success' | 'error' | 'info' | 'primary';
  /** DaisyUI color variant for the decline button (default: 'ghost') */
  declineVariant?: 'warning' | 'success' | 'error' | 'info' | 'ghost';
}

export interface ServersFilterState {
  searchQuery: string;
  filterMap: string;
  filterMods: 'both' | 'mods-only' | 'no-mods';
  filterFirstPerson: 'both' | 'fp-only' | 'no-fp';
  filterPassword: 'both' | 'no-pwd' | 'pwd-only';
  filterBE: 'both' | 'be-only' | 'no-be';
  sortCol: 'ping' | 'players' | 'name' | 'map' | 'mods' | 'none';
  sortAsc: boolean;
}

export interface CliArgs {
  /** "ip" or "ip:port" from --connect, null if not provided */
  connect: string | null;
  /** true if --reconnect was passed */
  reconnect: boolean;
  /** A .dzch file path or dzch:// URL to auto-connect */
  open: string | null;
}

/** A mod entry inside a .dzch server config. */
export interface DzchMod {
  id: number;
  name: string;
}

/** .dzch server connection config (mirrors Rust DzchConfig). */
export interface DzchConfig {
  version: number;
  ip: string;
  /** Game port (used to connect). */
  port: number;
  /** Query port (used for A2S server info). */
  query_port?: number | null;
  name: string;
  password?: string | null;
  mods: DzchMod[];
}

export interface ModOpState {
  active: boolean;
  phase: 'shutting_down' | 'steam_guard_mobile' | 'password_required' | 'downloading' | 'finished';
  current: number;
  total: number;
  currentName: string;
  completed: Array<{ id: number; name: string; ok: boolean }>;
  ok: number;
  failed: number;
  hint: string | null;
  /** Raw steamcmd output lines */
  log: string[];
}
