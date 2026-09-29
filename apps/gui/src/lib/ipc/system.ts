/** The machine, Steam, SteamCMD, the command line, .dzch files, the launcher's own updates. */
import { call, type Channel } from "./core";
import type { CliArgs, DzchConfig, SystemSpecsDto } from "./types";

export type SteamcmdStatus = {
  found: boolean;
  path: string | null;
  platform: string;
  [k: string]: unknown;
};

export const getSystemSpecs = () => call<SystemSpecsDto>("get_system_specs");
export const fetchSteamAvatar = () => call<string | null>("fetch_steam_avatar");
export const detectSteamcmd = () => call<SteamcmdStatus>("detect_steamcmd");
export const watchSteamcmd = () => call<SteamcmdStatus>("watch_steamcmd");
export const downloadSteamcmdWindows = () => call<string>("download_steamcmd_windows");

export const getCliArgs = () => call<CliArgs>("get_cli_args");
export const readDzchFile = (path: string) => call<DzchConfig>("read_dzch_file", { path });
export const writeDzchFile = (path: string, config: DzchConfig) =>
  call<void>("write_dzch_file", { path, config });
export const parseDzchUrl = (url: string) => call<DzchConfig>("parse_dzch_url", { url });

export const fetchImage = (url: string) => call<string>("fetch_image", { url });
export const resolveCachedImages = (urls: string[]) =>
  call<Record<string, string>>("resolve_cached_images", { urls });

export type UpdateInfo = { version: string; currentVersion: string; body: string | null; date: string | null };
export type DownloadEvent =
  | { event: "Started"; data: { contentLength: number | null } }
  | { event: "Progress"; data: { chunkLength: number } }
  | { event: "Finished" };

export const checkForUpdate = () => call<UpdateInfo | null>("check_for_update");
export const installUpdate = (onEvent: Channel<DownloadEvent>) => call<void>("install_update", { onEvent });
