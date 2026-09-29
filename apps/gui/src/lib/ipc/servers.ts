/** The public server list, one server's details, and live queries. */
import { call, type Channel } from "./core";
import type { MapCount, ScanProgress, ServerPage, ServerQuery, ServerRow } from "./bindings";
import type {
  A2sDetailsDto,
  AppStatsDto,
  BattleMetricsDto,
  PingResult,
  ServerFullDto,
} from "./types";

export type InitResult = { server_count: number; from_cache: boolean; is_first_launch: boolean };

export const checkFirstLaunch = () => call<boolean>("check_first_launch");
export const initialize = () => call<InitResult>("initialize");
/** Fetch a fresh list into the backend; the UI only learns how many. */
export const refreshServers = () => call<number>("refresh_servers");

// ── the list, filtered, sorted and paged by the backend ─────────────────
export type {
  MapCount,
  ScanProgress,
  ServerPage,
  ServerQuery,
  ServerRow,
  ServerStats,
  SortCol,
  Tri,
} from "./bindings";

export const serversQuery = (query: ServerQuery) => call<ServerPage>("servers_query", { query });
export const serverMaps = () => call<MapCount[]>("server_maps");
/** Rows for a few addresses ("ip:port", query or game port); null where unknown. */
export const serversLookup = (keys: string[]) => call<(ServerRow | null)[]>("servers_lookup", { keys });
export const startScan = (onProgress: Channel<ScanProgress>) => call<void>("start_scan", { onProgress });
export const getServerDetails = (ip: string, port: number) =>
  call<ServerFullDto>("get_server_details", { ip, port });
export const getAppStats = () => call<AppStatsDto>("get_app_stats");
export const fetchSteamPlayerCount = () => call<number>("fetch_steam_player_count");

export const queryA2s = (ip: string, queryPort: number, gamePort: number | null) =>
  call<A2sDetailsDto>("query_a2s", { ip, queryPort, gamePort });

export const fetchBattleMetrics = (ip: string, port: number, queryPort: number, name: string) =>
  call<BattleMetricsDto>("fetch_battlemetrics_server", { ip, port, queryPort, name });

// ── ping ──────────────────────────────────────────────────────────────────
export const pingServers = (
  targets: string[],
  concurrency: number,
  timeoutMs: number,
  onProgress: Channel<PingResult[]>,
) => call<void>("ping_servers", { targets, concurrency, timeoutMs, onProgress });

export const pingSingle = (ip: string, port: number, timeoutMs: number) =>
  call<number>("ping_single", { ip, port, timeoutMs });
export const getPings = (targets: string[]) => call<PingResult[]>("get_pings", { targets });
export const cancelPing = () => call<void>("cancel_ping");
export const togglePingPause = () => call<boolean>("toggle_ping_pause");
