/**
 * Joining a server: find out which of its mods are missing or may be stale,
 * ask the player whether to fetch them first, then launch. Every way into a
 * server (a row, a favourite, an address, the command line, a .dzch file)
 * ends here.
 */
import { getServerDetails } from "$lib/ipc/servers";
import { launchDirect, launchServer, setupModSymlinks } from "$lib/ipc/launch";
import { parseDzchUrl, readDzchFile } from "$lib/ipc/system";
import type { CliArgs, ModDto, ServerDto } from "$lib/ipc/types";
import { words } from "$lib/i18n";
import { app } from "./app.svelte";
import { mods } from "./mods.svelte";
import { profile } from "./profile.svelte";
import { servers } from "./servers.svelte";
import { say, errorText } from "./say";

export type ConnectMod = {
  id: number;
  name: string;
  local_updated?: number;
  remote_updated?: number | null;
  size_human?: string;
};

export type ConnectRequest = {
  serverName: string;
  /** `missing`: mods must be fetched to join. `update`: they are here, maybe stale. */
  kind: "missing" | "update";
  mods: ConnectMod[];
  /** Go ahead; `fetch` says whether to install or update the mods first. */
  go: (fetch: boolean) => void;
};

export type Target = { ip: string; port: number; password?: string; extraArgs?: string[] };

class Connect {
  request = $state<ConnectRequest | null>(null);
  /** What Direct Connect should open with: filled by "open in Direct Connect". */
  prefill = $state<{ ip: string; port: number; queryPort?: number; password?: string } | null>(null);

  #split(serverMods: ModDto[]) {
    const missing: ConnectMod[] = [];
    const present: ConnectMod[] = [];
    for (const m of serverMods) {
      const have = mods.byId.get(m.steam_workshop_id);
      if (!have) missing.push({ id: m.steam_workshop_id, name: m.name || `Workshop ${m.steam_workshop_id}` });
      else
        present.push({
          id: have.id,
          name: have.name,
          local_updated: have.local_updated,
          remote_updated: have.remote_updated,
          size_human: have.size_human,
        });
    }
    return { missing, present };
  }

  async #modsOf(ip: string, queryPort: number, count: number): Promise<ModDto[]> {
    if (count <= 0) return [];
    try {
      return (await getServerDetails(ip, queryPort)).mods;
    } catch {
      return [];
    }
  }

  /** A server from the list. */
  async server(s: ServerDto) {
    const serverMods = await this.#modsOf(s.ip, s.query_port, s.mods_count);
    const { missing, present } = this.#split(serverMods);
    if (missing.length > 0) {
      this.request = {
        serverName: s.name,
        kind: "missing",
        mods: missing,
        go: (fetch) =>
          fetch
            ? mods.start("install_server", { ip: s.ip, port: s.query_port }, () => void this.#launchListed(s))
            : void this.#launchListed(s),
      };
    } else if (serverMods.length > 0) {
      this.request = {
        serverName: s.name,
        kind: "update",
        mods: present,
        go: (fetch) =>
          fetch
            ? mods.start("update_server", { ip: s.ip, port: s.query_port }, () => void this.#launchListed(s))
            : void this.#launchListed(s),
      };
    } else {
      await this.#launchListed(s);
    }
  }

  /** An address from a favourite or the history: the list's entry if there is one. */
  async address(ip: string, port: number) {
    const s = servers.find(ip, port);
    if (s) await this.server(s);
    else await this.direct({ ip, port });
  }

  /** Straight to an address (Direct Connect, the command line). */
  async direct(t: Target) {
    const listed = servers.find(t.ip, t.port);
    const serverMods = listed ? await this.#modsOf(t.ip, listed.query_port, listed.mods_count) : [];
    if (serverMods.length === 0) return this.#launchAddress(t);
    const { missing, present } = this.#split(serverMods);
    const name = listed?.name ?? `${t.ip}:${t.port}`;
    const ids = (list: { id: number; name: string }[]) => ({
      modIds: list.map((m) => m.id),
      modNames: list.map((m) => m.name),
    });
    if (missing.length > 0) {
      this.request = {
        serverName: name,
        kind: "missing",
        mods: missing,
        go: (fetch) =>
          fetch
            ? mods.start("update_selected", ids(missing), () => void this.#launchAddress(t))
            : void this.#launchAddress(t),
      };
    } else {
      const all = serverMods.map((m) => ({ id: m.steam_workshop_id, name: m.name || String(m.steam_workshop_id) }));
      this.request = {
        serverName: name,
        kind: "update",
        mods: present,
        go: (fetch) =>
          fetch
            ? mods.start("update_selected", ids(all), () => void this.#launchAddress(t))
            : void this.#launchAddress(t),
      };
    }
  }

  /** The server played last. */
  async rejoin() {
    const last = profile.data?.history?.[0];
    if (last) await this.address(last.ip, last.port);
    else say.warn(words("shell").noHistory);
  }

  async #launchListed(s: ServerDto) {
    const w = words("connect");
    say.info(w.connectLaunchingServer({ name: s.name }));
    try {
      await setupModSymlinks(s.ip, s.query_port).catch(() => {});
      await launchServer(s.ip, s.query_port, null);
      say.info(w.connectWaitingSteam);
    } catch (e) {
      say.err(w.connectLaunchFailed({ error: errorText(e) }));
    }
  }

  async #launchAddress(t: Target) {
    const w = words("connect");
    say.info(w.connectLaunchingAddress({ address: `${t.ip}:${t.port}` }));
    try {
      await launchDirect(t.ip, t.port, t.password ?? null, t.extraArgs ?? null);
      say.info(w.connectWaitingSteam);
    } catch (e) {
      say.err(w.connectLaunchFailed({ error: errorText(e) }));
    }
  }

  /** Open a server in Direct Connect, to query it or join with a password. */
  openInDirect(ip: string, port: number, queryPort?: number, password?: string) {
    this.prefill = { ip, port, queryPort, password };
    app.go("connect");
  }

  // ── the command line, deep links and .dzch files ────────────────────────
  #pendingCli: CliArgs | null = null;

  async cli(args: CliArgs) {
    if (!app.initialized) {
      this.#pendingCli = args;
      return;
    }
    if (args.open) return this.openDzch(args.open);
    if (args.connect) {
      const raw = args.connect.trim();
      const i = raw.lastIndexOf(":");
      const port = i === -1 ? NaN : parseInt(raw.slice(i + 1), 10);
      const ip = Number.isNaN(port) ? raw : raw.slice(0, i);
      app.go("connect");
      return this.direct({ ip, port: Number.isNaN(port) ? 2302 : port });
    }
    if (args.reconnect) {
      if (profile.data?.history?.[0]) return this.rejoin();
      say.warn(words("connect").cliNoHistory);
    }
  }

  /** Arguments that arrived before the app was ready. */
  flushCli() {
    const a = this.#pendingCli;
    this.#pendingCli = null;
    if (a) void this.cli(a);
  }

  async openDzch(raw: string) {
    try {
      const c = raw.startsWith("dzch://") ? await parseDzchUrl(raw) : await readDzchFile(raw);
      app.go("connect");
      await this.direct({ ip: c.ip, port: c.port, password: c.password ?? undefined });
    } catch (e) {
      say.err(words("connect").cliDzchOpenFailed({ error: errorText(e) }));
    }
  }
}

export const connect = new Connect();
