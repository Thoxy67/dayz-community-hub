/**
 * The theme presets: twelve colours each, everything else is derived in
 * `styles/tokens.css` (borders, muted text, tints) so a preset cannot forget one.
 *
 * A curated set: the two house themes, then dark and light ones that each
 * have a reason to exist. Every one passes the legibility rules in
 * `contrast.ts` (`theme.test.ts` checks). Surfaces are sorted so `bg` is the
 * window floor, `panel` what content sits on and `raised` a control or a
 * hover. Community palettes keep their own colours where they pass; a
 * colour that did not (too faint text, a status tone that vanished on the
 * panel) was moved in lightness only, keeping its hue.
 *
 * Ids are stable: a stored choice or a `?theme=` flag names one. Presets
 * that were removed resolve through `RETIRED` below.
 */
export const TOKEN_NAMES = [
  "bg",
  "panel",
  "raised",
  "fg",
  "accent",
  "accentFg",
  "ok",
  "warn",
  "err",
  "info",
  "map",
  "mods",
] as const;
export type TokenName = (typeof TOKEN_NAMES)[number];
export type ThemeTokens = Record<TokenName, string>;
export type Scheme = "dark" | "light";
/** `blurb` is a key of the `theme` dictionary: the one-line character. */
export type Preset = {
  readonly id: string;
  readonly name: string;
  readonly blurb: `blurb${string}`;
  readonly scheme: Scheme;
  readonly tokens: ThemeTokens;
};

