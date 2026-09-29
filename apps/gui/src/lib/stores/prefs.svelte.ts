/**
 * The interface's own preferences: how the window is laid out, not what the
 * launcher does. Kept in one `localStorage` key and written as they change;
 * the launcher's settings (Steam, ping, launch options) live in the profile,
 * on the Rust side.
 */
const KEY = "dzch.prefs";

type Stored = {
  /** Sizes of the resizable panes, by id, in pixels. */
  panes: Record<string, number>;
  /** The navigation rail shows icons only. */
  railCollapsed: boolean;
  /** The last-played strip was put away for this server ("ip:port"). */
  dismissedRejoin: string | null;
};

const DEFAULTS: Stored = { panes: {}, railCollapsed: false, dismissedRejoin: null };

function load(): Stored {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) return { ...DEFAULTS, ...(JSON.parse(raw) as Partial<Stored>) };
  } catch {
    // A corrupt or disabled store starts from the defaults.
  }
  return { ...DEFAULTS };
}

class Prefs {
  #s = $state<Stored>(load());

  #save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.#s));
    } catch {
      // Still applied for this session.
    }
  }

  pane(id: string, fallback: number): number {
    return this.#s.panes[id] ?? fallback;
  }
  setPane(id: string, px: number) {
    this.#s.panes[id] = Math.round(px);
    this.#save();
  }
  resetPane(id: string) {
    delete this.#s.panes[id];
    this.#save();
  }

  get railCollapsed() {
    return this.#s.railCollapsed;
  }
  set railCollapsed(v: boolean) {
    this.#s.railCollapsed = v;
    this.#save();
  }

  get dismissedRejoin() {
    return this.#s.dismissedRejoin;
  }
  set dismissedRejoin(v: string | null) {
    this.#s.dismissedRejoin = v;
    this.#save();
  }
}

export const prefs = new Prefs();
