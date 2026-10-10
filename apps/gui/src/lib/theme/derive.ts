/**
 * A whole theme from two colours: the window floor and the accent.
 *
 * How the other ten are made, all in OKLCH so a step in lightness looks
 * like the same step at every hue:
 * - scheme: dark when the floor is under 60% lightness, as the presets are;
 * - panel and raised: the floor's hue with a little more chroma, panel one
 *   step towards the text (+3.5% dark, +5% light) and raised a control's
 *   step (+8.5% dark, −4% light: a light theme's controls sit in the paper);
 * - fg: the floor's hue at 92% (dark) or 24% (light), pushed further out
 *   until it reaches 7:1 on the floor and on the panel, and the muted and
 *   faint text mixed from it reach theirs;
 * - accentFg: near-black or near-white in the accent's hue, whichever
 *   contrasts more; if neither reaches 4.5:1 the accent itself is moved
 *   away from mid-lightness, a step at a time;
 * - ok/warn/err/info/map/mods: fixed hues (green 140, amber 65, red 28,
 *   blue 240, khaki 90, teal 175) at the presets' usual chroma, at 74%
 *   (dark) or 52% (light), moved in lightness until each reaches 3:1 on the
 *   panel; a tone that lands too close to the accent (OKLab < DISTINCT_MIN)
 *   steps away from it in lightness.
 * The result passes `audit()` for any floor and any accent that is not
 * itself grey; the editor shows the checks either way.
 */
import { DISTINCT_MIN, deltaE, mixOklch, wcag } from "./contrast";
import { formatOklch, parseOklch, type OklchColor } from "./oklch";
import type { Scheme, ThemeTokens } from "./presets";

const clamp = (v: number, lo: number, hi: number) => Math.max(lo, Math.min(hi, v));
const fmt = (c: OklchColor) => formatOklch({ ...c, l: clamp(c.l, 0, 1), c: Math.max(0, c.c) });

/** Move `c` in lightness (`dir` ±1) until it reaches `min` against every ground. */
function reach(c: OklchColor, grounds: string[], min: number, dir: number): OklchColor {
  let out = { ...c };
  for (let i = 0; i < 40; i++) {
    if (grounds.every((g) => wcag(fmt(out), g) >= min)) break;
    out = { ...out, l: clamp(out.l + dir * 0.01, 0, 1) };
  }
  return out;
}

export function deriveTheme(
  bgColor: string,
  accentColor: string,
): {
  scheme: Scheme;
  tokens: ThemeTokens;
} {
  const bg = parseOklch(bgColor) ?? { l: 0.18, c: 0.01, h: 130 };
  let accent = parseOklch(accentColor) ?? { l: 0.79, c: 0.14, h: 85 };
  const dark = bg.l < 0.6;
  const toward = dark ? 1 : -1;

  const tint = Math.min(bg.c * 1.4 + 0.004, 0.06);
  const panel = { l: clamp(bg.l + (dark ? 0.035 : 0.05), 0, 0.995), c: tint, h: bg.h };
  const raised = { l: clamp(bg.l + (dark ? 0.085 : -0.04), 0, 1), c: tint * 1.3, h: bg.h };
  const grounds = [fmt(bg), fmt(panel)];

  let fg = reach({ l: dark ? 0.92 : 0.24, c: 0.02, h: bg.h }, grounds, 7, toward);
  // Muted and faint text are mixed from fg and the floor; they pass too.
  for (let i = 0; i < 20; i++) {
    const muted = mixOklch(fmt(fg), fmt(bg), 0.68);
    const faint = mixOklch(fmt(fg), fmt(bg), 0.5);
    if (grounds.every((g) => wcag(muted, g) >= 4.5 && wcag(faint, g) >= 3)) break;
    fg = { ...fg, l: clamp(fg.l + toward * 0.01, 0, 1) };
  }

  // The accent's label: whichever end contrasts more.
  const ends = (a: OklchColor) => {
    const black = { l: 0.18, c: Math.min(a.c, 0.03), h: a.h };
    const white = { l: 0.985, c: Math.min(a.c, 0.015), h: a.h };
    const cb = wcag(fmt(black), fmt(a));
    const cw = wcag(fmt(white), fmt(a));
    return cb >= cw ? { c: black, r: cb, away: 1 } : { c: white, r: cw, away: -1 };
  };
  let label = ends(accent);
  for (let i = 0; i < 20 && label.r < 4.5; i++) {
    accent = { ...accent, l: clamp(accent.l + label.away * 0.01, 0, 1) };
    label = ends(accent);
  }
  // A mark must also stand out from the panel.
  accent = reach(accent, [fmt(panel)], 3, toward);

  const tone = (h: number, c: number): string => {
    let t = reach({ l: dark ? 0.74 : 0.52, c, h }, [fmt(panel)], 3, toward);
    for (let i = 0; i < 12 && deltaE(fmt(t), fmt(accent)) < DISTINCT_MIN; i++)
      t = { ...t, l: clamp(t.l + toward * 0.02, 0, 1) };
    return fmt(t);
  };

  return {
    scheme: dark ? "dark" : "light",
    tokens: {
      bg: fmt(bg),
      panel: fmt(panel),
      raised: fmt(raised),
      fg: fmt(fg),
      accent: fmt(accent),
      accentFg: fmt(label.c),
      ok: tone(140, 0.14),
      warn: tone(65, 0.145),
      err: tone(28, 0.17),
      info: tone(240, 0.08),
      map: tone(90, 0.08),
      mods: tone(175, 0.09),
    },
  };
}
