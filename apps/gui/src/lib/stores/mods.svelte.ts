/**
 * Installed Workshop mods, their update state, and the one SteamCMD operation
 * that may run at a time (install, update, delete), with its live log.
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

export type ModOpPhase = "shutting_down" | "steam_guard_mobile" | "password_required" | "downloading" | "finished";

export type ModOp = {
  active: boolean;
  phase: ModOpPhase;
  current: number;
  total: number;
  currentName: string;
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
  phase: "downloading",
  current: 0,
  total: 0,
  currentName: "",
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
  installed = $state<InstalledModDto[]>([]);
  loading = $state(false);
  checking = $state(false);
  lastChecked = $state(0);
  op = $state<ModOp>(idle());

  stale = $derived(this.installed.filter((m) => m.update_available));
  totalSize = $derived(this.installed.reduce((a, m) => a + m.size, 0));
  byId = $derived(new Map(this.installed.map((m) => [m.id, m])));

  async load() {
    this.loading = true;
    try {
      this.installed = await ipc.getInstalledMods();
    } catch (e) {
      say.err(words("mods").loadFailed({ error: errorText(e) }));
    } finally {
      this.loading = false;
    }
  }

  /** Ask the Workshop which mods are behind. Needs a Steam API key. */
  async checkUpdates(force = false) {
    if (!profile.data?.steam_api_key) return;
    if (!force && Date.now() - this.lastChecked < UPDATES_TTL_MS) return;
    this.checking = true;
    try {
      this.installed = await ipc.checkModUpdates();
      this.lastChecked = Date.now();
    } catch (e) {
      say.err(words("mods").updateCheckFailed({ error: errorText(e) }));
    } finally {
      this.checking = false;
    }
  }

  async refresh() {
    await this.load();
    await this.checkUpdates(true);
  }

  async remove(mod: InstalledModDto) {
    const w = words("mods");
    const ok = await confirm({
      title: String(w.deleteSingleTitle),
      message: String(w.deleteSingleMessage({ name: mod.name, id: String(mod.id), size: mod.size_human })),
      danger: true,
    });
    if (!ok) return;
    try {
      await ipc.deleteMod(mod.id);
      await this.load();
      say.ok(w.deletedSingle({ name: mod.name }));
    } catch (e) {
      say.err(`${String(words("shell").errorFailed)}: ${errorText(e)}`);
    }
  }

  async removeMany(ids: number[]) {
    if (ids.length === 0) return;
    const w = words("mods");
    const size = bytes(this.installed.filter((m) => ids.includes(m.id)).reduce((a, m) => a + m.size, 0));
    const many = ids.length > 1;
    const ok = await confirm({
      title: String(w.deleteSelectedTitle),
      message: String(
        many ? w.deleteSelectedMessagePlural({ count: ids.length, size }) : w.deleteSelectedMessage({ count: ids.length, size }),
      ),
      confirmLabel: String(w.deleteButton({ count: ids.length })),
      danger: true,
    });
    if (!ok) return;
    try {
      await ipc.deleteModsBulk(ids);
      await this.load();
      say.ok(many ? w.deletedSelectedPlural({ count: ids.length }) : w.deletedSelected({ count: ids.length }));
    } catch (e) {
      say.err(`${String(w.deleteFailed)}: ${errorText(e)}`);
    }
  }

  async toggleManaged(mod: InstalledModDto) {
    const w = words("mods");
    try {
      const managed = await ipc.toggleModManaged(mod.id);
      await this.load();
      say.ok(managed ? w.linked({ name: mod.name }) : w.unlinked({ name: mod.name }));
    } catch (e) {
      say.err(w.toggleFailed({ error: errorText(e) }));
    }
  }

  async cleanup() {
    const w = words("mods");
    if (!(await confirm({ title: String(w.cleanupTitle), message: String(w.cleanupMessage) }))) return;
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
  install = (ids: number[]) =>
    ids.length > 0 && this.start("install_manual", { modIds: ids, modNames: ids.map(String) });

  /** Run an operation; `onSuccess` fires when every mod came through. */
  start(opType: ipc.ModOpType, args: Record<string, unknown>, onSuccess?: () => void) {
    const w = words("mods");
    const t0 = Date.now();
    this.op = { ...idle(), active: true, currentName: String(w.preparing), startedAt: t0, itemStartedAt: t0 };
    const ch = new Channel<ModProgressEvent>();
    // The last log entry is a transient "\r" progress line: the next one
    // overwrites it instead of appending, so a download is one live line.
    let lastWasProgress = false;
    ch.onmessage = (ev) => {
      const op = this.op;
      switch (ev.kind) {
        case "shutting_down_steam":
          op.phase = "shutting_down";
          op.currentName = String(w.closingSteam);
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
          op.itemStartedAt = Date.now();
          break;
        case "done":
        case "failed":
          op.current = ev.current;
          op.total = ev.total;
          op.completed.push({ id: ev.mod_id, name: ev.name, ok: ev.kind === "done", ms: Date.now() - op.itemStartedAt });
          op.itemStartedAt = Date.now();
          break;
        case "log_line":
          if (ev.log_line) {
            // `op` is deep state: push is reactive and does not copy the log.
            op.log.push(ev.log_line);
            op.logAt.push(Date.now() - op.startedAt);
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
          if (!ev.hint && ev.failed === 0) {
            say.ok(w.updatedSuccessfully({ count: ev.ok }));
            onSuccess?.();
          } else if (ev.failed > 0) {
            say.warn(w.updateResults({ ok: ev.ok, failed: ev.failed }));
          }
          break;
      }
    };
    ipc.startModOperation(opType, args, ch).catch((e) => {
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
    await ipc.cancelModOperation().catch(() => {});
    await this.dismiss();
  }
}

export const mods = new Mods();
