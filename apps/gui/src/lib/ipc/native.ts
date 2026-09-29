/** What the desktop does for the app: links, files, the clipboard, where the player is. */
import { commands, run } from "./core";
import type { FileFilter, GeoLocation } from "./types";

export type { FileFilter, GeoLocation };

/** Open a web page in the system browser. */
export const openUrl = (url: string) => run(commands.openUrl(url));

/** A file (or folder, with `directory`) chosen by the player, or null if they cancelled. */
export const pickFile = (
  title: string,
  opts: { directory?: boolean; filters?: FileFilter[] } = {},
) => run(commands.pickFile(title, opts.directory ?? false, opts.filters ?? []));

/** Where to write a file, or null if they cancelled. */
export const saveFile = (title: string, defaultName: string, filters: FileFilter[] = []) =>
  run(commands.saveFile(title, defaultName, filters));

export const copyText = (text: string) => run(commands.copyText(text));

/** The player's approximate position from their IP address. */
export const geolocateIp = () => run(commands.geolocateIp());
