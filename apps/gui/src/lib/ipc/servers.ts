/** The public server list, one server's details, and live queries. */
import { commands, run, type Channel } from "./core";
import type { MapCount, ScanProgress, ServerQuery } from "./bindings";
import type { InitResult, PingResult } from "./types";

export type { InitResult };

export const checkFirstLaunch = () => run(commands.checkFirstLaunch());
export const initialize = () => run(commands.initialize());
/** Fetch a fresh list into the backend; the UI only learns how many. */
export const refreshServers = () => run(commands.refreshServers());

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

export const serversQuery = (query: ServerQuery) => run(commands.serversQuery(query));
export const serverMaps = (): Promise<MapCount[]> => run(commands.serverMaps());
/** Rows for a few addresses ("ip:port", query or game port); null where unknown. */
export const serversLookup = (keys: string[]) => run(commands.serversLookup(keys));
/** The backend pings every server in its own order; only progress comes back. */
export const startScan = (onProgress: Channel<ScanProgress>) => run(commands.startScan(onProgress));
export const getServerDetails = (ip: string, queryPort: number) =>
  run(commands.getServerDetails(ip, queryPort));
export const getAppStats = () => run(commands.getAppStats());
export const fetchSteamPlayerCount = () => run(commands.fetchSteamPlayerCount());

export const queryA2s = (ip: string, queryPort: number, gamePort: number | null) =>
  run(commands.queryA2s(ip, queryPort, gamePort));


/** DayZ Metrics' long view of a server (rank, schedules, fake verdict, 24 h); no key needed. */
export const fetchServerMetrics = (ip: string, gamePort: number, queryPort: number) =>
  run(commands.fetchServerMetrics(ip, gamePort, queryPort));

// ── pings of explicit, small lists ──────────────────────────────────────
export const pingServers = (
  targets: string[],
  concurrency: number | null,
  timeoutMs: number | null,
  onProgress: Channel<PingResult[]>,
) => run(commands.pingServers(targets, concurrency, timeoutMs, onProgress));
export const pingSingle = (ip: string, port: number, timeoutMs: number | null) =>
  run(commands.pingSingle(ip, port, timeoutMs));
export const getPings = (targets: string[]) => run(commands.getPings(targets));
export const cancelPing = () => run(commands.cancelPing());
export const togglePingPause = () => run(commands.togglePingPause());
