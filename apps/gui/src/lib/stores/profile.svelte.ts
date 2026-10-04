/**
 * The player's profile and what changes it. Edits are applied here first and
 * then written; a failed write reloads the profile from disk, so the screen
 * never shows something that was not saved.
 */
import * as ipc from "$lib/ipc/profile";
import { fetchSteamAvatar } from "$lib/ipc/system";
import { pickFile, saveFile } from "$lib/ipc/native";
import type { FavoriteDto, HistoryDto, ModDownloaderDto, ProfileDto } from "$lib/ipc/types";
import { setModDownloader } from "$lib/ipc/mods";
import { words } from "$lib/i18n";
import { confirm } from "./dialogs.svelte";
import { say, errorText } from "./say";

export type AccountSettings = Pick<
  ipc.ProfileSettings,
  | "player"
  | "steamLogin"
  | "steamRoot"
  | "steamcmdPath"
  | "steamApiKey"
  | "steamId"
  | "userLocation"
>;

export type PingSettings = Pick<
  ipc.ProfileSettings,
  | "pingConcurrency"
  | "pingTimeoutAuto"
  | "pingTimeoutManual"
  | "pingMaxRetries"
  | "pingScanFavorites"
  | "pingScanHistory"
  | "pingScanServers"
>;

const addrKey = (ip: string, port: number) => `${ip}:${port}`;

class Profile {
  data = $state<ProfileDto | null>(null);
  avatarUrl = $state<string | null>(null);

  favoriteKeys = $derived(new Set((this.data?.favorites ?? []).map((f) => addrKey(f.ip, f.port))));
  excludedIps = $derived(new Set(this.data?.excluded_ips ?? []));

  isFavorite(ip: string, port: number) {
    return this.favoriteKeys.has(addrKey(ip, port));
  }

  async load() {
    try {
      this.data = await ipc.getProfile();
    } catch (e) {
      say.err(words("settings").profileLoadFailed({ error: errorText(e) }));
    }
  }

  async loadAvatar() {
    if (!this.data?.steam_api_key || !this.data.steam_id) {
      this.avatarUrl = null;
      return;
    }
    this.avatarUrl = await fetchSteamAvatar().catch(() => null);
  }

