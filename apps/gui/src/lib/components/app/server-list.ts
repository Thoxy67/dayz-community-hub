/**
 * The columns of every server list (the browser, favourites, history), shared
 * by the headings and the rows so they line up and so the three lists read
 * the same. Written out in full: Tailwind only sees class names it can read.
 *
 *   star · ping · players · server · map & clock · mods · [extra] · actions
 *
 * The actions column is a fixed width that its buttons never exceed, so
 * nothing spills over the columns before it. Lists are size containers
 * (`@container`): below 720 px of their own width the map column goes and
 * the server's name keeps its room.
 */
export const LIST_GRID =
  "grid items-center gap-x-3 grid-cols-[1.5rem_4.75rem_6.25rem_minmax(0,1fr)_8.5rem_3.25rem_7.5rem] @max-[720px]:grid-cols-[1.5rem_4.75rem_6.25rem_minmax(0,1fr)_3.25rem_7.5rem]";

/** The same, with one more column before the actions (history: when you last played). */
export const LIST_GRID_EXTRA =
  "grid items-center gap-x-3 grid-cols-[1.5rem_4.75rem_6.25rem_minmax(0,1fr)_8.5rem_3.25rem_7.5rem_7.5rem] @max-[720px]:grid-cols-[1.5rem_4.75rem_6.25rem_minmax(0,1fr)_3.25rem_7.5rem_7.5rem]";

/** On a cell that gives way when the list is narrow. */
export const LIST_NARROW_HIDDEN = "@max-[720px]:hidden";

/** Pixels per row: two lines of text and the players bar. */
export const LIST_ROW_PX = 48;
