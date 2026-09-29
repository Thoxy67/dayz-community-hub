/** Starting DayZ: on a listed server, at an address, and the mod links it needs. */
import { call } from "./core";

export const setupModSymlinks = (ip: string, port: number) => call<void>("setup_mod_symlinks", { ip, port });
export const launchServer = (ip: string, port: number, password: string | null) =>
  call<void>("launch_server", { ip, port, password });
export const launchDirect = (ip: string, gamePort: number, password: string | null, extraArgs: string[] | null) =>
  call<void>("launch_direct", { ip, gamePort, password, extraArgs });
