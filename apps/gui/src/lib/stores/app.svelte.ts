/**
 * The app as a whole: which view is showing, whether startup finished, the
 * setup wizard, and what happens when the player comes back to the window.
 */
import { checkFirstLaunch, initialize } from "$lib/ipc/servers";
import { errorText } from "$lib/ipc/core";

export type ViewId =
  | "servers"
  | "favorites"
  | "history"
  | "connect"
  | "offline"
  | "mods"
  | "options"
  | "news"
  | "settings"
  | "about";

/** Away this long and the server list is refreshed on return. */
const AWAY_SERVERS_MS = 5 * 60 * 1000;
/** Away this long and mods are checked for updates on return. */
const AWAY_MODS_MS = 10 * 60 * 1000;

class App {
  view = $state<ViewId>("servers");
  /** A view asked to open a sub-part: `settings#ping`, the history's detail. */
  focus = $state<string | null>(null);
  initialized = $state(false);
  initError = $state<string | null>(null);
  showWizard = $state(false);
  maximized = $state(false);
  focused = $state(true);
  #awayAt: number | null = null;

  go(view: ViewId, focus: string | null = null) {
    this.view = view;
    this.focus = focus;
  }

  /**
   * Startup. The wizard shows at once on a first launch while the server
   * list loads behind it; a cached list shows immediately and is refreshed
   * in the background.
   */
  async init() {
    const [{ servers }, { profile }, { mods }, { connect }, { updater }] = await Promise.all([
      import("./servers.svelte"),
      import("./profile.svelte"),
      import("./mods.svelte"),
      import("./connect.svelte"),
      import("./updater.svelte"),
    ]);
    try {
      if (await checkFirstLaunch()) {
        this.showWizard = true;
        this.initialized = true;
      }
      const result = await initialize();
      this.initialized = true;
      await Promise.all([profile.load(), servers.loadStats(), servers.loadSteamPlayers(), mods.load()]);
      void mods.checkUpdates();
      void updater.check();
      void profile.loadAvatar();
      connect.flushCli();
      await servers.load();
      void servers.startScan();
      void servers.prefetchTop();
      if (result.from_cache) void servers.refresh(true);
    } catch (e) {
      this.initError = errorText(e);
    }
  }

  /** The window lost the player's attention. */
  away() {
    this.#awayAt ??= Date.now();
    this.focused = false;
  }

  /** The window is back: refresh what went stale meanwhile. */
  async back() {
    this.focused = true;
    if (this.#awayAt === null) return;
    const gone = Date.now() - this.#awayAt;
    this.#awayAt = null;
    if (gone >= AWAY_SERVERS_MS) (await import("./servers.svelte")).servers.refresh(true);
    if (gone >= AWAY_MODS_MS) (await import("./mods.svelte")).mods.checkUpdates();
  }
}

export const app = new App();
