/**
 * Installed Workshop mods, their update state, and the one download
 * operation that may run at a time (install, update, delete), with its live
 * log. SteamCMD or the running Steam client downloads, as the profile says:
 * both report the same steps.
 */
import * as ipc from "$lib/ipc/mods";
import { Channel } from "$lib/ipc/core";
import type { InstalledModDto, ModProgressEvent } from "$lib/ipc/types";
import { words } from "$lib/i18n";
import { bytes } from "$lib/format";
import { confirm } from "./dialogs.svelte";
import { profile } from "./profile.svelte";
import { say, errorText } from "./say";

const UPDATES_TTL_MS = 5 * 60 * 1000;
/**
 * Lines of SteamCMD output kept for the dialog. A long batch prints tens of
 * thousands; past this the oldest go, so the window stays responsive.
 */
export const LOG_MAX = 5000;

export type ModOpPhase = "steam_guard_mobile" | "password_required" | "downloading" | "finished";

export type ModOp = {
  active: boolean;
  /** What was asked for: a login alone reports differently from downloads. */
  kind: ipc.ModOpType;
  /** What downloads: SteamCMD, or the Steam client (Steamworks). */
  via: "steamcmd" | "steamworks";
  phase: ModOpPhase;
  current: number;
  total: number;
  currentName: string;
  /** The mod being worked on (0 before the first starts). */
  currentId: number;
  /** Every mod this operation works on, as far as known when it started. */
  ids: number[];
  /** Mods that came through or failed, with how long each took. */
  completed: { id: number; name: string; ok: boolean; ms: number }[];
  ok: number;
  failed: number;
  hint: string | null;
  /** Raw SteamCMD output. The live download line is rewritten in place. */
  log: string[];
  /** When each log line arrived, ms since `startedAt` (parallel to `log`). */
  logAt: number[];
  startedAt: number;
  /** When the mod being worked on started. */
  itemStartedAt: number;
  /** The dialog is closed while the operation carries on; the status bar shows it. */
  minimised: boolean;
};

const idle = (): ModOp => ({
  active: false,
  kind: "update_all",
  via: "steamcmd",
  phase: "downloading",
  current: 0,
  total: 0,
  currentName: "",
  currentId: 0,
  ids: [],
  completed: [],
  ok: 0,
  failed: 0,
  hint: null,
  log: [],
  logAt: [],
  startedAt: 0,
  itemStartedAt: 0,
  minimised: false,
});

class Mods {
  // Raw: always replaced whole from the backend, never edited in place.
  installed = $state.raw<InstalledModDto[]>([]);
  loading = $state(false);
  /** The disk has been read at least once (an empty list is then an answer). */
  loaded = $state(false);
  checking = $state(false);
  lastChecked = $state(0);
  op = $state<ModOp>(idle());

  stale = $derived(this.installed.filter((m) => m.update_available));
  totalSize = $derived(this.installed.reduce((a, m) => a + m.size, 0));
  byId = $derived(new Map(this.installed.map((m) => [m.id, m])));

  #loading: Promise<void> | null = null;
  #checking: Promise<void> | null = null;

