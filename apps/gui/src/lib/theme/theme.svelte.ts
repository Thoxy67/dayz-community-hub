/**
 * The theme being worn: a preset, one of the player's own themes, or "follow
 * the system" (a dark and a light preset the OS switch toggles between),
 * plus how the frameless window is framed. Written to :root as `--t-*`
 * variables; `styles/tokens.css` derives everything else from those.
 */
import {
  DEFAULT_DARK,
  PRESETS,
  RETIRED,
  TOKEN_NAMES,
  presetById,
  type Scheme,
  type ThemeTokens,
} from "./presets";
import {
  fullTokens,
  isCustom,
  load as parse,
  newId,
  resolvePreset,
  KEY,
  type CustomTheme,
  type Stored,
  type SystemPair,
  type WindowFrame,
} from "./stored";

export type { CustomTheme, SystemPair, WindowFrame } from "./stored";

function load(): Stored {
  try {
    return parse((k) => localStorage.getItem(k));
  } catch {
    return parse(() => null);
  }
}

function watchOs(set: (s: Scheme) => void): Scheme {
  if (typeof window === "undefined" || !window.matchMedia) return "dark";
  const q = window.matchMedia("(prefers-color-scheme: light)");
  q.addEventListener?.("change", (e) => set(e.matches ? "light" : "dark"));
  return q.matches ? "light" : "dark";
}

export type Worn = { id: string; name?: string; scheme: Scheme; tokens: ThemeTokens };

class Theme {
  #s = $state<Stored>(load());
  #os = $state<Scheme>(watchOs((s) => (this.#os = s)));
  /** A theme shown while the pointer rests on it in a menu; never stored. */
  #preview = $state<string | null>(null);

  /** What is selected: a preset id, "custom" for any of the player's own, or null for "follow the system". */
  get selected(): string | null {
    return isCustom(this.#s.preset) ? "custom" : this.#s.preset;
  }
  /** The selection exactly: a preset id, a custom theme's id, or null. */
  get worn(): string | null {
    return this.#s.preset;
  }
  get frame() {
    return this.#s.frame;
  }
  get pair(): SystemPair {
    return this.#s.pair;
  }
  get customs(): readonly CustomTheme[] {
    return this.#s.customs;
  }
  /** The custom theme being worn, or null. */
  get custom(): CustomTheme | null {
    const id = this.#s.preset;
    return isCustom(id) ? (this.#s.customs.find((c) => c.id === id) ?? null) : null;
  }
  get os(): Scheme {
    return this.#os;
  }

  #resolve(id: string | null): Worn {
    if (isCustom(id)) {
      const c = this.#s.customs.find((x) => x.id === id);
      if (c) return c;
    }
    const preset =
      (id && presetById(id)) ||
      presetById(this.#os === "light" ? this.#s.pair.light : this.#s.pair.dark) ||
      presetById(DEFAULT_DARK)!;
    return preset;
  }

  /** The colours actually worn (a hover preview included). */
  current = $derived.by((): Worn => this.#resolve(this.#preview ?? this.#s.preset));

  /** The colours of any selectable id, for a preview card. */
  look(id: string | null): Worn {
    return this.#resolve(id);
  }

  #save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.#s));
    } catch {
      // Still worn for this session.
    }
  }

  use(id: string | null) {
    this.#preview = null;
    if (id === "custom") id = this.#s.customs[0]?.id ?? null;
    if (id && !isCustom(id)) id = resolvePreset(id);
    this.#s.preset = id;
    this.#save();
  }

  /** Wear `id` for a moment without choosing it; null ends the preview. */
  preview(id: string | null) {
    this.#preview = id;
  }

  setPair(scheme: Scheme, id: string) {
    const p = resolvePreset(id, scheme);
    if (!p) return;
    this.#s.pair = { ...this.#s.pair, [scheme]: p };
    this.#save();
  }

  /**
   * A new custom theme copied from `from` (a preset or custom id; what is
   * worn when omitted), worn at once. Returns its id.
   */
  duplicate(from?: string | null, name?: string): string {
    const src = from === undefined ? this.current : this.#resolve(from);
    const preset = isCustom(src.id) ? undefined : src.id;
    const base = isCustom(src.id) ? (src as CustomTheme).base : preset;
    const c: CustomTheme = {
      id: newId(),
      name: name ?? `${src.name ?? presetById(src.id)?.name ?? "Custom"} *`,
      scheme: src.scheme,
      tokens: { ...src.tokens },
      ...(base ? { base } : {}),
    };
    this.#s.customs.push(c);
    this.#s.preset = c.id;
    this.#preview = null;
    this.#save();
    return c.id;
  }

  /** Start editing: the worn custom theme, or a copy of what is worn. */
  beginCustom() {
    if (!this.custom) this.duplicate();
  }

  #edit(): CustomTheme {
    if (!this.custom) this.duplicate();
    return this.custom!;
  }

  setToken(name: keyof ThemeTokens, value: string) {
    this.#edit().tokens[name] = value;
    this.#save();
  }

  /** Replace the worn custom theme's colours (a new one if none is worn). */
  setCustom(custom: { scheme: Scheme; tokens: ThemeTokens; name?: string }) {
    const c = this.#edit();
    c.scheme = custom.scheme;
    c.tokens = { ...custom.tokens };
    if (custom.name) c.name = custom.name;
    this.#save();
  }

  setScheme(scheme: Scheme) {
    this.#edit().scheme = scheme;
    this.#save();
  }

  rename(id: string, name: string) {
    const c = this.#s.customs.find((x) => x.id === id);
    if (!c || !name.trim()) return;
    c.name = name.trim();
    this.#save();
  }

  /** Back to the preset it was started from. */
  reset(id: string) {
    const c = this.#s.customs.find((x) => x.id === id);
    const p = c?.base ? presetById(c.base) : null;
    if (!c || !p) return;
    c.tokens = { ...p.tokens };
    c.scheme = p.scheme;
    this.#save();
  }

  remove(id: string) {
    const i = this.#s.customs.findIndex((x) => x.id === id);
    if (i < 0) return;
    this.#s.customs.splice(i, 1);
    if (this.#s.preset === id) this.#s.preset = null;
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

  /**
   * A theme as a file others can import. The v2 format, `name` added:
   * `{version: 2, app, name, scheme, tokens}`; files without a name import.
   */
  exportJson(id?: string | null): string {
    const c = id === undefined ? this.current : this.#resolve(id);
    const name = c.name ?? presetById(c.id)?.name;
    return JSON.stringify(
      { version: 2, app: "dayz-community-hub", name, scheme: c.scheme, tokens: c.tokens },
      null,
      2,
    );
  }

  /** Add a theme file as a new custom theme and wear it. */
  importJson(text: string, fallbackName = "Imported"): boolean {
    try {
      const d = JSON.parse(text) as {
        name?: unknown;
        scheme?: Scheme;
        tokens?: Partial<ThemeTokens>;
      };
      if (!d || typeof d.tokens !== "object" || !d.tokens) return false;
      const tokens = fullTokens(d.tokens, this.current.tokens);
      const c: CustomTheme = {
        id: newId(),
        name:
          typeof d.name === "string" && d.name.trim() ? d.name.trim().slice(0, 48) : fallbackName,
        scheme: d.scheme === "light" ? "light" : "dark",
        tokens,
      };
      this.#s.customs.push(c);
      this.#s.preset = c.id;
      this.#save();
      return true;
    } catch {
      return false;
    }
  }
}

export const theme = new Theme();
export { PRESETS, RETIRED };
