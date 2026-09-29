/** DayZ Community Offline Mode: its missions, installing it, playing one. */
import { call } from "./core";

export const getOfflineMissions = () => call<string[]>("get_offline_missions");
export const updateOfflineMode = () => call<void>("update_offline_mode");
export const removeOfflineMode = () => call<number>("remove_offline_mode");
export const removeMission = (mission: string) => call<void>("remove_mission", { mission });
export const clearOfflineSaves = () => call<number>("clear_offline_saves");
export const launchOfflineMission = (mission: string) => call<void>("launch_offline_mission", { mission });
export const openMissionDir = (mission: string) => call<void>("open_mission_dir", { mission });
export const openMissionsDir = () => call<void>("open_missions_dir");
