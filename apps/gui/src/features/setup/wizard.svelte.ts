/**
 * The first-launch setup: which step, what has been typed in, and whether
 * SteamCMD is there. SteamCMD is looked for when its step opens; when it is
 * missing the backend keeps polling (the player may be installing it in a
 * terminal right now) and says when it appears.
 */
import { events, type SteamcmdStatusDto } from "$lib/ipc/bindings";
import { detectSteamcmd, downloadSteamcmdWindows, watchSteamcmd } from "$lib/ipc/system";
import { errorText } from "$lib/ipc/core";
import { profile } from "$lib/stores/profile.svelte";
import { servers } from "$lib/stores/servers.svelte";
import { app } from "$lib/stores/app.svelte";

export const STEPS = ["welcome", "steamcmd", "account", "services", "appearance", "done"] as const;
export type Step = (typeof STEPS)[number];

class Wizard {
  step = $state<Step>("welcome");
  index = $derived(STEPS.indexOf(this.step));

  player = $state("");
  steamLogin = $state("");
  steamPassword = $state("");
  steamRoot = $state("");
  steamcmdPath = $state("");
  steamApiKey = $state("");
  steamId = $state("");
  battlemetricsKey = $state("");

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
  /** The account step cannot be passed without a Steam login. */
  get canAdvance() {
    return this.step !== "account" || this.steamLogin.trim().length > 0;
  }

  go(step: Step) {
    this.step = step;
    if (step === "steamcmd" && !this.found) void this.detect();
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

  async #watch() {
    if (this.#unlisten) return;
    this.#unlisten = await events.steamcmdDetected.listen((e) => this.#apply(e.payload)).catch(() => null);
    void watchSteamcmd().catch(() => {});
  }

  stopWatching() {
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

  async finish() {
    this.saving = true;
    const blank = (v: string) => v.trim() || null;
    await profile.saveAccount({
      player: blank(this.player),
      steamLogin: blank(this.steamLogin),
      steamPassword: this.steamPassword || null,
      steamRoot: blank(this.steamRoot),
      steamcmdPath: blank(this.steamcmdPath),
      steamApiKey: blank(this.steamApiKey),
      steamId: blank(this.steamId),
      battlemetricsApiKey: blank(this.battlemetricsKey),
      userLocation: null,
    });
    this.saving = false;
    this.close();
  }

  close() {
    this.stopWatching();
    app.showWizard = false;
    void Promise.all([profile.load(), servers.loadStats()]).then(() => profile.loadAvatar());
  }
}

export const wizard = new Wizard();
