/**
 * Generic sorting state management for table components.
 * Eliminates duplicate sorting logic across FavoritesTab, HistoryTab, and ServersTab.
 *
 * @example
 * const sort = createSortState<'name' | 'players' | 'ping'>('name', true);
 *
 * // In template:
 * <th onclick={() => sort.toggle('name')}>
 *   Name {sort.icon('name')}
 * </th>
 *
 * // For sorted array:
 * let sorted = $derived(sort.sortArray(items, (item, col, asc) => {
 *   // return comparison logic
 * }));
 */
export function createSortState<T extends string>(initialCol: T, initialAsc: boolean, defaultAscCols?: T[]) {
  let sortCol = $state<T>(initialCol);
  let sortAsc = $state(initialAsc);

  return {
    get col() {
      return sortCol;
    },
    get asc() {
      return sortAsc;
    },
    toggle(col: T) {
      if (sortCol === col) {
        sortAsc = !sortAsc;
      } else {
        sortCol = col;
        // Default to ascending for name-like columns, descending otherwise
        sortAsc = defaultAscCols?.includes(col) ?? col === ('name' as any);
      }
    },
    icon(col: T): string {
      if (sortCol !== col) return '↕';
      return sortAsc ? '↑' : '↓';
    },
    /**
     * Sort an array using the current sort state.
     * @param items Array to sort
     * @param compareFn Comparison function: (item, col, dir) => sortValue
     */
    sortArray<U>(items: U[], compareFn: (a: U, b: U, col: T, dir: 1 | -1) => number): U[] {
      const arr = items.slice();
      const dir = sortAsc ? 1 : -1;
      arr.sort((a, b) => compareFn(a, b, sortCol, dir as 1 | -1));
      return arr;
    },
  };
}
