/** The player's profile: identity, Steam, favourites, history, launch options. */
import { call } from "./core";
import type { ProfileDto } from "./types";

export const getProfile = () => call<ProfileDto>("get_profile");

export type ProfileSettings = {
  player: string | null;
  steamLogin: string | null;
  steamPassword: string | null;
  steamRoot: string | null;
  steamcmdEnabled: boolean;
  steamcmdPath: string | null;
  steamApiKey: string | null;
  steamId: string | null;
  battlemetricsApiKey: string | null;
  userLocation: [number, number] | null;
  pingConcurrency: number;
  pingTimeoutAuto: number;
  pingTimeoutManual: number;
  pingMaxRetries: number;
  pingScanFavorites: boolean;
  pingScanHistory: boolean;
  pingScanServers: boolean;
};
export const saveProfileSettings = (settings: ProfileSettings) =>
  call<void>("save_profile_settings", { settings });

export const addFavorite = (name: string, ip: string, port: number, password: string | null) =>
  call<void>("add_favorite", { name, ip, port, password });
export const removeFavorite = (ip: string, port: number) => call<void>("remove_favorite", { ip, port });
export const removeHistoryEntry = (ip: string, port: number) =>
  call<void>("remove_history_entry", { ip, port });
export const clearHistory = () => call<void>("clear_history");
export const addExcludedIp = (ip: string) => call<void>("add_excluded_ip", { ip });
export const removeExcludedIp = (ip: string) => call<void>("remove_excluded_ip", { ip });

export const toggleLaunchOption = (key: string) => call<boolean>("toggle_launch_option", { key });
export const setLaunchOptionValue = (key: string, value: string | null) =>
  call<void>("set_launch_option_value", { key, value });

export const exportProfile = (path: string, includeMods: boolean) =>
  call<void>("export_profile", { path, includeMods });
export const importProfile = (path: string) => call<void>("import_profile", { path });
export const resetProfile = () => call<void>("reset_profile");
export const restartApp = () => call<void>("restart_app");
