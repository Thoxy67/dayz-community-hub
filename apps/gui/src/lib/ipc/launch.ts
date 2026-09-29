/** Starting DayZ: on a listed server, at an address, and the mod links it needs. */
import { commands, run } from "./core";

export const setupModSymlinks = (ip: string, port: number) => run(commands.setupModSymlinks(ip, port));
export const launchServer = (ip: string, port: number, password: string | null) =>
  run(commands.launchServer(ip, port, password));
export const launchDirect = (ip: string, gamePort: number, password: string | null, extraArgs: string[] | null) =>
  run(commands.launchDirect(ip, gamePort, password, extraArgs));
