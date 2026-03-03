/**
 * Generic filter cycler for three-state filters (both/option1/option2).
 * Reduces repetitive filter cycling functions in ServersTab.
 *
 * @example
 * const modFilter = createFilterCycler(
 *   ['both', 'mods-only', 'no-mods'] as const,
 *   (f) => {
 *     if (f === 'both') return m.servers_filter_mods_all();
 *     if (f === 'mods-only') return m.servers_filter_mods_only();
 *     return m.servers_filter_mods_none();
 *   },
 *   (f) => {
 *     if (f === 'both') return m.servers_filter_mods_title_all();
 *     if (f === 'mods-only') return m.servers_filter_mods_title_only();
 *     return m.servers_filter_mods_title_none();
 *   }
 * );
 */
export function createFilterCycler<T extends string>(
  values: readonly T[],
  getLabel: (value: T) => string,
  getTitle: (value: T) => string,
) {
  return {
    cycle: (current: T): T => {
      const idx = values.indexOf(current);
      return values[(idx + 1) % values.length] as T;
    },
    getLabel,
    getTitle,
  };
}
