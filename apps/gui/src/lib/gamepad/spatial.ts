/**
 * Which element a direction leads to: pure geometry over rectangles, so it is
 * tested without a page. The DOM side (`nav.ts`) gathers the candidates.
 *
 * A candidate counts only if it lies on the side the direction points to.
 * Among those, the nearest wins, where "near" is the gap along the direction
 * plus the sideways gap weighted heavier: pressing Down from a button goes
 * to what is under it, not to something a little closer but off to the side.
 */
export type Dir = "up" | "down" | "left" | "right";

export type Box = { left: number; top: number; right: number; bottom: number };

/** Sideways distance costs this much more than distance along the direction. */
const SIDEWAYS = 3;

/** The cost of going from `from` to `to` in `dir`; `null` if it is not that way at all. */
export function cost(from: Box, to: Box, dir: Dir): number | null {
  const fh = from.bottom - from.top;
  const th = to.bottom - to.top;
  const fw = from.right - from.left;
  const tw = to.right - to.left;
  // Beyond the edge it leaves by, give or take a quarter of the smaller of
  // the two: something on the same line is beside, not below.
  const slackY = Math.min(fh, th) * 0.25;
  const slackX = Math.min(fw, tw) * 0.25;
  let along: number;
  let lo1: number, hi1: number, lo2: number, hi2: number;
  switch (dir) {
    case "down":
      if (to.top < from.bottom - slackY) return null;
      along = Math.max(0, to.top - from.bottom);
      [lo1, hi1, lo2, hi2] = [from.left, from.right, to.left, to.right];
      break;
    case "up":
      if (to.bottom > from.top + slackY) return null;
      along = Math.max(0, from.top - to.bottom);
      [lo1, hi1, lo2, hi2] = [from.left, from.right, to.left, to.right];
      break;
    case "right":
      if (to.left < from.right - slackX) return null;
      along = Math.max(0, to.left - from.right);
      [lo1, hi1, lo2, hi2] = [from.top, from.bottom, to.top, to.bottom];
      break;
    case "left":
      if (to.right > from.left + slackX) return null;
      along = Math.max(0, from.left - to.right);
      [lo1, hi1, lo2, hi2] = [from.top, from.bottom, to.top, to.bottom];
      break;
  }
  const gap = Math.max(0, Math.max(lo1, lo2) - Math.min(hi1, hi2));
  // Among those in line, the one whose middle is nearest breaks ties.
  const offset = Math.abs((lo1 + hi1) / 2 - (lo2 + hi2) / 2);
  return along + gap * SIDEWAYS + offset * 0.01;
}

/** The index of the best candidate in `dir`, or -1. */
export function best(from: Box, candidates: readonly Box[], dir: Dir): number {
  let at = -1;
  let low = Infinity;
  candidates.forEach((c, i) => {
    const k = cost(from, c, dir);
    if (k !== null && k < low) {
      low = k;
      at = i;
    }
  });
  return at;
}

/** The candidates that are that way at all, cheapest first, as indices. */
export function ranked(from: Box, candidates: readonly Box[], dir: Dir): number[] {
  return candidates
    .map((c, i) => [cost(from, c, dir), i] as const)
    .filter((x): x is readonly [number, number] => x[0] !== null)
    .sort((a, b) => a[0] - b[0])
    .map(([, i]) => i);
}