  /** Apply locally, write, and on failure reload and say why. */
  async #write(
    apply: () => void,
    write: () => Promise<unknown>,
    done?: unknown,
    failPrefix?: unknown,
  ) {
    apply();
    try {
      await write();
      if (done) say.ok(done);
    } catch (e) {
      await this.load();
      say.err(`${String(failPrefix ?? words("shell").errorFailed)}: ${errorText(e)}`);
    }
  }

  #settings(): ipc.ProfileSettings {
    const p = this.data;
    return {
      player: p?.player ?? null,
      steamLogin: p?.steam_login ?? null,
      steamRoot: p?.steam_root ?? null,
      steamcmdEnabled: p?.steamcmd_enabled ?? true,
      steamcmdPath: p?.steamcmd_path ?? null,
      steamApiKey: p?.steam_api_key ?? null,
      steamId: p?.steam_id ?? null,
      userLocation: p?.user_location ?? null,
      pingConcurrency: p?.ping_concurrency ?? 64,
      pingTimeoutAuto: p?.ping_timeout_auto ?? 2000,
      pingTimeoutManual: p?.ping_timeout_manual ?? 10000,
      pingMaxRetries: p?.ping_max_retries ?? 0,
      pingScanFavorites: p?.ping_scan_favorites ?? true,
      pingScanHistory: p?.ping_scan_history ?? true,
      pingScanServers: p?.ping_scan_servers ?? true,
    };
  }

  // ── account ─────────────────────────────────────────────────────────────
  async saveAccount(a: AccountSettings) {
    const w = words("settings");
    try {
      await ipc.saveProfileSettings({ ...this.#settings(), ...a });
      await this.load();
      void this.loadAvatar();
      const { servers } = await import("./servers.svelte");
      void servers.loadStats();
      say.ok(w.profileSettingsSaved);
    } catch (e) {
      say.err(w.profileSettingsSaveFailed({ error: errorText(e) }));
    }
  }

  /** Ping settings are written as the sliders move, without a toast. */
  async savePing(s: PingSettings) {
    if (!this.data) return;
    try {
      await ipc.saveProfileSettings({ ...this.#settings(), ...s });
      this.data = {
        ...this.data,
        ping_concurrency: s.pingConcurrency,
        ping_timeout_auto: s.pingTimeoutAuto,
        ping_timeout_manual: s.pingTimeoutManual,
        ping_max_retries: s.pingMaxRetries,
        ping_scan_favorites: s.pingScanFavorites,
        ping_scan_history: s.pingScanHistory,
        ping_scan_servers: s.pingScanServers,
      };
    } catch (e) {
      say.err(words("settings").profileSettingsSaveFailed({ error: errorText(e) }));
    }
  }

  /** SteamCMD or the Steam client: takes effect at once, from the next operation. */
  async setModDownloader(downloader: ModDownloaderDto) {
    await this.#write(
      () => {
        if (this.data) this.data.mod_downloader = downloader;
      },
      () => setModDownloader(downloader),
    );
  }

  /** Mods download through the running Steam client rather than SteamCMD. */
  get viaSteam() {
    return this.data?.mod_downloader === "steamworks";
  }

  // ── favourites ──────────────────────────────────────────────────────────
  async addFavorite(name: string, ip: string, port: number, password: string | null = null) {
    await this.#write(
      () => {
        if (!this.data) return;
        const rest = this.data.favorites.filter((f) => !(f.ip === ip && f.port === port));
        const prev = this.data.favorites.find((f) => f.ip === ip && f.port === port);
        this.data.favorites = [
          ...rest,
          { name, ip, port, password: password ?? prev?.password ?? null },
        ];
      },
      () => ipc.addFavorite(name, ip, port, password),
      words("favorites").added({ name }),
    );
  }

  async removeFavorite(ip: string, port: number) {
    await this.#write(
      () => {
        if (this.data)
          this.data.favorites = this.data.favorites.filter(
            (f) => !(f.ip === ip && f.port === port),
          );
      },
      () => ipc.removeFavorite(ip, port),
      words("favorites").removed,
    );
  }

  async toggleFavorite(name: string, ip: string, port: number) {
    if (this.isFavorite(ip, port)) await this.removeFavorite(ip, port);
    else await this.addFavorite(name, ip, port);
  }

  /** Remove, after asking. */
  async confirmRemoveFavorite(fav: FavoriteDto) {
    const w = words("favorites");
    if (
      await confirm({
        title: String(w.removeTitle),
        message: String(w.removeMessage({ name: fav.name })),
        danger: true,
      })
    ) {
      await this.removeFavorite(fav.ip, fav.port);
    }
  }

  // ── history ─────────────────────────────────────────────────────────────
  async removeHistory(h: HistoryDto) {
    const w = words("history");
    if (
      !(await confirm({
        title: String(w.removeTitle),
        message: String(w.removeMessage({ name: h.name })),
        danger: true,
      }))
    )
      return;
    await this.#write(
      () => {
        if (this.data)
          this.data.history = this.data.history.filter(
            (e) => !(e.ip === h.ip && e.port === h.port),
          );
      },
      () => ipc.removeHistoryEntry(h.ip, h.port),
      w.removed,
    );
  }

  async clearHistory() {
    const w = words("history");
    const count = this.data?.history.length ?? 0;
    if (
      !(await confirm({
        title: String(w.clearTitle),
        message: String(w.clearMessage({ count })),
        danger: true,
      }))
    )
      return;
    await this.#write(
      () => {
        if (this.data) this.data.history = [];
      },
      () => ipc.clearHistory(),
      w.cleared,
    );
  }

  // ── excluded IPs ────────────────────────────────────────────────────────
  async excludeIp(ip: string) {
    const w = words("favorites");
    await this.#write(
      () => {
        if (this.data && !this.data.excluded_ips.includes(ip))
          this.data.excluded_ips = [...this.data.excluded_ips, ip];
      },
      () => ipc.addExcludedIp(ip),
      w.ipExcluded({ ip }),
      w.ipExcludeFailed,
    );
  }

  async unexcludeIp(ip: string) {
    await this.#write(
      () => {
        if (this.data) this.data.excluded_ips = this.data.excluded_ips.filter((e) => e !== ip);
      },
      () => ipc.removeExcludedIp(ip),
      words("favorites").ipUnexcluded({ ip }),
    );
  }

  // ── launch options ──────────────────────────────────────────────────────
  async toggleOption(key: string) {
    const w = words("settings");
    const prev = this.data?.options.find((o) => o.key === key);
    if (this.data)
      this.data.options = this.data.options.map((o) =>
        o.key === key ? { ...o, enabled: !o.enabled } : o,
      );
    try {
      await ipc.toggleLaunchOption(key);
    } catch (e) {
      if (this.data && prev)
        this.data.options = this.data.options.map((o) => (o.key === key ? prev : o));
      say.err(w.profileOptionToggleFailed({ error: errorText(e) }));
    }
  }

  async setOptionValue(key: string, value: string | null) {
    const w = words("settings");
    const prev = this.data?.options.find((o) => o.key === key);
    if (this.data) {
      this.data.options = this.data.options.map((o) =>
        o.key === key ? { ...o, value, enabled: value !== null ? true : o.enabled } : o,
      );
    }
    try {
      await ipc.setLaunchOptionValue(key, value);
    } catch (e) {
      if (this.data && prev)
        this.data.options = this.data.options.map((o) => (o.key === key ? prev : o));
      say.err(w.profileOptionSetFailed({ error: errorText(e) }));
    }
  }

  // ── the profile file ────────────────────────────────────────────────────
  async exportTo(includeMods: boolean) {
    const w = words("settings");
    const filters = [{ name: String(w.profileFilterName), extensions: ["dchub"] }];
    const path = await saveFile(
      String(w.profileExportTitle),
      "dayz-community-hub-profile.dchub",
      filters,
    );
    if (!path) return;
    try {
      await ipc.exportProfile(path, includeMods);
      say.ok(w.profileExported);
    } catch (e) {
      say.err(w.profileExportFailed({ error: errorText(e) }));
    }
  }

  async importFrom() {
    const w = words("settings");
    const filters = [{ name: String(w.profileFilterName), extensions: ["dchub"] }];
    const path = await pickFile(String(w.profileImportTitle), { filters });
    if (!path) return;
    if (
      !(await confirm({
        title: String(w.profileImportConfirmTitle),
        message: String(w.profileImportConfirmMessage),
      }))
    )
      return;
    try {
      await ipc.importProfile(path);
      await ipc.restartApp();
    } catch (e) {
      say.err(`${String(w.profileImportFailed)}: ${errorText(e)}`);
    }
  }

  async reset() {
    const w = words("settings");
    if (
      !(await confirm({
        title: String(w.profileResetTitle),
        message: String(w.profileResetMessage),
        danger: true,
      }))
    )
      return;
    try {
      await ipc.resetProfile();
      await ipc.restartApp();
    } catch (e) {
      say.err(`${String(w.profileResetFailed)}: ${errorText(e)}`);
    }
  }
}

export const profile = new Profile();
