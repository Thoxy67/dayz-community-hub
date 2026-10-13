/** Starting DayZ: on a listed server, at an address, and the mod links it needs. */
import { commands, run } from "./core";

export const setupModSymlinks = (ip: string, port: number) =>
  run(commands.setupModSymlinks(ip, port));
export const launchServer = (ip: string, port: number, password: string | null) =>
  run(commands.launchServer(ip, port, password));
export const launchDirect = (
  ip: string,
  gamePort: number,
  password: string | null,
  extraArgs: string[] | null,
) => run(commands.launchDirect(ip, gamePort, password, extraArgs));
/** How DayZ gets started: the launcher's command, and what Steam adds (Proton, its launch options). */
export const steamLaunchInfo = () => run(commands.steamLaunchInfo());
/** DayZ is running, started from here or not. */
export const gameRunning = () => run(commands.gameRunning());
/** Close DayZ: how many of its processes were stopped. */
export const killGame = () => run(commands.killGame());
/** Where BattlEye stands: its service (Windows) or Proton runtime, and whether it runs. */
export const battleyeStatus = () => run(commands.battleyeStatus());
/**
 * Repair BattlEye (a UAC prompt on Windows, Steam's install on Linux): the
 * state after. `clear` first deletes the game's data outside its folder.
 */
export const repairBattleye = (clear = false) => run(commands.repairBattleye(clear));
/** The game's data outside its folder that a thorough repair deletes. */
export const gameDataDirs = () => run(commands.gameDataDirs());
