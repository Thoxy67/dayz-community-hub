/**
 * The arithmetic behind the browser's one long list: which blocks of rows to
 * ask the backend for around what is on screen, and which rows are far
 * enough away to let go of. Pure, so it is tested on its own.
 */

/** Rows per request. */
export const BLOCK = 100;
/** Blocks kept on each side of the view; rows beyond are dropped. */
export const KEEP_BLOCKS = 4;

/**
 * The blocks to hold for rows `first`..`last` (exclusive) of `total`: the
 * ones in view plus one on each side, never past the end. With nothing known
 * yet (`total` 0) the first block is still asked for, since it is what says
 * how many there are.
 */
export function blocksFor(first: number, last: number, total: number, size = BLOCK): number[] {
  const from = Math.max(0, Math.floor(first / size) - 1);
  const to = Math.floor(Math.max(first, last - 1) / size) + 1;
  const end = Math.max(1, Math.ceil(total / size));
  const out: number[] = [];
  for (let b = from; b <= to && b < end; b++) out.push(b);
  return out;
}

/** Whether row `index` lies outside the blocks worth keeping around the view. */
export function isFar(index: number, first: number, last: number, size = BLOCK, keep = KEEP_BLOCKS): boolean {
  return index < first - keep * size || index >= last + keep * size;
}
