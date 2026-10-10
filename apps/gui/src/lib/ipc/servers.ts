/** The public server list, one server's details, and live queries. */
import { commands, run, type Channel } from "./core";
import type { MapCount, ServerQuery } from "./bindings";
import type { InitResult, PingResult } from "./types";

export type { InitResult };

export const checkFirstLaunch = () => run(commands.checkFirstLaunch());
export const initialize = () => run(commands.initialize());
/**
 * Start a refresh of the list in the backend (a no-op while one runs) and
 * return where it stands at once; `refresh-changed` says when it is over.
 */
export const serversRefresh = () => run(commands.serversRefresh());
/** The refresh as it stands: what a window that has just loaded shows. */
export const refreshState = () => run(commands.refreshState());
/** Add the servers Steam's master lists; turning it on lists them now. */
/** Where the list comes from and how far each source has got. */
export const sourcesStatus = () => run(commands.sourcesStatus());
export const setSteamServerList = (enabled: boolean) => run(commands.setSteamServerList(enabled));

// ── the list, filtered, sorted and paged by the backend ─────────────────
export type {
  MapCount,
  RefreshState,
  ScanProgress,
  ScanState,
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
/**
 * The backend pings every server in its own order; only progress comes back,
 * as `ping-scan-progress` events. Resolves to the new scan's first progress.
 */
export const startScan = () => run(commands.startScan());
/** The whole-list scan as it stands, for a window that has just loaded. */
export const pingScanState = () => run(commands.pingScanState());
export const getServerDetails = (ip: string, queryPort: number) =>
  run(commands.getServerDetails(ip, queryPort));
export const getAppStats = () => run(commands.getAppStats());
export const fetchSteamPlayerCount = () => run(commands.fetchSteamPlayerCount());

export const queryA2s = (ip: string, queryPort: number, gamePort: number | null) =>
  run(commands.queryA2s(ip, queryPort, gamePort));

/** DayZ Metrics' long view of a server (rank, schedules, fake verdict, 24 h); no key needed. */
export const fetchServerMetrics = (ip: string, gamePort: number, queryPort: number, name: string) =>
  run(commands.fetchServerMetrics(ip, gamePort, queryPort, name));
/** Player counts over a span the site keeps, by its id for the server. */
export const fetchMetricsHistory = (id: number, range: MetricsRange) =>
  run(commands.fetchMetricsHistory(id, range));
/** The site's daily rank of a server over the last month. */
export const fetchMetricsRankHistory = (id: number) =>
  run(commands.fetchMetricsRankHistory(id, "1m"));
/** Average players by weekday and UTC hour. */
export const fetchMetricsHeatmap = (id: number) => run(commands.fetchMetricsHeatmap(id));

export type MetricsRange = "1d" | "7d" | "2w" | "1m";

// ── pings of explicit, small lists ──────────────────────────────────────
export const pingServers = (
  targets: string[],
  concurrency: number | null,
  timeoutMs: number | null,
  onProgress: Channel<PingResult[]>,
) => run(commands.pingServers(targets, concurrency, timeoutMs, onProgress));
export const pingSingle = (ip: string, port: number, timeoutMs: number | null) =>
  run(commands.pingSingle(ip, port, timeoutMs));
export const cancelPing = () => run(commands.cancelPing());
export const togglePingPause = () => run(commands.togglePingPause());
