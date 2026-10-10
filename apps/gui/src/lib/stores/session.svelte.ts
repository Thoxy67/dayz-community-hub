/**
 * What a reload of the window would otherwise throw away: the view on
 * screen, the server browser's search, filters and order, the selected
 * server, the detail panel, the list's scroll. A reload (F5 in development,
 * a crash recovery, "Retry") keeps the backend and everything it runs; this
 * keeps the window's side of it.
 *
 * Kept in `sessionStorage`, which survives a reload of the page but not a new
 * launch (html.spec.whatwg.org, "The sessionStorage getter": one storage per
 * top-level browsing session), under the backend's own session token as
 * well: data saved by another process is never restored, whatever the
 * webview does with its storage. Choices meant to last across launches
 * belong in `prefs`, not here.
 */
const KEY = "dzch.session";
/** Writes wait this long after the last change (scrolling changes it often). */
const SAVE_MS = 250;

type Stored = { session: string; data: Record<string, unknown> };

class Session {
  /** The window was reloaded inside a backend that was already running. */
  resumed = false;
  #session: string | null = null;
  #data: Record<string, unknown> = {};
  #restorers = new Map<string, (v: unknown) => void>();
  #timer: ReturnType<typeof setTimeout> | undefined;

  constructor() {
    // A reload fires `pagehide` before the page goes: the last change is kept.
    if (typeof addEventListener === "function") addEventListener("pagehide", () => this.#flush());
  }

  /**
   * Startup told who we are (`initialize`): a resumed window gets back what
   * it saved under the same session; a fresh one starts clean.
   */
  begin(session: string, resumed: boolean) {
    this.resumed = resumed;
    this.#session = session;
    let saved: Record<string, unknown> | null = null;
    try {
      const raw = sessionStorage.getItem(KEY);
      const s = raw ? (JSON.parse(raw) as Stored) : null;
      if (resumed && s?.session === session) saved = s.data;
    } catch {
      // Unreadable: nothing to restore.
    }
    if (saved) {
      this.#data = { ...this.#data, ...saved };
      for (const [k, v] of Object.entries(saved)) this.#restorers.get(k)?.(v);
    }
    this.#save();
  }

  /** What was saved under `key` for this session, if the window resumed. */
  get<T>(key: string): T | undefined {
    return this.#session ? (this.#data[key] as T | undefined) : undefined;
  }

  /**
   * Keep `read()` under `key` while the returned cleanup is not called, and
   * hand a saved value to `restore`: now if startup has already told us,
   * else when it does. `read` is tracked: every change is saved.
   */
  keep<T>(key: string, read: () => T, restore: (v: T) => void): () => void {
    this.#restorers.set(key, restore as (v: unknown) => void);
    if (this.#session && key in this.#data) restore(this.#data[key] as T);
    const stop = $effect.root(() => {
      $effect(() => {
        this.#data[key] = $state.snapshot(read());
        this.#save();
      });
    });
    return () => {
      stop();
      this.#restorers.delete(key);
    };
  }

  #save() {
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => this.#flush(), SAVE_MS);
  }

  #flush() {
    if (this.#timer === undefined) return;
    clearTimeout(this.#timer);
    this.#timer = undefined;
    // Before startup there is no session to file it under.
    if (!this.#session) return;
    try {
      sessionStorage.setItem(KEY, JSON.stringify({ session: this.#session, data: this.#data }));
    } catch {
      // Full or disabled: a reload starts from the defaults.
    }
  }
}

export const session = new Session();