export const PRESETS = [
  // The launcher's own: khaki ground, amber accent.
  {
    id: "chernarus",
    name: "Chernarus",
    blurb: "blurbChernarus",
    scheme: "dark",
    tokens: {
      bg: "oklch(18.3% 0.007 135)",
      panel: "oklch(21.7% 0.012 129)",
      raised: "oklch(26.6% 0.017 127)",
      fg: "oklch(91.4% 0.022 98)",
      accent: "oklch(78.8% 0.141 85)",
      accentFg: "oklch(19.5% 0.019 98)",
      ok: "oklch(74.8% 0.141 130)",
      warn: "oklch(73.4% 0.144 62)",
      err: "oklch(61.9% 0.170 32)",
      info: "oklch(70.1% 0.063 246)",
      map: "oklch(77.0% 0.078 88)",
      mods: "oklch(74.7% 0.086 173)",
    },
  },
  // The launcher's own in daylight: survey-map paper, olive ink.
  {
    id: "topo",
    name: "Topo",
    blurb: "blurbTopo",
    scheme: "light",
    tokens: {
      bg: "oklch(90.3% 0.018 121)",
      panel: "oklch(95.1% 0.012 117)",
      raised: "oklch(86.4% 0.024 120)",
      fg: "oklch(24.1% 0.016 129)",
      accent: "oklch(53.2% 0.111 78)",
      accentFg: "oklch(97.3% 0.022 93)",
      ok: "oklch(52.9% 0.133 139)",
      warn: "oklch(55.7% 0.137 52)",
      err: "oklch(51.2% 0.169 31)",
      info: "oklch(48.0% 0.079 241)",
      map: "oklch(49.8% 0.089 85)",
      mods: "oklch(51.1% 0.079 174)",
    },
  },
  // Pure black floor and the house amber, for OLED screens and night play.
  {
    id: "oled",
    name: "OLED",
    blurb: "blurbOled",
    scheme: "dark",
    tokens: {
      bg: "oklch(0% 0 0)",
      panel: "oklch(10.5% 0.004 85)",
      raised: "oklch(18% 0.006 85)",
      fg: "oklch(94% 0.008 90)",
      accent: "oklch(78.8% 0.141 85)",
      accentFg: "oklch(16% 0.02 85)",
      ok: "oklch(76% 0.15 140)",
      warn: "oklch(76% 0.15 60)",
      err: "oklch(66% 0.19 28)",
      info: "oklch(74% 0.1 240)",
      map: "oklch(82% 0.06 95)",
      mods: "oklch(76% 0.1 180)",
    },
  },
  // GitHub's dark theme: blue-black, GitHub blue. Palette: https://primer.style/foundations/color
  {
    id: "github_dark",
    name: "GitHub Dark",
    blurb: "blurbGithubDark",
    scheme: "dark",
    tokens: {
      bg: "oklch(17.6% 0.014 258)",
      panel: "oklch(22.0% 0.016 257)",
      raised: "oklch(26.7% 0.015 257)",
      fg: "oklch(94.3% 0.011 244)",
      accent: "oklch(71.5% 0.152 253)",
      accentFg: "oklch(17.6% 0.014 258)",
      ok: "oklch(69.5% 0.181 146)",
      warn: "oklch(72.0% 0.140 80)",
      err: "oklch(66.5% 0.205 27)",
      info: "oklch(75.5% 0.116 202)",
      map: "oklch(79.0% 0.139 85)",
      mods: "oklch(66.1% 0.193 298)",
    },
  },
  // Folke Lemaitre's Tokyo Night: cold night blue, neon marks. Palette: https://github.com/folke/tokyonight.nvim
  {
    id: "tokyonight",
    name: "Tokyo Night",
    blurb: "blurbTokyonight",
    scheme: "dark",
    tokens: {
      bg: "oklch(20.4% 0.016 285)",
      panel: "oklch(22.6% 0.021 280)",
      raised: "oklch(30.6% 0.037 273)",
      fg: "oklch(84.6% 0.061 275)",
      accent: "oklch(71.9% 0.132 264)",
      accentFg: "oklch(20.4% 0.016 285)",
      ok: "oklch(79.5% 0.139 130)",
      warn: "oklch(78.7% 0.137 51)",
      err: "oklch(72.3% 0.159 10)",
      info: "oklch(82.0% 0.105 236)",
      map: "oklch(78.4% 0.106 75)",
      mods: "oklch(75.1% 0.134 299)",
    },
  },
  // Arctic Ice Studio's Nord: slate and frost, soft contrast. Palette: https://www.nordtheme.com/docs/colors-and-palettes
  {
    id: "nord",
    name: "Nord",
    blurb: "blurbNord",
    scheme: "dark",
    tokens: {
      bg: "oklch(32.4% 0.023 264)",
      panel: "oklch(35.2% 0.026 267)",
      raised: "oklch(41.6% 0.032 264)",
      fg: "oklch(95.1% 0.007 261)",
      accent: "oklch(77.5% 0.062 217)",
      accentFg: "oklch(32.4% 0.023 264)",
      ok: "oklch(76.8% 0.075 131)",
      warn: "oklch(69.3% 0.096 38)",
      err: "oklch(69% 0.15 8)",
      info: "oklch(69.7% 0.059 249)",
      map: "oklch(85.5% 0.089 84)",
      mods: "oklch(69.2% 0.062 333)",
    },
  },
  // Deep pine green with moss and lichen marks.
  {
    id: "forest",
    name: "Forest",
    blurb: "blurbForest",
    scheme: "dark",
    tokens: {
      bg: "oklch(16% 0.022 150)",
      panel: "oklch(19.5% 0.026 150)",
      raised: "oklch(25% 0.032 150)",
      fg: "oklch(91% 0.025 130)",
      accent: "oklch(74% 0.13 165)",
      accentFg: "oklch(16% 0.03 165)",
      ok: "oklch(78% 0.16 130)",
      warn: "oklch(77% 0.14 75)",
      err: "oklch(66% 0.18 30)",
      info: "oklch(72% 0.09 230)",
      map: "oklch(80% 0.09 95)",
      mods: "oklch(74% 0.1 300)",
    },
  },
  // Olive drab and coyote tan, low glare.
  {
    id: "military",
    name: "Military",
    blurb: "blurbMilitary",
    scheme: "dark",
    tokens: {
      bg: "oklch(20% 0.02 115)",
      panel: "oklch(24% 0.025 115)",
      raised: "oklch(30% 0.03 113)",
      fg: "oklch(90% 0.025 100)",
      accent: "oklch(76% 0.08 75)",
      accentFg: "oklch(20% 0.02 100)",
      ok: "oklch(76% 0.14 128)",
      warn: "oklch(76% 0.15 58)",
      err: "oklch(67% 0.18 30)",
      info: "oklch(73% 0.08 230)",
      map: "oklch(83% 0.07 100)",
      mods: "oklch(74% 0.07 175)",
    },
  },
  // Near-black with a crimson accent.
  {
    id: "blood_moon",
    name: "Blood Moon",
    blurb: "blurbBloodMoon",
    scheme: "dark",
    tokens: {
      bg: "oklch(13% 0.018 20)",
      panel: "oklch(17% 0.024 20)",
      raised: "oklch(23% 0.032 20)",
      fg: "oklch(90% 0.02 30)",
      accent: "oklch(55% 0.2 25)",
      accentFg: "oklch(98% 0.01 25)",
      ok: "oklch(72% 0.14 140)",
      warn: "oklch(78% 0.14 70)",
      err: "oklch(72% 0.17 40)",
      info: "oklch(70% 0.1 240)",
      map: "oklch(80% 0.08 60)",
      mods: "oklch(70% 0.14 345)",
    },
  },
  // Warm brown and lamplight amber.
  {
    id: "sepia",
    name: "Sepia",
    blurb: "blurbSepia",
    scheme: "dark",
    tokens: {
      bg: "oklch(21% 0.022 60)",
      panel: "oklch(25% 0.027 60)",
      raised: "oklch(31% 0.032 60)",
      fg: "oklch(90% 0.03 75)",
      accent: "oklch(76% 0.13 72)",
      accentFg: "oklch(20% 0.03 60)",
      ok: "oklch(74% 0.12 125)",
      warn: "oklch(72% 0.15 45)",
      err: "oklch(66% 0.17 25)",
      info: "oklch(72% 0.07 225)",
      map: "oklch(82% 0.07 90)",
      mods: "oklch(74% 0.07 170)",
    },
  },
  // Pavel Pertsev's Gruvbox: retro yellow on charcoal. Palette: https://github.com/morhetz/gruvbox
  {
    id: "gruvbox_dark",
    name: "Gruvbox Dark",
    blurb: "blurbGruvboxDark",
    scheme: "dark",
    tokens: {
      bg: "oklch(27.7% 0.000 90)",
      panel: "oklch(31.1% 0.003 49)",
      raised: "oklch(37.5% 0.009 53)",
      fg: "oklch(89.4% 0.057 89)",
      accent: "oklch(83.2% 0.159 83)",
      accentFg: "oklch(27.7% 0.000 90)",
      ok: "oklch(76.5% 0.158 111)",
      warn: "oklch(73.1% 0.182 52)",
      err: "oklch(66.0% 0.218 30)",
      info: "oklch(69.3% 0.042 170)",
      map: "oklch(75.6% 0.108 138)",
      mods: "oklch(70.5% 0.098 2)",
    },
  },
  // Catppuccin's Mocha flavour: pastel mauve on deep indigo. Palette: https://catppuccin.com/palette
  {
    id: "catppuccin",
    name: "Catppuccin Mocha",
    blurb: "blurbCatppuccin",
    scheme: "dark",
    tokens: {
      bg: "oklch(21.6% 0.025 284)",
      panel: "oklch(24.3% 0.030 284)",
      raised: "oklch(32.4% 0.032 282)",
      fg: "oklch(87.9% 0.043 272)",
      accent: "oklch(78.7% 0.119 305)",
      accentFg: "oklch(18.3% 0.020 284)",
      ok: "oklch(85.8% 0.109 143)",
      warn: "oklch(82.4% 0.101 53)",
      err: "oklch(75.6% 0.130 3)",
      info: "oklch(76.6% 0.111 260)",
      map: "oklch(91.9% 0.070 87)",
      mods: "oklch(85.8% 0.079 183)",
    },
  },
  // Warm paper and rust ink.
  {
    id: "sand",
    name: "Sand",
    blurb: "blurbSand",
    scheme: "light",
    tokens: {
      bg: "oklch(88% 0.025 70)",
      panel: "oklch(95% 0.018 75)",
      raised: "oklch(78% 0.035 65)",
      fg: "oklch(20% 0.025 55)",
      accent: "oklch(52% 0.14 55)",
      accentFg: "oklch(98% 0.01 55)",
      ok: "oklch(48% 0.16 145)",
      warn: "oklch(58% 0.18 65)",
      err: "oklch(50% 0.20 25)",
      info: "oklch(50% 0.14 220)",
      map: "oklch(58% 0.18 65)",
      mods: "oklch(48% 0.12 85)",
    },
  },
  // GitHub's light theme: white, cool grey and GitHub blue. Palette: https://primer.style/foundations/color
  {
    id: "github_light",
    name: "GitHub Light",
    blurb: "blurbGithubLight",
    scheme: "light",
    tokens: {
      bg: "oklch(97.8% 0.003 248)",
      panel: "oklch(100.0% 0.000 90)",
      raised: "oklch(87.6% 0.012 248)",
      fg: "oklch(25.4% 0.011 254)",
      accent: "oklch(54.0% 0.191 257)",
      accentFg: "oklch(100.0% 0.000 90)",
      ok: "oklch(52.4% 0.140 148)",
      warn: "oklch(55.4% 0.117 75)",
      err: "oklch(55.7% 0.206 25)",
      info: "oklch(53.7% 0.084 202)",
      map: "oklch(46.6% 0.101 70)",
      mods: "oklch(56.3% 0.207 295)",
    },
  },
  // Gruvbox on cream paper, earthy marks. Palette: https://github.com/morhetz/gruvbox
  {
    id: "gruvbox_light",
    name: "Gruvbox Light",
    blurb: "blurbGruvboxLight",
    scheme: "light",
    tokens: {
      bg: "oklch(92.2% 0.055 93)",
      panel: "oklch(95.6% 0.055 96)",
      raised: "oklch(85.8% 0.057 88)",
      fg: "oklch(27.7% 0.000 90)",
      accent: "oklch(51.3% 0.162 39)",
      accentFg: "oklch(95.6% 0.055 96)",
      ok: "oklch(54.6% 0.112 106)",
      warn: "oklch(61.8% 0.128 71)",
      err: "oklch(43.7% 0.179 28)",
      info: "oklch(47.1% 0.082 216)",
      map: "oklch(53.4% 0.082 155)",
      mods: "oklch(48.9% 0.124 344)",
    },
  },
  // Catppuccin's Latte flavour: soft lilac grey, vivid marks. Palette: https://catppuccin.com/palette
  {
    id: "catppuccin_latte",
    name: "Catppuccin Latte",
    blurb: "blurbCatppuccinLatte",
    scheme: "light",
    tokens: {
      bg: "oklch(93.3% 0.009 265)",
      panel: "oklch(95.8% 0.006 265)",
      raised: "oklch(85.7% 0.014 268)",
      fg: "oklch(30% 0.04 280)",
      accent: "oklch(55.5% 0.250 297)",
      accentFg: "oklch(95.8% 0.006 265)",
      ok: "oklch(54% 0.15 140)",
      warn: "oklch(58% 0.17 48)",
      err: "oklch(55.0% 0.216 20)",
      info: "oklch(55.9% 0.226 262)",
      map: "oklch(57% 0.12 75)",
      mods: "oklch(60.2% 0.098 201)",
    },
  },
  // Rosé Pine's Dawn variant: parchment, rose and pine. Palette: https://rosepinetheme.com/palette/ingredients/
  {
    id: "rosepine_dawn",
    name: "Rosé Pine Dawn",
    blurb: "blurbRosepineDawn",
    scheme: "light",
    tokens: {
      bg: "oklch(97.0% 0.011 72)",
      panel: "oklch(98.7% 0.011 77)",
      raised: "oklch(93.9% 0.015 64)",
      fg: "oklch(31% 0.05 290)",
      accent: "oklch(56% 0.12 28)",
      accentFg: "oklch(98.7% 0.011 77)",
      ok: "oklch(49.1% 0.077 228)",
      warn: "oklch(60% 0.13 68)",
      err: "oklch(50% 0.15 0)",
      info: "oklch(55% 0.09 300)",
      map: "oklch(57% 0.1 78)",
      mods: "oklch(55% 0.07 205)",
    },
  },
] as const satisfies readonly Preset[];

