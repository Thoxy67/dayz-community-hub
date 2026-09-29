/**
 * The theme being worn: a preset, or the player's own twelve colours, plus
 * how the frameless window is framed (corner radius, border when focused and
 * not). Written to :root as `--t-*` variables; `styles/tokens.css` derives
 * everything else from those.
 */
import {
  DEFAULT_DARK,
  DEFAULT_LIGHT,
  PRESETS,
  TOKEN_NAMES,
  presetById,
  type Scheme,
  type ThemeTokens,
} from "./presets";

const KEY = "dzch.theme";

export type WindowFrame = {
  /** Corner radius in px when not maximised (Linux compositors honour it). */
  radius: number;
  /** Border width in px; 0 for none. */
  border: number;
  borderFocus: string;
  borderBlur: string;
};

type Stored = {
  /** A preset id, or "custom". Null follows the OS: chernarus or topo. */
  preset: string | null;
  custom: { scheme: Scheme; tokens: ThemeTokens } | null;
  frame: WindowFrame;
};

const DEFAULT_FRAME: WindowFrame = {
  radius: 0,
  border: 0,
  borderFocus: "oklch(78.8% 0.141 85)",
  borderBlur: "oklch(30% 0.01 130)",
};

function osScheme(): Scheme {
  return typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: light)").matches
    ? "light"
    : "dark";
}

function load(): Stored {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw) {
      const s = JSON.parse(raw) as Partial<Stored>;
      return {
        preset: s.preset ?? null,
        custom: s.custom ?? null,
        frame: { ...DEFAULT_FRAME, ...s.frame },
      };
    }
    // The previous interface kept the preset id here; "dark"/"light" were its
    // DaisyUI defaults, which are now chernarus and topo.
    const legacy = localStorage.getItem("active-preset-id");
    const preset =
      legacy === "dark"
        ? DEFAULT_DARK
        : legacy === "light"
          ? DEFAULT_LIGHT
          : legacy && presetById(legacy)
            ? legacy
            : null;
    const win = localStorage.getItem("window-settings");
    const w = win ? (JSON.parse(win) as Record<string, string>) : {};
    return {
      preset,
      custom: null,
      frame: {
        ...DEFAULT_FRAME,
        radius: parseFloat(w.windowRadius ?? "0") || 0,
        border: parseFloat(w.windowBorderSize ?? "0") || 0,
        borderFocus: w.windowBorderFocus ?? DEFAULT_FRAME.borderFocus,
        borderBlur: w.windowBorderBlur ?? DEFAULT_FRAME.borderBlur,
      },
    };
  } catch {
    return { preset: null, custom: null, frame: { ...DEFAULT_FRAME } };
  }
}

class Theme {
  #s = $state<Stored>(load());

  /** What is selected: a preset id, "custom", or null for "follow the system". */
  get selected() {
    return this.#s.preset;
  }
  get frame() {
    return this.#s.frame;
  }
  get custom() {
    return this.#s.custom;
  }

  /** The colours actually worn. */
  current = $derived.by((): { id: string; scheme: Scheme; tokens: ThemeTokens } => {
    const p = this.#s.preset;
    if (p === "custom" && this.#s.custom) return { id: "custom", ...this.#s.custom };
    const preset =
      (p && presetById(p)) || presetById(osScheme() === "light" ? DEFAULT_LIGHT : DEFAULT_DARK)!;
    return preset;
  });

  #save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.#s));
    } catch {
      // Still worn for this session.
    }
  }

  use(preset: string | null) {
    this.#s.preset = preset;
    this.#save();
  }

  /** Start editing from what is worn now. */
  beginCustom() {
    const c = this.current;
    this.#s.custom = { scheme: c.scheme, tokens: { ...c.tokens } };
    this.#s.preset = "custom";
    this.#save();
  }

  setToken(name: keyof ThemeTokens, value: string) {
    if (!this.#s.custom) this.beginCustom();
    this.#s.custom!.tokens[name] = value;
    this.#s.preset = "custom";
    this.#save();
  }

  setCustom(custom: { scheme: Scheme; tokens: ThemeTokens }) {
    this.#s.custom = custom;
    this.#s.preset = "custom";
    this.#save();
  }

  setFrame(frame: Partial<WindowFrame>) {
    this.#s.frame = { ...this.#s.frame, ...frame };
    this.#save();
  }

  /** Write the worn colours to :root. Call from an effect at the root. */
  apply() {
    const { tokens, scheme } = this.current;
    const root = document.documentElement;
    for (const name of TOKEN_NAMES) root.style.setProperty(`--t-${name}`, tokens[name]);
    root.style.colorScheme = scheme;
    root.dataset.scheme = scheme;
  }

  /** A theme as a file others can import. */
  exportJson(): string {
    const c = this.current;
    return JSON.stringify(
      { version: 2, app: "dayz-community-hub", scheme: c.scheme, tokens: c.tokens },
      null,
      2,
    );
  }

  importJson(text: string): boolean {
    try {
      const d = JSON.parse(text) as { scheme?: Scheme; tokens?: Partial<ThemeTokens> };
      if (!d.tokens) return false;
      const base = this.current.tokens;
      const tokens = Object.fromEntries(
        TOKEN_NAMES.map((n) => [n, d.tokens?.[n] ?? base[n]]),
      ) as ThemeTokens;
      this.setCustom({ scheme: d.scheme === "light" ? "light" : "dark", tokens });
      return true;
    } catch {
      return false;
    }
  }
}

export const theme = new Theme();
export { PRESETS };
