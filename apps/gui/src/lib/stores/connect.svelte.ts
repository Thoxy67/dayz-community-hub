/**
 * Joining a server: find out which of its mods are missing or may be stale,
 * ask the player whether to fetch them first, then launch. Every way into a
 * server (a row, a favourite, an address, the command line, a .dzch file)
 * ends here.
 */
import { getServerDetails } from "$lib/ipc/servers";
import { launchDirect, launchServer, setupModSymlinks } from "$lib/ipc/launch";
import { parseDzchUrl, readDzchFile } from "$lib/ipc/system";
import { copyText } from "$lib/ipc/native";
import type { CliArgs, ModDto } from "$lib/ipc/types";
import type { ServerRow as ServerDto } from "$lib/ipc/servers";
import { words } from "$lib/i18n";
import { DEFAULT_GAME_PORT, splitHostPort } from "$lib/address";
import { dzchLink, type DzchTarget } from "$lib/dzch";
import { app } from "./app.svelte";
import { mods } from "./mods.svelte";
import { profile } from "./profile.svelte";
import { servers } from "./servers.svelte";
import { askPassword, confirm } from "./dialogs.svelte";
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
  prefill = $state<{ ip: string; port: number; queryPort?: number; password?: string } | null>(
    null,
  );

  #split(serverMods: ModDto[]) {
    const missing: ConnectMod[] = [];
    const present: ConnectMod[] = [];
    for (const m of serverMods) {
      const have = mods.byId.get(m.steam_workshop_id);
      if (!have)
        missing.push({
          id: m.steam_workshop_id,
          name: m.name || `Workshop ${m.steam_workshop_id}`,
        });
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

  /** The server's mods; `null` when they could not be read (after asking whether to go on). */
  async #modsOf(ip: string, queryPort: number, count: number, name: string) {
    if (count <= 0) return [];
    try {
      return (await getServerDetails(ip, queryPort)).mods;
    } catch (e) {
      const w = words("connect");
      const go = await confirm({
        title: String(w.modsUnknownTitle),
        message: String(w.modsUnknownMessage({ name, error: errorText(e) })),
        confirmLabel: String(w.modsUnknownGo),
      });
      return go ? [] : null;
    }
  }

  /**
   * The password to join with: the one given, else the favourite's, else
   * asked for. `undefined` for an open server, `null` when the player gives up.
   */
  async #passwordFor(
    name: string,
    ip: string,
    ports: number[],
    known?: string,
  ): Promise<string | undefined | null> {
    if (known) return known;
    const fav = profile.data?.favorites?.find(
      (f) => f.ip === ip && ports.includes(f.port) && f.password,
    );
    if (fav?.password) return fav.password;
    const a = await askPassword(name);
    if (!a) return null;
    if (a.save) void profile.addFavorite(name, ip, ports[0]!, a.password);
    return a.password;
  }

  /** A server from the list. */
  async server(s: ServerDto, known?: string) {
    let password: string | undefined;
    if (s.password) {
      const p = await this.#passwordFor(s.name, s.ip, [s.query_port, s.game_port], known);
      if (p === null) return;
      password = p;
    }
    const serverMods = await this.#modsOf(s.ip, s.query_port, s.mods_count, s.name);
    if (serverMods === null) return;
    const launch = () => void this.#launchListed(s, password);
    const { missing, present } = this.#split(serverMods);
    if (missing.length > 0) {
      this.request = {
        serverName: s.name,
        kind: "missing",
        mods: missing,
        go: (fetch) =>
          fetch ? mods.start("install_server", { ip: s.ip, port: s.query_port }, launch) : launch(),
      };
    } else if (serverMods.length > 0) {
      this.request = {
        serverName: s.name,
        kind: "update",
        mods: present,
        go: (fetch) =>
          fetch ? mods.start("update_server", { ip: s.ip, port: s.query_port }, launch) : launch(),
      };
    } else {
      await this.#launchListed(s, password);
    }
  }

  /** An address from a favourite or the history: the list's entry if there is one. */
  async address(ip: string, port: number, password?: string) {
    const s = await servers.resolve(ip, port);
    if (s) await this.server(s, password);
    else await this.direct({ ip, port, password });
  }

  /** Straight to an address (Direct Connect, the command line). */
  async direct(t: Target) {
    const listed = await servers.resolve(t.ip, t.port);
    const name = listed?.name ?? `${t.ip}:${t.port}`;
    if (listed?.password) {
      const p = await this.#passwordFor(
        name,
        t.ip,
        [listed.query_port, listed.game_port],
        t.password,
      );
      if (p === null) return;
      t = { ...t, password: p };
    }
    const serverMods = listed
      ? await this.#modsOf(t.ip, listed.query_port, listed.mods_count, name)
      : [];
    if (serverMods === null) return;
    if (serverMods.length === 0) return this.#launchAddress(t);
    const { missing, present } = this.#split(serverMods);
    const ids = (list: { id: number; name: string }[]) => ({
      modIds: list.map((m) => m.id),
      modNames: list.map((m) => m.name),
    });
    const launch = () => void this.#launchAddress(t);
    if (missing.length > 0) {
      this.request = {
        serverName: name,
        kind: "missing",
        mods: missing,
        go: (fetch) => (fetch ? mods.start("update_selected", ids(missing), launch) : launch()),
      };
    } else {
      const all = serverMods.map((m) => ({
        id: m.steam_workshop_id,
        name: m.name || String(m.steam_workshop_id),
      }));
      this.request = {
        serverName: name,
        kind: "update",
        mods: present,
        go: (fetch) => (fetch ? mods.start("update_selected", ids(all), launch) : launch()),
      };
    }
  }

  /** The server played last. */
  async rejoin() {
    const last = profile.data?.history?.[0];
    if (last) await this.address(last.ip, last.port);
    else say.warn(words("shell").noHistory);
  }

  async #launchListed(s: ServerDto, password?: string) {
    const w = words("connect");
    say.info(w.connectLaunchingServer({ name: s.name }));
    try {
      await setupModSymlinks(s.ip, s.query_port).catch(() => {});
      await launchServer(s.ip, s.query_port, password ?? null);
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
      const { host, port } = splitHostPort(args.connect);
      app.go("connect");
      return this.direct({ ip: host, port: port ?? DEFAULT_GAME_PORT });
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

  /**
   * A server as a `dzch://` link on the clipboard, to send to a friend. A
   * saved password stays out of it: the link may be pasted anywhere.
   */
  async copyLink(t: Omit<DzchTarget, "password">) {
    try {
      await copyText(dzchLink(t));
      say.ok(words("connect").linkCopied);
    } catch (e) {
      say.err(errorText(e));
    }
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
