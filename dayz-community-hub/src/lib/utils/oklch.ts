/**
 * OKLCH color utilities for theme editor
 * Converts between OKLCH (used in CSS variables) and hex/RGB for color pickers
 */

export interface OklchColor {
  l: number; // Lightness 0-1
  c: number; // Chroma 0-0.4
  h: number; // Hue 0-360
}

/**
 * Parse OKLCH string like "oklch(65% 0.20 255)" to components
 */
export function parseOklch(oklchStr: string): OklchColor | null {
  const match = oklchStr.match(/oklch\(\s*([\d.]+)%?\s+([\d.]+)\s+([\d.]+)\s*\)/i);
  if (!match) return null;

  let l = parseFloat(match[1]);
  // If percentage, convert to 0-1
  if (l > 1) l = l / 100;

  return {
    l,
    c: parseFloat(match[2]),
    h: parseFloat(match[3])
  };
}

/**
 * Format OKLCH color to CSS string
 */
export function formatOklch(color: OklchColor): string {
  return `oklch(${Math.round(color.l * 100)}% ${color.c.toFixed(2)} ${Math.round(color.h)})`;
}

/**
 * Convert OKLCH to sRGB
 * Based on https://bottosson.github.io/posts/oklab/
 */
export function oklchToRgb(color: OklchColor): { r: number; g: number; b: number } {
  const { l, c, h } = color;

  // Convert OKLCH to OKLab
  const hRad = (h * Math.PI) / 180;
  const a = c * Math.cos(hRad);
  const b = c * Math.sin(hRad);

  // OKLab to linear RGB
  const l_ = l + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = l - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = l - 0.0894841775 * a - 1.291485548 * b;

  const L = l_ * l_ * l_;
  const M = m_ * m_ * m_;
  const S = s_ * s_ * s_;

  let r = +4.0767416621 * L - 3.3077115913 * M + 0.2309699292 * S;
  let g = -1.2684380046 * L + 2.6097574011 * M - 0.3413193965 * S;
  let bVal = -0.0041960863 * L - 0.7034186147 * M + 1.707614701 * S;

  // Linear to sRGB gamma correction
  const gammaCorrect = (x: number) => {
    if (x >= 0.0031308) {
      return 1.055 * Math.pow(x, 1 / 2.4) - 0.055;
    }
    return 12.92 * x;
  };

  r = Math.round(Math.max(0, Math.min(1, gammaCorrect(r))) * 255);
  g = Math.round(Math.max(0, Math.min(1, gammaCorrect(g))) * 255);
  bVal = Math.round(Math.max(0, Math.min(1, gammaCorrect(bVal))) * 255);

  return { r, g, b: bVal };
}

/**
 * Convert RGB to OKLCH
 */
export function rgbToOklch(r: number, g: number, b: number): OklchColor {
  // Normalize to 0-1
  r = r / 255;
  g = g / 255;
  b = b / 255;

  // sRGB to linear RGB
  const linearize = (x: number) => {
    if (x >= 0.04045) {
      return Math.pow((x + 0.055) / 1.055, 2.4);
    }
    return x / 12.92;
  };

  const rLin = linearize(r);
  const gLin = linearize(g);
  const bLin = linearize(b);

  // Linear RGB to OKLab
  const L = Math.cbrt(0.4122214708 * rLin + 0.5363325363 * gLin + 0.0514459929 * bLin);
  const M = Math.cbrt(0.2119034982 * rLin + 0.6806995451 * gLin + 0.1073969566 * bLin);
  const S = Math.cbrt(0.0883024619 * rLin + 0.2817188376 * gLin + 0.6299787005 * bLin);

  const l = 0.2104542553 * L + 0.793617785 * M - 0.0040720468 * S;
  const a = 1.9779984951 * L - 2.428592205 * M + 0.4505937099 * S;
  const bVal = 0.0259040371 * L + 0.7827717662 * M - 0.808675766 * S;

  // OKLab to OKLCH
  const c = Math.sqrt(a * a + bVal * bVal);
  let h = (Math.atan2(bVal, a) * 180) / Math.PI;
  if (h < 0) h += 360;

  return { l, c, h };
}

/**
 * Convert OKLCH to hex color string
 */
export function oklchToHex(color: OklchColor): string {
  const { r, g, b } = oklchToRgb(color);
  return `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
}

/**
 * Parse hex color to OKLCH
 */
export function hexToOklch(hex: string): OklchColor {
  const cleanHex = hex.replace('#', '');
  const r = parseInt(cleanHex.substring(0, 2), 16);
  const g = parseInt(cleanHex.substring(2, 4), 16);
  const b = parseInt(cleanHex.substring(4, 6), 16);
  return rgbToOklch(r, g, b);
}
