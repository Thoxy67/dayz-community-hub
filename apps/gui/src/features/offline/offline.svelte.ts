/**
 * DayZ Community Offline Mode: which missions are installed, installing or
 * updating it (the backend downloads in the background and says when it is
 * done through `offline-mode-updated` / `offline-mode-error`), and playing,
 * removing or wiping one.
 */
import * as ipc from "$lib/ipc/offline";
import { events, inTauri, errorText } from "$lib/ipc/core";
import { words } from "$lib/i18n";
import { confirm } from "$lib/stores/dialogs.svelte";
import { say } from "$lib/stores/say";

export type Tone = "neutral" | "ok" | "warn" | "err";

/** Known maps by the name their mission folder carries. */
const MAPS: Record<string, { name: string; terrain: string }> = {
  chernarusplus: { name: "Chernarus+", terrain: "225 km²" },
  enoch: { name: "Livonia", terrain: "163 km²" },
  sakhal: { name: "Sakhal", terrain: "236 km²" },
  namalsk: { name: "Namalsk", terrain: "~100 km²" },
  deerisle: { name: "Deer Isle", terrain: "~150 km²" },
  takistanplus: { name: "Takistan+", terrain: "~150 km²" },
  esseker: { name: "Esseker", terrain: "~130 km²" },
  banov: { name: "Banov", terrain: "~150 km²" },
  pripyat: { name: "Pripyat", terrain: "~100 km²" },
};

export type Mission = {
  /** The folder name, which is what the backend wants back. */
  id: string;
  /** The map's own key (`chernarusplus`), lower case. */
  mapKey: string;
  map: string;
  terrain: string | null;
  /** What kind of mission the folder says it is: offline, coop, pvp… */
  kind: string;
};

/**
 * `DayZCommunityOfflineMode.ChernarusPlus` → Chernarus+, offline. Folders
 * are `<mode>.<map>` or `<map>.<mode>`; whichever half is not the mode is the map.
 */
export function describe(id: string): Mission {
  const parts = id.split(".");
  const isMode = (p: string) => /offline|coop|pvp|surviv|mission/i.test(p);
  const mapPart = parts.find((p) => !isMode(p)) ?? parts[0] ?? id;
  const modePart = parts.find((p) => isMode(p)) ?? "";
  const key = mapPart.toLowerCase();
  const known = MAPS[key];
  const pretty = mapPart
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/[_-]+/g, " ")
    .replace(/\bplus\b/i, "+")
    .replace(/\b\w/g, (c) => c.toUpperCase());
  const m = modePart.toLowerCase();
  const kind = m.includes("coop") ? "coop" : m.includes("pvp") ? "pvp" : m.includes("surv") ? "survival" : m.includes("offline") ? "offline" : "mission";
  return { id, mapKey: key, map: known?.name ?? pretty, terrain: known?.terrain ?? null, kind };
}

class Offline {
  missions = $state<string[]>([]);
  loading = $state(false);
  /** An install or update is running in the background. */
  installing = $state(false);
  loaded = $state(false);
  status = $state("");
  tone = $state<Tone>("neutral");
  #listening = false;

  described = $derived(this.missions.map(describe).sort((a, b) => a.map.localeCompare(b.map)));

  #say(text: unknown, tone: Tone) {
    this.status = String(text);
    this.tone = tone;
  }

  /** Once: hear the background install finish. */
  listen() {
    if (this.#listening || !inTauri) return;
    this.#listening = true;
    void events.offlineModeUpdated.listen(() => {
      this.installing = false;
      this.#say(words("shell").statusOfflineUpdated, "ok");
      say.ok(words("shell").statusOfflineUpdated);
      void this.load(true);
    });
    void events.offlineModeError.listen((e) => {
      this.installing = false;
      this.#say(words("shell").statusUpdateFailed({ error: e.payload }), "err");
    });
  }

  async load(keepStatus = false) {
    const w = words("offline");
    this.loading = true;
    try {
      this.missions = await ipc.getOfflineMissions();
      if (!keepStatus) {
        const count = this.missions.length;
        if (count === 0) this.#say(w.statusNoMissions, "warn");
        else this.#say(count === 1 ? w.statusAvailableOne({ count }) : w.statusAvailable({ count }), "ok");
      }
    } catch (e) {
      this.missions = [];
      this.#say(errorText(e), "err");
    } finally {
      this.loading = false;
      this.loaded = true;
    }
  }

  async update() {
    this.installing = true;
    this.#say(words("offline").statusDownloading, "neutral");
    try {
      await ipc.updateOfflineMode();
      // Outside Tauri there is no event to wait for.
      if (!inTauri) {
        this.installing = false;
        await this.load();
      }
    } catch (e) {
      this.installing = false;
      this.#say(errorText(e), "err");
    }
  }

  async launch(id: string) {
    const w = words("offline");
    say.info(w.statusLaunching({ mission: describe(id).map }));
    try {
      await ipc.launchOfflineMission(id);
    } catch (e) {
      say.err(w.statusLaunchFailed({ error: errorText(e) }));
    }
  }

  openDir = (id: string) => ipc.openMissionDir(id).catch(() => {});
  openMissionsDir = () => ipc.openMissionsDir().catch(() => {});

  async removeMission(id: string) {
    const w = words("offline");
    const ok = await confirm({
      title: String(w.dialogRemoveMissionTitle),
      message: String(w.dialogRemoveMissionMessage({ mission: id })),
      confirmLabel: String(w.dialogRemoveMissionConfirm),
      danger: true,
    });
    if (!ok) return;
    this.loading = true;
    this.#say(w.statusRemovingMission({ mission: id }), "neutral");
    try {
      await ipc.removeMission(id);
      await this.load(true);
      this.#say(w.statusRemovedMission({ mission: id }), "ok");
    } catch (e) {
      this.loading = false;
      this.#say(w.statusRemoveFailed({ error: errorText(e) }), "err");
    }
  }

  async removeAll() {
    const w = words("offline");
    const ok = await confirm({
      title: String(w.dialogRemoveTitle),
      message: String(w.dialogRemoveMessage),
      confirmLabel: String(w.dialogRemoveConfirm),
      danger: true,
    });
    if (!ok) return;
    this.loading = true;
    this.#say(w.statusRemovingAll, "neutral");
    try {
      const n = await ipc.removeOfflineMode();
      await this.load(true);
      this.#say(
        n > 0 ? (n === 1 ? w.statusRemovedFoldersOne({ count: n }) : w.statusRemovedFolders({ count: n })) : w.statusNothingToRemove,
        "ok",
      );
    } catch (e) {
      this.loading = false;
      this.#say(w.statusRemoveFailed({ error: errorText(e) }), "err");
    }
  }

  async clearSaves() {
    const w = words("offline");
    const ok = await confirm({
      title: String(w.dialogClearTitle),
      message: String(w.dialogClearMessage),
      confirmLabel: String(w.dialogClearConfirm),
      danger: true,
    });
    if (!ok) return;
    try {
      const n = await ipc.clearOfflineSaves();
      this.#say(
        n > 0 ? (n === 1 ? w.statusClearedSavesOne({ count: n }) : w.statusClearedSaves({ count: n })) : w.statusNoSaves,
        "ok",
      );
    } catch (e) {
      this.#say(w.statusClearFailed({ error: errorText(e) }), "err");
    }
  }
}

export const offline = new Offline();
