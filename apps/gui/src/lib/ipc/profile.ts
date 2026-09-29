/** The player's profile: identity, Steam, favourites, history, launch options. */
import { commands, run } from "./core";
import type { ProfileSettingsInput } from "./types";

export type ProfileSettings = ProfileSettingsInput;

export const getProfile = () => run(commands.getProfile());
export const saveProfileSettings = (settings: ProfileSettings) => run(commands.saveProfileSettings(settings));

export const addFavorite = (name: string, ip: string, port: number, password: string | null) =>
  run(commands.addFavorite(name, ip, port, password));
export const removeFavorite = (ip: string, port: number) => run(commands.removeFavorite(ip, port));
export const removeHistoryEntry = (ip: string, port: number) => run(commands.removeHistoryEntry(ip, port));
export const clearHistory = () => run(commands.clearHistory());
export const addExcludedIp = (ip: string) => run(commands.addExcludedIp(ip));
export const removeExcludedIp = (ip: string) => run(commands.removeExcludedIp(ip));

export const toggleLaunchOption = (key: string) => run(commands.toggleLaunchOption(key));
export const setLaunchOptionValue = (key: string, value: string | null) =>
  run(commands.setLaunchOptionValue(key, value));

export const exportProfile = (path: string, includeMods: boolean) =>
  run(commands.exportProfile(path, includeMods));
/** Replace the profile with a file's; resolves to the profile now in place. */
export const importProfile = (path: string) => run(commands.importProfile(path));
export const resetProfile = () => run(commands.resetProfile());
export const restartApp = () => run(commands.restartApp());
