/** The machine, Steam, SteamCMD, the command line, .dzch files, images, the launcher's own updates. */
import { commands, run, type Channel } from "./core";
import type { DzchConfig, SteamcmdStatusDto, UpdateInfo, DownloadEvent } from "./types";

export type { UpdateInfo, DownloadEvent };
export type SteamcmdStatus = SteamcmdStatusDto;

export const getSystemSpecs = () => run(commands.getSystemSpecs());
export const fetchSteamAvatar = () => run(commands.fetchSteamAvatar());
export const detectSteamcmd = () => run(commands.detectSteamcmd());
/** Poll for SteamCMD in the background; its arrival is the `steamcmdDetected` event. */
export const watchSteamcmd = () => run(commands.watchSteamcmd());
export const downloadSteamcmdWindows = () => run(commands.downloadSteamcmdWindows());

export const getCliArgs = () => run(commands.getCliArgs());
export const readDzchFile = (path: string) => run(commands.readDzchFile(path));
export const writeDzchFile = (path: string, config: DzchConfig) => run(commands.writeDzchFile(path, config));
export const parseDzchUrl = (url: string) => run(commands.parseDzchUrl(url));

/** A remote image cached to disk; resolves to its local path. */
export const fetchImage = (url: string) => run(commands.fetchImage(url));
/** Local copies of remote images already cached: [url, local path] pairs. */
export const resolveCachedImages = (urls: string[]) => run(commands.resolveCachedImages(urls));

export const checkForUpdate = (): Promise<UpdateInfo | null> => run(commands.checkForUpdate());
export const installUpdate = (onEvent: Channel<DownloadEvent>) => run(commands.installUpdate(onEvent));
