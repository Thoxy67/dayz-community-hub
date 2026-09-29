/**
 * The shapes the backend sends, as the app names them. Everything here comes
 * from `bindings.ts`, generated from the Rust types by `make bindings`; only
 * the names the interface uses are chosen here.
 */
import type {
  BattleMetricsServer,
  PingResultDto,
  ServerDto as ServerFull,
  ServerRow,
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
  ModDownloaderDto,
  ModOpType,
  ModSourceDto,
  ModProgressEvent,
  ModProgressKind,
  ProfileDto,
  ProfileSettingsInput,
  SteamcmdDirsDto,
  SteamcmdStatusDto,
  SteamworksStatusDto,
  SteamSubscriptionDto,
  SteamSubscriptionsDto,
  SystemSpecsDto,
  UpdateInfo,
} from "./bindings";

/** A server as every list shows it, with its live ping and head-count merged in by the backend. */
export type ServerDto = ServerRow;
/** A server with its mod list, fetched on demand. */
export type ServerFullDto = ServerFull;
export type BattleMetricsDto = BattleMetricsServer;
export type { ServerMetrics, RestartSchedule, WipeSchedule, MetricsLink } from "./bindings";
export type PingResult = PingResultDto;
