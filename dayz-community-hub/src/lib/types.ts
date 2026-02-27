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
  favorites: FavoriteDto[];
  history: HistoryDto[];
  options: LaunchOptionDto[];
  /** IPs excluded from the server browser. */
  excluded_ips: string[];
}

/** BattleMetrics server info fetched on demand for the detail panel. */
export interface BattleMetricsDto {
  /** BattleMetrics server ID (used to build the BM page URL). */
  id: string;
  /** Global rank (1 = most popular). null if not ranked. */
  rank: number | null;
  /** "online" | "offline" | "dead" */
  status: string;
  /** ISO 3166-1 alpha-2 country code, e.g. "DE". null if unknown. */
  country: string | null;
  /** Uptime % over last 30 days (0–100). null if not available. */
  uptime: number | null;
  /** Player count data points for the last 24 h: [unix_secs, count] pairs. */
  player_history: [number, number][];
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

export interface A2sDetailsDto {
  server_name: string;
  game: string;
  players: number;
  max_players: number;
  map: string;
  version: string;
  players_list: A2sPlayerDto[];
  /** Mods from server list (empty if server not found in list) */
  mods: ModDto[];
  /** Actual query port used */
  query_port: number;
  /** Server game port: from server list (priority) or A2S extended info. null if unavailable. */
  game_port: number | null;
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
  kind: 'shutting_down_steam' | 'steam_guard_mobile_required' | 'password_required' | 'log_line' | 'starting' | 'done' | 'failed' | 'finished';
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
}

// ─── App-level UI state ────────────────────────────────────────────────────────

export type TabId =
  | 'servers'
  | 'favorites'
  | 'history'
  | 'mods'
  | 'news'
  | 'connect'
  | 'options'
  | 'offline'
  | 'about';

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
