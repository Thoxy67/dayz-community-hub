/**
 * How legible a theme is: the colours `styles/tokens.css` derives from the
 * twelve, worked out the same way here, and the contrast between the pairs
 * the interface actually draws. One rule set serves the preset test
 * (`theme.test.ts`) and the custom editor's live readout.
 */
import { encodeSrgb, formatOklch, oklchToLinear, parseOklch, type OklchColor } from "./oklch";
import type { ThemeTokens } from "./presets";

function read(color: string): OklchColor {
  return parseOklch(color) ?? { l: 0, c: 0, h: 0 };
}

/**
 * `color-mix(in oklch, a p, b)` as the browser computes it: lightness and
 * chroma mixed linearly, hue the shorter way round; a grey has no hue to
 * contribute ("powerless", CSS Color 4 §4.4 and §12.4,
 * https://www.w3.org/TR/css-color-4/#interpolation-missing).
 */
export function mixOklch(a: string, b: string, p: number): string {
  const x = read(a);
  const y = read(b);
  const grey = 1e-4;
  let h: number;
  if (x.c < grey) h = y.h;
  else if (y.c < grey) h = x.h;
  else {
    let d = y.h - x.h;
    if (d > 180) d -= 360;
    else if (d < -180) d += 360;
    h = x.h + d * (1 - p);
  }
  return formatOklch({ l: x.l * p + y.l * (1 - p), c: x.c * p + y.c * (1 - p), h });
}

/** The text colours tokens.css derives (`--color-fg-muted`, `--color-fg-faint`). */
export function derivedText(t: ThemeTokens): { muted: string; faint: string } {
  return { muted: mixOklch(t.fg, t.bg, 0.68), faint: mixOklch(t.fg, t.bg, 0.5) };
}

/** sRGB channels 0..1 (gamut clipped), as the screen shows them. */
function srgb(color: string): [number, number, number] {
  const lin = oklchToLinear(read(color));
  return [encodeSrgb(lin.r), encodeSrgb(lin.g), encodeSrgb(lin.b)];
}

/**
 * WCAG 2 relative luminance and contrast ratio, 1..21.
 * https://www.w3.org/TR/WCAG22/#dfn-relative-luminance
 * https://www.w3.org/TR/WCAG22/#dfn-contrast-ratio
 */
export function luminance(color: string): number {
  const [r, g, b] = srgb(color).map((v) =>
    v <= 0.04045 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4),
  ) as [number, number, number];
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

export function wcag(a: string, b: string): number {
  const x = luminance(a);
  const y = luminance(b);
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

/**
 * APCA lightness contrast (Lc), text on ground; positive for dark text on a
 * light ground, negative the other way. APCA-W3 0.0.98G-4g constants, from
 * https://github.com/Myndex/apca-w3/blob/master/src/apca-w3.js
 * Lc 75 is the floor for body text, 60 for larger or bold text, 45 for
 * large headings and 30 for non-text (https://readtech.org/ARC/).
 */
export function apca(text: string, ground: string): number {
  const y = (c: string) => {
    const [r, g, b] = srgb(c);
    const v = 0.2126729 * r ** 2.4 + 0.7151522 * g ** 2.4 + 0.072175 * b ** 2.4;
    return v > 0.022 ? v : v + (0.022 - v) ** 1.414;
  };
  const yt = y(text);
  const yb = y(ground);
  if (Math.abs(yb - yt) < 0.0005) return 0;
  if (yb > yt) {
    const s = (yb ** 0.56 - yt ** 0.57) * 1.14;
    return s < 0.1 ? 0 : (s - 0.027) * 100;
  }
  const s = (yb ** 0.65 - yt ** 0.62) * 1.14;
  return s > -0.1 ? 0 : (s + 0.027) * 100;
}

/** Distance in OKLab: how different two colours look, hue and lightness both. */
export function deltaE(a: string, b: string): number {
  const x = read(a);
  const y = read(b);
  const ax = x.c * Math.cos((x.h * Math.PI) / 180);
  const bx = x.c * Math.sin((x.h * Math.PI) / 180);
  const ay = y.c * Math.cos((y.h * Math.PI) / 180);
  const by = y.c * Math.sin((y.h * Math.PI) / 180);
  return Math.hypot(x.l - y.l, ax - ay, bx - by);
}

/** A pair the interface draws, and the WCAG ratio it must reach. */
export type Check = {
  /** Stable id, also the content key suffix for its label. */
  id: string;
  fg: string;
  bg: string;
  min: number;
  ratio: number;
  pass: boolean;
};

/**
 * The rule set. WCAG 2 ratios (AAA for body text, AA for the rest, 3:1 for
 * non-text and for the faint captions that are never the only carrier of
 * information):
 * - text on the floor and on panels 7:1, on a raised control 4.5:1;
 * - the accent button's label 4.5:1;
 * - muted text 4.5:1 and faint text 3:1 on the floor and on panels;
 * - every coloured mark (status tones, accent, map, mods) 3:1 on a panel,
 *   since each is also used as text: a map name, a ping figure.
 */
export function audit(t: ThemeTokens): Check[] {
  const { muted, faint } = derivedText(t);
  const pairs: Array<[string, string, string, number]> = [
    ["fgBg", t.fg, t.bg, 7],
    ["fgPanel", t.fg, t.panel, 7],
    ["fgRaised", t.fg, t.raised, 4.5],
    ["accentFg", t.accentFg, t.accent, 4.5],
    ["muted", muted, t.bg, 4.5],
    ["mutedPanel", muted, t.panel, 4.5],
    ["faint", faint, t.bg, 3],
    ["faintPanel", faint, t.panel, 3],
    ["accent", t.accent, t.panel, 3],
    ["ok", t.ok, t.panel, 3],
    ["warn", t.warn, t.panel, 3],
    ["err", t.err, t.panel, 3],
    ["info", t.info, t.panel, 3],
    ["map", t.map, t.panel, 3],
    ["mods", t.mods, t.panel, 3],
  ];
  return pairs.map(([id, fg, bg, min]) => {
    const ratio = wcag(fg, bg);
    return { id, fg, bg, min, ratio, pass: ratio >= min };
  });
}

/**
 * Colours that must not be mistaken for one another: the four status tones
 * among themselves, and map against mods. OKLab distance 0.06 is about where
 * two small marks stop reading as "the same colour" at a glance (a JND is
 * ~0.02, https://bottosson.github.io/posts/oklab/; three of them for marks a
 * few pixels wide). Returns the pairs that are too close.
 */
export const DISTINCT_MIN = 0.06;
export function confusable(t: ThemeTokens): Array<[string, string, number]> {
  const groups: Array<Array<keyof ThemeTokens>> = [
    ["ok", "warn", "err", "info", "accent"],
    ["map", "mods"],
  ];
  const out: Array<[string, string, number]> = [];
  for (const g of groups)
    for (let i = 0; i < g.length; i++)
      for (let j = i + 1; j < g.length; j++) {
        const d = deltaE(t[g[i]!], t[g[j]!]);
        if (d < DISTINCT_MIN) out.push([g[i]!, g[j]!, d]);
      }
  return out;
}
