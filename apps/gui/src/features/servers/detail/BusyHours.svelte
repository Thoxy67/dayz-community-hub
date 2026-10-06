<script lang="ts">
  import { dict } from "$lib/i18n";
  import { WeekHeat } from "$lib/components/app";
  import type { HeatCell } from "$lib/ipc/types";

  /**
   * When a server is full: a week of hours, each cell as bright as the
   * average count then. The site sends UTC; the grid is drawn in the
   * player's own time.
   */
  let { cells }: { cells: HeatCell[] } = $props();
  const c = dict("detail");

  const mod = (n: number, m: number) => ((n % m) + m) % m;

  /** `[day][hour]`, Monday first, in the player's time. */
  const values = $derived.by(() => {
    const shift = Math.round(-new Date().getTimezoneOffset() / 60);
    const g = Array.from({ length: 7 }, () => Array<number>(24).fill(0));
    for (const cell of cells) {
      const h = cell.hour + shift;
      // The site numbers weekdays as JavaScript does (0 is Sunday).
      const dow = mod(cell.dow + Math.floor(h / 24), 7);
      g[mod(dow - 1, 7)]![mod(h, 24)] = cell.avg ?? 0;
    }
    return g;
  });
</script>

<WeekHeat
  {values}
  hint={$c.dmBusyHint.value}
  cell={(day, hour, v) => $c.dmBusyCell({ day, hour, count: Math.round(v) }).value}
/>