export type PresetId = (typeof PRESETS)[number]["id"];

export const DEFAULT_DARK: PresetId = "chernarus";
export const DEFAULT_LIGHT: PresetId = "topo";

/** The house themes, whose names are translated (`presetChernarus`, `presetTopo`). */
export const HOUSE: readonly string[] = [DEFAULT_DARK, DEFAULT_LIGHT];

/**
 * Presets that were removed, and the survivor each one now resolves to, so a
 * stored choice keeps working:
 * - near-duplicate dark greys and blues: neutral, midnight, kanagawa →
 *   their closest survivor;
 * - purples that were catppuccin again: dracula, rosepine, twilight;
 * - mocha was sepia made lighter, with every status tone under 3:1 on its
 *   panel;
 * - ocean, rose, mint, lavender were one light tint each with the same
 *   status colours: four of a kind, none with a character of its own;
 * - tokyonight_light's text and marks failed, and fixed it was GitHub Light.
 */
export const RETIRED: Readonly<Record<string, PresetId>> = {
  neutral: "github_dark",
  midnight: "tokyonight",
  kanagawa: "tokyonight",
  dracula: "catppuccin",
  rosepine: "catppuccin",
  twilight: "catppuccin",
  mocha: "sepia",
  ocean: "github_light",
  rose: "rosepine_dawn",
  mint: "topo",
  lavender: "catppuccin_latte",
  tokyonight_light: "github_light",
};

export function presetById(id: string): Preset | undefined {
  return PRESETS.find((p) => p.id === (RETIRED[id] ?? id));
}
