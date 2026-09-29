/** Installed Workshop mods, and the SteamCMD operations that fetch them. */
import { commands, run, type Channel } from "./core";
import type { ModOpType, ModProgressEvent } from "./types";

export type { ModOpType };

export const getInstalledMods = () => run(commands.getInstalledMods());
export const checkModUpdates = () => run(commands.checkModUpdates());
export const deleteMod = (modId: number) => run(commands.deleteMod(modId));
export const deleteModsBulk = (modIds: number[]) => run(commands.deleteModsBulk(modIds));
export const toggleModManaged = (modId: number) => run(commands.toggleModManaged(modId));
export const cleanupMods = () => run(commands.cleanupMods());
export const openWorkshopDir = () => run(commands.openWorkshopDir());
export const openModDir = (modId: number) => run(commands.openModDir(modId));

/** What an operation works on; which fields matter depends on its type (see `ModOpType`). */
export type ModOpArgs = {
  ip?: string;
  port?: number;
  modId?: number;
  modName?: string;
  modIds?: number[];
  modNames?: string[];
};

export const startModOperation = (opType: ModOpType, a: ModOpArgs, onProgress: Channel<ModProgressEvent>) =>
  run(
    commands.startModOperation(
      opType,
      a.ip ?? null,
      a.port ?? null,
      a.modId ?? null,
      a.modName ?? null,
      a.modIds ?? null,
      a.modNames ?? null,
      onProgress,
    ),
  );
export const sendSteamcmdInput = (input: string) => run(commands.sendSteamcmdInput(input));
export const cancelModOperation = () => run(commands.cancelModOperation());
