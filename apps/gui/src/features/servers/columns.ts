/**
 * The browser's columns, shared by the header and every row so they line up.
 * The list is a size container (`@container` on its pane): below 820 px of
 * its own width the clock and OS columns go (cells marked `NARROW_HIDDEN`),
 * so the server's name keeps its room.
 */
export const GRID =
  "grid grid-cols-[2.25rem_1.5rem_5rem_6.5rem_minmax(0,1fr)_7.5rem_4rem_3rem_1.5rem] @max-[820px]:grid-cols-[2.25rem_1.5rem_5rem_6.5rem_minmax(0,1fr)_6.5rem_3rem] items-center gap-x-2";

/** On a cell that gives way when the list is narrow. */
export const NARROW_HIDDEN = "@max-[820px]:hidden";

/** Pixels per row: two lines of text and the players bar. */
export const ROW_PX = 46;
