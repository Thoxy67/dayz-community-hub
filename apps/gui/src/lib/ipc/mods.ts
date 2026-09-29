/** Installed Workshop mods, and the SteamCMD operations that fetch them. */
import { call, type Channel } from "./core";
import type { InstalledModDto, ModOpType, ModProgressEvent } from "./types";

export const getInstalledMods = () => call<InstalledModDto[]>("get_installed_mods");
export const checkModUpdates = () => call<InstalledModDto[]>("check_mod_updates");
export const deleteMod = (modId: number) => call<void>("delete_mod", { modId });
export const deleteModsBulk = (modIds: number[]) => call<void>("delete_mods_bulk", { modIds });
export const toggleModManaged = (modId: number) => call<boolean>("toggle_mod_managed", { modId });
export const cleanupMods = () => call<string>("cleanup_mods");
export const openWorkshopDir = () => call<void>("open_workshop_dir");
export const openModDir = (modId: number) => call<void>("open_mod_dir", { modId });

export type { ModOpType };

export const startModOperation = (
  opType: ModOpType,
  args: Record<string, unknown>,
  onProgress: Channel<ModProgressEvent>,
) => call<void>("start_mod_operation", { opType, ...args, onProgress });
export const sendSteamcmdInput = (input: string) => call<void>("send_steamcmd_input", { input });
export const cancelModOperation = () => call<void>("cancel_mod_operation");
