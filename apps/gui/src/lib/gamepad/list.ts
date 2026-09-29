/**
 * A virtual list, as the pad sees it: rows by index, not by element.
 *
 * Only the rows on screen exist, so moving through a list of nine thousand
 * servers cannot be "focus the next element below": there is none past the
 * last mounted row. A list marks its scroller with `use:padList` and each row
 * with `data-pad-index={i}` (and `tabindex="-1"`); Up and Down inside it then
 * ask the list to bring row i±1 into view and focus it once it is drawn.
 * Left and Right, or Up past the first row, leave by the spatial rules.
 */
import type { Action } from "svelte/action";

export type PadList = {
  /** How many rows there are in all, mounted or not. */
  count: () => number;
  /** Scroll so row `i` is in view. */
  reveal: (i: number) => void;
  /** The row to go to when the list is entered with no row remembered (the selection). */
  current?: () => number;
  /** Rows in one page, for the triggers. */
  page?: () => number;
};

const lists = new WeakMap<Element, PadList>();
/** The row each list last had focused, to come back to it. */
const remembered = new WeakMap<Element, number>();

export const padList: Action<HTMLElement, PadList> = (node, api) => {
  lists.set(node, api);
  node.setAttribute("data-pad-list", "");
  return {
    update(next: PadList) {
      lists.set(node, next);
    },
    destroy() {
      lists.delete(node);
    },
  };
};

export const listOf = (el: Element | null) => {
  const node = el?.closest("[data-pad-list]") ?? null;
  const api = node ? lists.get(node) : undefined;
  return node && api ? { node, api } : null;
};

export const indexOf = (el: Element | null): number | null => {
  const row = el?.closest<HTMLElement>("[data-pad-index]");
  const i = row ? Number(row.dataset.padIndex) : NaN;
  return Number.isNaN(i) ? null : i;
};

export const rememberRow = (list: Element, i: number) => remembered.set(list, i);
export const rememberedRow = (list: Element) => remembered.get(list);

export const rowEl = (list: Element, i: number) =>
  list.querySelector<HTMLElement>(`[data-pad-index="${i}"]`);
