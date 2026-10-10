/**
 * The app as a whole: which view is showing, whether startup finished, the
 * setup wizard, and what happens when the player comes back to the window.
 */
import { checkFirstLaunch, initialize } from "$lib/ipc/servers";
import { errorText } from "$lib/ipc/core";
import { session } from "./session.svelte";

export type ViewId =
  | "servers"
  | "favorites"
  | "history"
  | "friends"
  | "connect"
  | "offline"
  | "mods"
  | "options"
  | "news"
  | "stats"
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
   *
   * After a reload the backend is already running (`resumed`): the window
   * reads what it is doing (the scan, a refresh, a mod operation, an update
   * download) and shows it, and fires nothing merely because it loaded. What
   * the window itself held comes back from the session.
   */
  #started = false;

  async init() {
    if (this.#started) return;
    this.#started = true;
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
      session.begin(result.session, result.resumed);
      servers.listError = result.list_error;
      this.initialized = true;
      // The list first: nothing below waits on the network or the disk
      // before it. The profile is what the rest needs (the rejoin card, the
      // command line's "reconnect"), so it is the one thing awaited.
      const list = servers.load().then(() => servers.resume());
      void servers.loadStats();
      void servers.loadSteamPlayers();
      void updater.resume();
      const prof = profile.load();
      // The Workshop check needs the Steam API key from the profile. The
      // backend keeps the last answer, which the disk read shows: a reload
      // does not ask the Workshop again.
      void Promise.all([mods.load(), prof]).then(() => {
        if (!result.resumed) void mods.checkUpdates();
      });
      if (result.resumed) void mods.resume();
      await prof;
      void profile.loadAvatar();
      connect.flushCli();
      await list;
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

// The view on screen comes back after a reload.
session.keep(
  "app.view",
  () => app.view,
  (v) => app.go(v),
);
