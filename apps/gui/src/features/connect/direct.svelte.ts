/**
 * The Direct Connect form: an address, what the server says about itself when
 * asked, and the extra launch arguments (mods the query did not report, any
 * flag) sent along when joining. Kept outside the view so the form survives
 * leaving the page and coming back.
 */
import { getServerDetails, queryA2s } from "$lib/ipc/servers";
import { errorText } from "$lib/ipc/core";
import { copyText, pickFile, saveFile } from "$lib/ipc/native";
import { writeDzchFile } from "$lib/ipc/system";
import { splitHostPort } from "$lib/address";
import type { A2sDetailsDto, DzchConfig, ModDto, ServerFullDto } from "$lib/ipc/types";
import { words } from "$lib/i18n";
import { connect } from "$lib/stores/connect.svelte";
import { mods } from "$lib/stores/mods.svelte";
import { profile } from "$lib/stores/profile.svelte";
import { servers } from "$lib/stores/servers.svelte";
import { say } from "$lib/stores/say";

/** One argument appended to the launch: a mod (`-mod=@id`) or a flag, verbatim. */
export type LaunchArg = {
  id: string;
  kind: "mod" | "custom";
  value: string;
  label: string;
  /** Reported by the server's query; listed first. */
  fromServer: boolean;
  enabled: boolean;
};

export type ModStatus = "installed" | "stale" | "missing";

/** Which port the player typed, once the query has told us. */
export type PortKind = "game" | "query" | "unknown";

let uid = 0;
const nextId = () => `a${++uid}`;

class DirectForm {
  address = $state("");
  port = $state("2302");
  /** Only when the server answers queries on a port of its own. */
  queryPort = $state("");
  password = $state("");
  /** The password came from a favourite, not the keyboard. */
  passwordFromFavorite = $state(false);

  a2s = $state<A2sDetailsDto | null>(null);
  details = $state<ServerFullDto | null>(null);
  querying = $state(false);
  detailsLoading = $state(false);
  error = $state("");
  resolvedQueryPort = $state<number | null>(null);
  portKind = $state<PortKind>("unknown");
  queriedAt = $state<number | null>(null);

  args = $state<LaunchArg[]>([]);

  // ── what the form resolves to ───────────────────────────────────────────
  ip = $derived(this.address.trim());
  portNum = $derived(parseInt(this.port, 10));
  addressError = $derived(this.ip === "" ? "missing" : null);
  portError = $derived(
    Number.isNaN(this.portNum) || this.portNum < 1 || this.portNum > 65535 ? "invalid" : null,
  );
  valid = $derived(!this.addressError && !this.portError);

  /** The server in the public list at this address, when there is one. */
  listed = $derived(this.valid ? (servers.find(this.ip, this.portNum) ?? null) : null);

  /** The favourite at this address (either port), for its saved password. */
  favorite = $derived.by(() => {
    if (!this.valid) return null;
    const l = this.listed;
    return (
      profile.data?.favorites.find(
        (f) =>
          f.ip === this.ip &&
          (f.port === this.portNum || (!!l && (f.port === l.game_port || f.port === l.query_port))),
      ) ?? null
    );
  });

  gamePort = $derived(this.listed?.game_port ?? this.a2s?.game_port ?? this.portNum);
  queryPortResolved = $derived(
    this.listed?.query_port ?? this.resolvedQueryPort ?? (parseInt(this.queryPort, 10) || null),
  );
  name = $derived(this.a2s?.server_name ?? this.listed?.name ?? "");

  /** The server's mods: the list's (with proper names) first, then any only the query saw. */
  serverMods = $derived.by((): ModDto[] => {
    const list = this.details?.mods.length ? this.details.mods : (this.a2s?.mods ?? []);
    const ids = new Set(list.map((m) => m.steam_workshop_id));
    const more = [...(this.a2s?.mods ?? []), ...(this.a2s?.mods_a2s ?? [])];
    return [...list, ...more.filter((m) => !ids.has(m.steam_workshop_id) && ids.add(m.steam_workshop_id))];
  });

  modStatus(id: number): ModStatus {
    const m = mods.byId.get(id);
    return !m ? "missing" : m.update_available ? "stale" : "installed";
  }

