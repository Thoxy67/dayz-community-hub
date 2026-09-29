/** The public server list, one server's details, and live queries. */
import { call, type Channel } from "./core";
import type {
  A2sDetailsDto,
  AppStatsDto,
  BattleMetricsDto,
  PingResult,
  ServerDto,
  ServerFullDto,
} from "./types";

export type InitResult = { server_count: number; from_cache: boolean; is_first_launch: boolean };

export const checkFirstLaunch = () => call<boolean>("check_first_launch");
export const initialize = () => call<InitResult>("initialize");
export const getServers = () => call<ServerDto[]>("get_servers");
export const refreshServers = () => call<ServerDto[]>("refresh_servers");
export const getServerDetails = (ip: string, port: number) =>
  call<ServerFullDto>("get_server_details", { ip, port });
export const getAppStats = () => call<AppStatsDto>("get_app_stats");
export const fetchSteamPlayerCount = () => call<number>("fetch_steam_player_count");

export const queryA2s = (ip: string, queryPort: number, gamePort: number | null) =>
  call<A2sDetailsDto>("query_a2s", { ip, queryPort, gamePort });

export const fetchBattleMetrics = (ip: string, port: number, queryPort: number, name: string) =>
  call<BattleMetricsDto>("fetch_battlemetrics_server", { ip, port, queryPort, name });

// ── ping ──────────────────────────────────────────────────────────────────
export const pingAllBackground = (
  targets: string[],
  concurrency: number,
  timeoutMs: number,
  onProgress: Channel<PingResult[]>,
) => call<void>("ping_all_background", { targets, concurrency, timeoutMs, onProgress });

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
