/** What the desktop does for the app: links, files, the clipboard, where the player is. */
import { call } from "./core";

export type FileFilter = { name: string; extensions: string[] };
export type GeoLocation = { lat: number; lon: number; city: string; country: string; country_code: string };

/** Open a web page in the system browser. */
export const openUrl = (url: string) => call<void>("open_url", { url });

/** A file (or folder, with `directory`) chosen by the player, or null if they cancelled. */
export const pickFile = (title: string, opts: { directory?: boolean; filters?: FileFilter[] } = {}) =>
  call<string | null>("pick_file", { title, directory: opts.directory ?? false, filters: opts.filters ?? [] });

/** Where to write a file, or null if they cancelled. */
export const saveFile = (title: string, defaultName: string, filters: FileFilter[] = []) =>
  call<string | null>("save_file", { title, defaultName, filters });

export const copyText = (text: string) => call<void>("copy_text", { text });

/** The player's approximate position from their IP address. */
export const geolocateIp = () => call<GeoLocation>("geolocate_ip");
