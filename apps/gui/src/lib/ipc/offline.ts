/** DayZ Community Offline Mode: its missions, installing it, playing one. */
import { commands, run } from "./core";

export const getOfflineMissions = () => run(commands.getOfflineMissions());
export const updateOfflineMode = () => run(commands.updateOfflineMode());
export const removeOfflineMode = () => run(commands.removeOfflineMode());
export const removeMission = (mission: string) => run(commands.removeMission(mission));
export const clearOfflineSaves = () => run(commands.clearOfflineSaves());
/** Each mission's save: its size and when it was last played. */
export const offlineSaves = () => run(commands.offlineSaves());
export const clearMissionSave = (mission: string) => run(commands.clearMissionSave(mission));
export const launchOfflineMission = (mission: string) =>
  run(commands.launchOfflineMission(mission));
export const openMissionDir = (mission: string) => run(commands.openMissionDir(mission));
export const openMissionsDir = () => run(commands.openMissionsDir());