  /** Read what is on disk. Callers at the same moment share one read. */
  load(): Promise<void> {
    return (this.#loading ??= (async () => {
      this.loading = true;
      try {
        this.installed = await ipc.getInstalledMods();
      } catch (e) {
        say.err(words("mods").loadFailed({ error: errorText(e) }));
      } finally {
        this.loading = false;
        this.loaded = true;
        this.#loading = null;
      }
    })());
  }

  /** Ask the Workshop which mods are behind. Needs a Steam API key. */
  checkUpdates(force = false): Promise<void> {
    if (!profile.data?.steam_api_key) return Promise.resolve();
    if (this.#checking) return this.#checking;
    if (!force && Date.now() - this.lastChecked < UPDATES_TTL_MS) return Promise.resolve();
    return (this.#checking = (async () => {
      this.checking = true;
      try {
        this.installed = await ipc.checkModUpdates();
        this.lastChecked = Date.now();
      } catch (e) {
        say.err(words("mods").updateCheckFailed({ error: errorText(e) }));
      } finally {
        this.checking = false;
        this.#checking = null;
      }
    })());
  }

  async refresh() {
    await this.load();
    await this.checkUpdates(true);
  }

  async remove(mod: InstalledModDto) {
    const w = words("mods");
    const ok = await confirm({
      title: String(w.deleteSingleTitle),
      message: String(
        w.deleteSingleMessage({ name: mod.name, id: String(mod.id), size: mod.size_human }),
      ),
      danger: true,
    });
    if (!ok) return;
    try {
      const keptInSteam = await ipc.deleteMod(mod.id);
      await this.load();
      // The launcher never deletes in a Steam library: only the link went.
      if (keptInSteam) say.info(w.keptInSteam({ name: mod.name }));
      else say.ok(w.deletedSingle({ name: mod.name }));
    } catch (e) {
      say.err(`${String(words("shell").errorFailed)}: ${errorText(e)}`);
    }
  }

  async removeMany(ids: number[]) {
    if (ids.length === 0) return;
    const w = words("mods");
    const size = bytes(
      this.installed.filter((m) => ids.includes(m.id)).reduce((a, m) => a + m.size, 0),
    );
    const many = ids.length > 1;
    const ok = await confirm({
      title: String(w.deleteSelectedTitle),
      message: String(
        many
          ? w.deleteSelectedMessagePlural({ count: ids.length, size })
          : w.deleteSelectedMessage({ count: ids.length, size }),
      ),
      confirmLabel: String(w.deleteButton({ count: ids.length })),
      danger: true,
    });
    if (!ok) return;
    try {
      const kept = await ipc.deleteModsBulk(ids);
      await this.load();
      say.ok(
        many
          ? w.deletedSelectedPlural({ count: ids.length })
          : w.deletedSelected({ count: ids.length }),
      );
      if (kept.length > 0) say.info(w.keptInSteamMany({ count: kept.length }));
    } catch (e) {
      say.err(`${String(w.deleteFailed)}: ${errorText(e)}`);
    }
  }

  async toggleManaged(mod: InstalledModDto) {
    const w = words("mods");
    try {
      const managed = await ipc.toggleModManaged(mod.id);
      // Only the link changed: no need to rescan the disk.
      this.installed = this.installed.map((m) => (m.id === mod.id ? { ...m, managed } : m));
      say.ok(managed ? w.linked({ name: mod.name }) : w.unlinked({ name: mod.name }));
    } catch (e) {
      say.err(w.toggleFailed({ error: errorText(e) }));
    }
  }

  async cleanup() {
    const w = words("mods");
    if (!(await confirm({ title: String(w.cleanupTitle), message: String(w.cleanupMessage) })))
      return;
    try {
      const result = await ipc.cleanupMods();
      await this.load();
      say.ok(result);
    } catch (e) {
      say.err(`${String(w.cleanupFailed)}: ${errorText(e)}`);
    }
  }

  openWorkshopDir = () => ipc.openWorkshopDir().catch(() => {});
  openModDir = (id: number) => ipc.openModDir(id).catch(() => {});

  // ── SteamCMD operations ─────────────────────────────────────────────────
  update = (mod: InstalledModDto) => this.start("update_one", { modId: mod.id, modName: mod.name });
  updateAll = () => this.start("update_all", {});
  updateStale = () => this.start("update_stale", {});
  updateMany = (ids: number[]) => ids.length > 0 && this.start("update_selected", { modIds: ids });
  /** Download again and check every file: slow, for a mod that does not load. */
  repair = (mod: InstalledModDto) =>
    this.start("repair", { modIds: [mod.id], modNames: [mod.name] });
  /** Log SteamCMD in once, so downloads use its cached login. */
  login = () => this.start("login", {});
  install = (ids: number[]) =>
    ids.length > 0 && this.start("install_manual", { modIds: ids, modNames: ids.map(String) });

  /**
   * Where a mod stands in the running operation: being downloaded now,
   * waiting its turn, or neither (no operation, or already through).
   */
  opState(id: number): "downloading" | "queued" | null {
    const op = this.op;
    if (!this.busy || !op.ids.includes(id)) return null;
    if (op.currentId === id && !op.completed.some((c) => c.id === id)) return "downloading";
    return op.completed.some((c) => c.id === id) ? null : "queued";
  }

  /** Run an operation; `onSuccess` fires when every mod came through. */
  #opId = 0;

  /** True while SteamCMD is working (not merely showing a finished summary). */
  get busy() {
    return this.op.active && this.op.phase !== "finished";
  }

  start(opType: ipc.ModOpType, args: ipc.ModOpArgs, onSuccess?: () => void) {
    const w = words("mods");
    // One SteamCMD at a time: the backend holds a single PTY. The running
    // one is brought back to the front instead.
    if (this.busy) {
      this.op.minimised = false;
      return;
    }
    const id = ++this.#opId;
    const t0 = Date.now();
    this.op = {
      ...idle(),
      active: true,
      kind: opType,
      // A login is always SteamCMD's.
      via: profile.viaSteam && opType !== "login" ? "steamworks" : "steamcmd",
      currentName: String(w.preparing),
      ids:
        args.modIds ??
        (args.modId !== undefined
          ? [args.modId]
          : opType === "update_stale"
            ? this.stale.map((m) => m.id)
            : opType === "update_all"
              ? this.installed.map((m) => m.id)
              : []),
      startedAt: t0,
      itemStartedAt: t0,
    };
    const ch = new Channel<ModProgressEvent>();
    // The last log entry is a transient "\r" progress line: the next one
    // overwrites it instead of appending, so a download is one live line.
    let lastWasProgress = false;
    const trim = (op: ModOp) => {
      if (op.log.length > LOG_MAX) {
        op.log.splice(0, op.log.length - LOG_MAX);
        op.logAt.splice(0, op.logAt.length - LOG_MAX);
      }
    };
    ch.onmessage = (ev) => {
      // A message from an operation that was cancelled and replaced.
      if (id !== this.#opId) return;
      const op = this.op;
      switch (ev.kind) {
        case "logged_in":
          // Its login is cached now, and a saved password forgotten.
          void profile.load();
          break;
        case "steam_guard_mobile_required":
          op.phase = "steam_guard_mobile";
          break;
        case "password_required":
          op.phase = "password_required";
          break;
        case "starting":
          op.phase = "downloading";
          op.current = ev.current;
          op.total = ev.total;
          op.currentName = ev.name;
          op.currentId = ev.mod_id;
          op.itemStartedAt = Date.now();
          break;
        case "done":
        case "failed":
          op.current = ev.current;
          op.total = ev.total;
          op.completed.push({
            id: ev.mod_id,
            name: ev.name,
            ok: ev.kind === "done",
            ms: Date.now() - op.itemStartedAt,
          });
          op.itemStartedAt = Date.now();
          break;
        case "log_line":
          if (ev.log_line) {
            // `op` is deep state: push is reactive and does not copy the log.
            op.log.push(ev.log_line);
            op.logAt.push(Date.now() - op.startedAt);
            trim(op);
            lastWasProgress = false;
          }
          break;
        case "log_progress":
          if (ev.log_line) {
            const at = Date.now() - op.startedAt;
            if (lastWasProgress && op.log.length > 0) {
              op.log[op.log.length - 1] = ev.log_line;
              op.logAt[op.logAt.length - 1] = at;
            } else {
              op.log.push(ev.log_line);
              op.logAt.push(at);
            }
            lastWasProgress = true;
          }
          break;
        case "finished":
          op.phase = "finished";
          op.ok = ev.ok;
          op.failed = ev.failed;
          op.hint = ev.hint;
          if (op.kind === "login") {
            if (ev.ok > 0) say.ok(w.loginOk);
            else say.err(w.loginFailed);
          } else if (!ev.hint && ev.failed === 0) {
            say.ok(w.updatedSuccessfully({ count: ev.ok }));
            onSuccess?.();
          } else if (ev.failed > 0) {
            say.warn(w.updateResults({ ok: ev.ok, failed: ev.failed }));
          }
          break;
      }
    };
    ipc.startModOperation(opType, args, ch).catch((e) => {
      if (id !== this.#opId) return;
      say.err(w.operationFailed({ error: errorText(e) }));
      this.op.active = false;
    });
  }

  /** Close the progress window and re-read what is on disk. */
  async dismiss() {
    this.op.active = false;
    if (profile.data?.steam_api_key) await this.checkUpdates(true);
    else await this.load();
    const { servers } = await import("./servers.svelte");
    void servers.loadStats();
  }

  async sendInput(input: string) {
    try {
      await ipc.sendSteamcmdInput(input);
    } catch (e) {
      say.err(words("mods").passwordSendFailed({ error: errorText(e) }));
    }
  }

  async cancel() {
    this.#opId++;
    await ipc.cancelModOperation().catch(() => {});
    await this.dismiss();
  }
}

export const mods = new Mods();
