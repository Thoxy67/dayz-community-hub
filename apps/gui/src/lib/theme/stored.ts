/**
 * The theme choice as stored in localStorage, and how every earlier shape of
 * it is read. Pure, so `theme.test.ts` can check the migrations.
 */
import {
  DEFAULT_DARK,
  DEFAULT_LIGHT,
  TOKEN_NAMES,
  presetById,
  type Scheme,
  type ThemeTokens,
} from "./presets";
import { parseOklch } from "./oklch";

export const KEY = "dzch.theme";

export type WindowFrame = {
  /** Corner radius in px when not maximised (Linux compositors honour it). */
  radius: number;
  /** Border width in px; 0 for none. */
  border: number;
  borderFocus: string;
  borderBlur: string;
};

/** One of the player's own themes. `id` always starts with `custom:`. */
export type CustomTheme = {
  id: string;
  name: string;
  scheme: Scheme;
  tokens: ThemeTokens;
  /** The preset it was started from, which "reset" goes back to. */
  base?: string;
};

/** Which preset "follow the system" wears for each OS scheme. */
export type SystemPair = { dark: string; light: string };

export type Stored = {
  /** A preset id, a custom theme's id, or null to follow the OS. */
  preset: string | null;
  customs: CustomTheme[];
  pair: SystemPair;
  frame: WindowFrame;
};

export const DEFAULT_FRAME: WindowFrame = {
  radius: 0,
  border: 0,
  borderFocus: "oklch(78.8% 0.141 85)",
  borderBlur: "oklch(30% 0.01 130)",
};
export const DEFAULT_PAIR: SystemPair = { dark: DEFAULT_DARK, light: DEFAULT_LIGHT };

export const isCustom = (id: string | null | undefined): id is string =>
  !!id?.startsWith("custom:");
export const newId = () =>
  `custom:${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;

/** A preset id as it is now: a retired preset's survivor, or null if unknown. */
export function resolvePreset(id: string | null | undefined, scheme?: Scheme): string | null {
  if (!id) return null;
  const p = presetById(id);
  return p && (!scheme || p.scheme === scheme) ? p.id : null;
}

/** Every token, each one an OKLCH colour; what is missing or unreadable comes from `base`. */
export function fullTokens(t: Partial<ThemeTokens> | undefined, base: ThemeTokens): ThemeTokens {
  return Object.fromEntries(
    TOKEN_NAMES.map((n) => {
      const v = t?.[n];
      return [n, typeof v === "string" && parseOklch(v) ? v : base[n]];
    }),
  ) as ThemeTokens;
}

/**
 * Read what was stored, from any earlier shape:
 * - v1 (this interface's first): `{preset: id | "custom" | null,
 *   custom: {scheme, tokens} | null, frame}`; the single custom becomes the
 *   first of the list, named "My theme", and "custom" selects it;
 * - the previous interface: `active-preset-id` and `window-settings`;
 * - a retired preset id resolves to its survivor (see `RETIRED` in presets.ts).
 */
export function load(get: (key: string) => string | null): Stored {
  const fallback = presetById(DEFAULT_DARK)!.tokens;
  try {
    const raw = get(KEY);
    if (raw) {
      const s = JSON.parse(raw) as Partial<Stored> & {
        custom?: { scheme?: Scheme; tokens?: Partial<ThemeTokens> } | null;
      };
      const customs: CustomTheme[] = Array.isArray(s.customs)
        ? s.customs
            .filter((c) => c && isCustom(c.id))
            .map((c) => ({
              ...c,
              name: c.name || "Custom",
              scheme: c.scheme === "light" ? "light" : "dark",
              tokens: fullTokens(c.tokens, fallback),
            }))
        : [];
      let preset = s.preset ?? null;
      if (s.custom?.tokens && customs.length === 0) {
        customs.push({
          id: newId(),
          name: "My theme",
          scheme: s.custom.scheme === "light" ? "light" : "dark",
          tokens: fullTokens(s.custom.tokens, fallback),
        });
      }
      if (preset === "custom") preset = customs[0]?.id ?? null;
      else if (isCustom(preset)) preset = customs.some((c) => c.id === preset) ? preset : null;
      else preset = resolvePreset(preset);
      return {
        preset,
        customs,
        pair: {
          dark: resolvePreset(s.pair?.dark, "dark") ?? DEFAULT_DARK,
          light: resolvePreset(s.pair?.light, "light") ?? DEFAULT_LIGHT,
        },
        frame: { ...DEFAULT_FRAME, ...s.frame },
      };
    }
    // The previous interface kept the preset id here; "dark"/"light" were its
    // DaisyUI defaults, which are now chernarus and topo.
    const legacy = get("active-preset-id");
    const preset =
      legacy === "dark" ? DEFAULT_DARK : legacy === "light" ? DEFAULT_LIGHT : resolvePreset(legacy);
    const win = get("window-settings");
    const w = win ? (JSON.parse(win) as Record<string, string>) : {};
    return {
      preset,
      customs: [],
      pair: { ...DEFAULT_PAIR },
      frame: {
        ...DEFAULT_FRAME,
        radius: parseFloat(w.windowRadius ?? "0") || 0,
        border: parseFloat(w.windowBorderSize ?? "0") || 0,
        borderFocus: w.windowBorderFocus ?? DEFAULT_FRAME.borderFocus,
        borderBlur: w.windowBorderBlur ?? DEFAULT_FRAME.borderBlur,
      },
    };
  } catch {
    return { preset: null, customs: [], pair: { ...DEFAULT_PAIR }, frame: { ...DEFAULT_FRAME } };
  }
}