  /** The arguments that will actually be passed, in order. */
  launchArgs = $derived(
    this.args
      .filter((a) => a.enabled)
      .map((a) => (a.kind === "mod" ? `-mod=@${a.value}` : a.value.startsWith("-") ? a.value : `-${a.value}`)),
  );

  constructor() {
    // A favourite's saved password fills an empty field.
    $effect.root(() => {
      $effect(() => {
        const pw = this.favorite?.password;
        if (pw && !this.password) {
          this.password = pw;
          this.passwordFromFavorite = true;
        }
      });
    });
  }

  // ── editing ─────────────────────────────────────────────────────────────
  /** "1.2.3.4:2402" typed or pasted into the address field: split the port off. */
  splitAddress() {
    const { host, port } = splitHostPort(this.address);
    if (port !== null) {
      this.address = host;
      this.port = String(port);
    }
  }

  /** Start from an address (a recent server, a prefill), then ask it. */
  load(ip: string, port: number, queryPort?: number, password?: string) {
    this.address = ip;
    this.port = String(port);
    this.queryPort = queryPort && queryPort !== port ? String(queryPort) : "";
    this.password = password ?? "";
    this.passwordFromFavorite = false;
    this.#reset();
    void this.query();
  }

  #reset() {
    this.detailsLoading = false;
    this.a2s = null;
    this.details = null;
    this.error = "";
    this.resolvedQueryPort = null;
    this.portKind = "unknown";
    this.queriedAt = null;
  }

  clear() {
    this.address = "";
    this.port = "2302";
    this.queryPort = "";
    this.password = "";
    this.passwordFromFavorite = false;
    this.args = this.args.filter((a) => !a.fromServer);
    this.#reset();
  }

  // ── asking the server ───────────────────────────────────────────────────
  #queryId = 0;

  async query() {
    this.splitAddress();
    if (!this.valid) return;
    // Only the latest query writes its answer: typing a new address while
    // one is out must not show the old server's details.
    const id = ++this.#queryId;
    const ip = this.ip;
    const typed = this.portNum;
    this.#reset();
    this.querying = true;
    const listed = this.listed ?? (await servers.resolve(ip, typed)) ?? null;
    if (id !== this.#queryId) return;

    // The list's mod roster is fetched alongside the query, so it still shows
    // when the server does not answer.
    let details: Promise<ServerFullDto | null> = Promise.resolve(null);
    if (listed && listed.mods_count > 0) {
      this.detailsLoading = true;
      details = getServerDetails(listed.ip, listed.query_port).catch(() => null);
    }

    const qp = listed?.query_port ?? (parseInt(this.queryPort, 10) || typed);
    try {
      const a2s = await queryA2s(listed?.ip ?? ip, qp, listed?.game_port ?? null);
      if (id !== this.#queryId) return;
      this.a2s = a2s;
      this.resolvedQueryPort = a2s.query_port;
      this.portKind = typed === a2s.query_port ? "query" : listed || a2s.game_port === typed ? "game" : "unknown";
      const game = listed?.game_port ?? a2s.game_port;
      if (game != null) this.port = String(game);
      if (a2s.query_port !== (game ?? typed)) this.queryPort = String(a2s.query_port);
      if (!listed) {
        this.detailsLoading = true;
        details = getServerDetails(ip, a2s.query_port).catch(() => null);
      }
      void servers.pingOne(listed?.ip ?? ip, a2s.query_port);
      this.queriedAt = Date.now();
    } catch (e) {
      if (id !== this.#queryId) return;
      this.error = errorText(e);
    } finally {
      if (id === this.#queryId) this.querying = false;
    }

    const found = await details;
    if (id !== this.#queryId) return;
    this.details = found;
    this.detailsLoading = false;
    this.#syncServerArgs();
  }

  /**
   * After a query: the server's mods become arguments (enabled, first), ones
   * that disappeared are dropped, and whatever the player added is kept.
   */
  #syncServerArgs() {
    const detected = this.serverMods;
    const ids = new Set(detected.map((m) => String(m.steam_workshop_id)));
    const kept = this.args.filter((a) => a.fromServer && ids.has(a.value));
    const keptIds = new Set(kept.map((a) => a.value));
    const fresh: LaunchArg[] = detected
      .filter((m) => !keptIds.has(String(m.steam_workshop_id)))
      .map((m) => ({
        id: nextId(),
        kind: "mod",
        value: String(m.steam_workshop_id),
        label: m.name || String(m.steam_workshop_id),
        fromServer: true,
        enabled: true,
      }));
    this.args = [...kept, ...fresh, ...this.args.filter((a) => !a.fromServer)];
  }

  // ── launch arguments ────────────────────────────────────────────────────
  addMod(id: number) {
    const m = mods.byId.get(id);
    if (!m || this.args.some((a) => a.kind === "mod" && a.value === String(id))) return;
    this.args = [...this.args, { id: nextId(), kind: "mod", value: String(id), label: m.name, fromServer: false, enabled: true }];
  }

  addCustom(raw: string) {
    const value = raw.trim();
    if (!value) return;
    this.args = [...this.args, { id: nextId(), kind: "custom", value, label: value, fromServer: false, enabled: true }];
  }

  removeArg(id: string) {
    this.args = this.args.filter((a) => a.id !== id);
  }

  toggleArg(id: string) {
    this.args = this.args.map((a) => (a.id === id ? { ...a, enabled: !a.enabled } : a));
  }

  moveArg(id: string, dir: -1 | 1) {
    const i = this.args.findIndex((a) => a.id === id);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= this.args.length) return;
    const next = [...this.args];
    [next[i], next[j]] = [next[j]!, next[i]!];
    this.args = next;
  }

  // ── acting ──────────────────────────────────────────────────────────────
  join() {
    this.splitAddress();
    if (!this.valid) return;
    void connect.direct({
      ip: this.ip,
      port: this.gamePort,
      password: this.password || undefined,
      extraArgs: this.launchArgs.length ? this.launchArgs : undefined,
    });
  }

  async favoriteIt() {
    if (!this.valid) return;
    await profile.addFavorite(this.name || `${this.ip}:${this.gamePort}`, this.ip, this.gamePort, this.password || null);
  }

  #config(): DzchConfig & { mods: NonNullable<DzchConfig["mods"]>; name: string } {
    const qp = this.queryPortResolved;
    return {
      version: 1,
      ip: this.ip,
      port: this.gamePort,
      query_port: qp && qp !== this.gamePort ? qp : null,
      name: this.name,
      password: this.password || null,
      mods: this.serverMods.map((m) => ({ id: m.steam_workshop_id, name: m.name })),
    };
  }

  /** The same server as a `dzch://` link, which opens this app straight on it. */
  link(): string {
    const c = this.#config();
    const params: string[] = [];
    if (c.query_port) params.push(`qport=${c.query_port}`);
    if (c.name) params.push(`name=${encodeURIComponent(c.name)}`);
    if (c.password) params.push(`password=${encodeURIComponent(c.password)}`);
    if (c.mods.length) params.push(`mods=${c.mods.map((m) => m.id).join(",")}`);
    return `dzch://${c.ip}:${c.port}${params.length ? `?${params.join("&")}` : ""}`;
  }

  async copyLink() {
    await copyText(this.link());
    say.ok(words("connect").linkCopied);
  }

  async exportFile() {
    const w = words("connect");
    const c = this.#config();
    const file = c.name ? `${c.name.replace(/[^\w\- ]/g, "_").slice(0, 60)}.dzch` : `${c.ip}_${c.port}.dzch`;
    const path = await saveFile(String(w.exportTitle), file, [{ name: String(w.dzchFilter), extensions: ["dzch"] }]);
    if (!path) return;
    try {
      await writeDzchFile(path, c);
      say.ok(w.exported({ file: path.split(/[\\/]/).pop() ?? path }));
    } catch (e) {
      say.err(w.exportFailed({ error: errorText(e) }));
    }
  }

  async openFile() {
    const w = words("connect");
    const path = await pickFile(String(w.openDzchTitle), { filters: [{ name: String(w.dzchFilter), extensions: ["dzch"] }] });
    if (path) await connect.openDzch(path);
  }
}

export const direct = new DirectForm();
