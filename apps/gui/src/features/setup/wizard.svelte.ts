/**
 * The first-launch setup: which step, what has been typed in, where DayZ
 * is, and how mods will download. DayZ is looked for when its step opens.
 * With SteamCMD chosen, SteamCMD is looked for too; when it is missing the
 * backend keeps polling (the player may be installing it in a terminal right
 * now) and says when it appears. With the Steam client chosen, SteamCMD and
 * the Steam login are not needed: whether Steam runs is checked instead.
 */
import {
  events,
  type DayzDetectDto,
  type ModDownloaderDto,
  type SteamcmdStatusDto,
  type SteamworksStatusDto,
} from "$lib/ipc/bindings";
import {
  detectDayz,
  detectSteamcmd,
  downloadSteamcmdWindows,
  watchSteamcmd,
} from "$lib/ipc/system";
import { steamworksStatus } from "$lib/ipc/mods";
import { errorText } from "$lib/ipc/core";
import { profile } from "$lib/stores/profile.svelte";
import { servers } from "$lib/stores/servers.svelte";
import { app, type ViewId } from "$lib/stores/app.svelte";

export const STEPS = [
  "welcome",
  "game",
  "downloads",
  "account",
  "services",
  "appearance",
  "done",
] as const;
export type Step = (typeof STEPS)[number];

class Wizard {
  step = $state<Step>("welcome");
  index = $derived(STEPS.indexOf(this.step));

  player = $state("");
  steamLogin = $state("");
  steamRoot = $state("");
  steamcmdPath = $state("");
  steamApiKey = $state("");
  steamId = $state("");
  downloader = $state<ModDownloaderDto>("steamcmd");

  game = $state<DayzDetectDto | null>(null);
  findingGame = $state(false);
  steam = $state<SteamworksStatusDto | null>(null);
  checkingSteam = $state(false);

  status = $state<SteamcmdStatusDto | null>(null);
  detecting = $state(false);
  downloading = $state(false);
  downloadError = $state("");
  saving = $state(false);

  #unlisten: (() => void) | null = null;

  get platform() {
    return this.status?.platform ?? (navigator.userAgent.includes("Windows") ? "windows" : "linux");
  }
  get found() {
    return this.status?.found === true;
  }
  get viaSteam() {
    return this.downloader === "steamworks";
  }
  /** SteamCMD needs a Steam login to download mods; the Steam client does not. */
  get needsLogin() {
    return !this.viaSteam;
  }
  /** The account step cannot be passed without a Steam login, when one is needed. */
  get canAdvance() {
    return this.step !== "account" || !this.needsLogin || this.steamLogin.trim().length > 0;
  }

  go(step: Step) {
    this.step = step;
    if (step === "game" && !this.game) void this.findGame();
    if (step === "downloads") this.#prepareDownloads();
  }

  #prepareDownloads() {
    if (this.viaSteam) {
      if (!this.steam) void this.checkSteam();
    } else if (!this.found) void this.detect();
  }

  choose(d: ModDownloaderDto) {
    this.downloader = d;
    if (d === "steamworks") this.stopWatching();
    this.#prepareDownloads();
  }

  /**
   * Where DayZ is: in the folder typed or picked (`steamRoot`), or wherever
   * Steam is installed. A found folder only shows as the field's placeholder,
   * so `steamRoot` holds nothing but what the player gave.
   */
  async findGame() {
    this.findingGame = true;
    try {
      this.game = await detectDayz(this.steamRoot.trim() || null);
    } catch {
      this.game = { steamapps: null, dayz_dir: null, workshop_mods: 0 };
    } finally {
      this.findingGame = false;
    }
  }

  /** Whether Valve's library loads and the Steam client runs. */
  async checkSteam() {
    this.checkingSteam = true;
    try {
      this.steam = await steamworksStatus();
    } catch (e) {
      this.steam = { library: false, error: errorText(e), steam_running: false };
    } finally {
      this.checkingSteam = false;
    }
  }
  next() {
    if (this.canAdvance && this.index < STEPS.length - 1) this.go(STEPS[this.index + 1]!);
  }
  back() {
    if (this.index > 0) this.go(STEPS[this.index - 1]!);
  }

  async detect() {
    this.detecting = true;
    this.downloadError = "";
    try {
      this.#apply(await detectSteamcmd());
    } catch {
      this.status = { found: false, path: null, platform: this.platform };
    } finally {
      this.detecting = false;
    }
    if (!this.found) void this.#watch();
  }

  #apply(s: SteamcmdStatusDto) {
    this.status = s;
    if (s.found && s.path) {
      this.steamcmdPath = s.path;
      this.stopWatching();
    }
  }

  #watching = false;

  async #watch() {
    // Set before the await: two detections in a row must not listen twice.
    if (this.#watching) return;
    this.#watching = true;
    const off = await events.steamcmdDetected
      .listen((e) => this.#apply(e.payload))
      .catch(() => null);
    if (!this.#watching) {
      off?.();
      return;
    }
    this.#unlisten = off;
    void watchSteamcmd().catch(() => {});
  }

  stopWatching() {
    this.#watching = false;
    this.#unlisten?.();
    this.#unlisten = null;
  }

  /** Windows: fetch SteamCMD from Valve into the app's folder. */
  async download() {
    this.downloading = true;
    this.downloadError = "";
    try {
      const path = await downloadSteamcmdWindows();
      this.#apply({ found: true, path, platform: "windows" });
    } catch (e) {
      this.downloadError = errorText(e);
    } finally {
      this.downloading = false;
    }
  }

  /** A path chosen by hand counts as found. */
  chose(path: string) {
    this.#apply({ found: true, path, platform: this.platform });
  }

  /** Save, close, and open `view` (the server list unless told otherwise). */
  async finish(view?: ViewId) {
    this.saving = true;
    const blank = (v: string) => v.trim() || null;
    await profile.saveAccount({
      player: blank(this.player),
      steamLogin: this.needsLogin ? blank(this.steamLogin) : null,
      // A folder found on its own is found again at every start; only a
      // picked one is worth pinning.
      steamRoot: blank(this.steamRoot),
      steamcmdPath: this.viaSteam ? null : blank(this.steamcmdPath),
      steamApiKey: blank(this.steamApiKey),
      steamId: blank(this.steamId),
      userLocation: null,
    });
    await profile.setModDownloader(this.downloader);
    this.saving = false;
    this.close();
    if (view) app.go(view);
  }

  close() {
    this.stopWatching();
    app.showWizard = false;
    void Promise.all([profile.load(), servers.loadStats()]).then(() => profile.loadAvatar());
  }
}

export const wizard = new Wizard();
