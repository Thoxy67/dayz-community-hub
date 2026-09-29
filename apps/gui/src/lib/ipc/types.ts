/**
 * The shapes the backend sends, as the app names them. Everything here comes
 * from `bindings.ts`, generated from the Rust types by `make bindings`; only
 * the names the interface uses are chosen here.
 */
import type {
  BattleMetricsServer,
  PingResultDto,
  ServerDto as ServerFull,
  ServerSlimDto,
} from "./bindings";

export type {
  A2sDetailsDto,
  A2sPlayerDto,
  A2sRuleDto,
  AppStatsDto,
  ArticleDto,
  CliArgs,
  DownloadEvent,
  DzchConfig,
  DzchMod,
  FavoriteDto,
  FileFilter,
  GeoLocation,
  HistoryDto,
  InitResult,
  InstalledModDto,
  LaunchOptionDto,
  ModDto,
  ModOpType,
  ModProgressEvent,
  ModProgressKind,
  ProfileDto,
  ProfileSettingsInput,
  SteamcmdStatusDto,
  SystemSpecsDto,
  UpdateInfo,
} from "./bindings";

/**
 * A server as the browser's table shows it. `bots` is not in the list: it
 * comes from pings (see the servers store), and old code read it from here.
 */
export type ServerDto = ServerSlimDto & { bots?: number };
/** A server with its mod list, fetched on demand. */
export type ServerFullDto = ServerFull;
export type BattleMetricsDto = BattleMetricsServer;
export type PingResult = PingResultDto;
