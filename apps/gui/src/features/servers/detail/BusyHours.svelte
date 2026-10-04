<script lang="ts">
  import { dict, getLocale } from "$lib/i18n";
  import type { HeatCell } from "$lib/ipc/types";

  /**
   * When a server is full: a week of hours, each cell as bright as the
   * average count then. The site sends UTC; the grid is drawn in the
   * player's own time, Monday first, with the hour it is now ringed.
   */
  let { cells }: { cells: HeatCell[] } = $props();
  const c = dict("detail");

  const mod = (n: number, m: number) => ((n % m) + m) % m;
  /** Rows top to bottom as JavaScript numbers weekdays (0 is Sunday). */
  const ROWS = [1, 2, 3, 4, 5, 6, 0];

  const grid = $derived.by(() => {
    const shift = Math.round(-new Date().getTimezoneOffset() / 60);
    const g = new Map<number, number>();
    for (const cell of cells) {
      const h = cell.hour + shift;
      const dow = mod(cell.dow + Math.floor(h / 24), 7);
      g.set(dow * 24 + mod(h, 24), cell.avg ?? 0);
    }
    return g;
  });
  const top = $derived(Math.max(1, ...grid.values()));

  const dayName = $derived.by(() => {
    const f = new Intl.DateTimeFormat(getLocale(), { weekday: "short", timeZone: "UTC" });
    // 1 January 2023 was a Sunday.
    return (dow: number) => f.format(new Date(Date.UTC(2023, 0, 1 + dow)));
  });
  const now = new Date();
  const nowKey = now.getDay() * 24 + now.getHours();
</script>

<div class="flex flex-col gap-1">
  <div
    class="grid grid-cols-[auto_repeat(24,minmax(0,1fr))] gap-px"
    role="img"
    aria-label={$c.dmBusyHint.value}
  >
    {#each ROWS as dow (dow)}
      <span class="pr-1.5 font-mono text-3xs leading-none text-fg-faint">{dayName(dow)}</span>
      {#each { length: 24 } as _, hour (hour)}
        {@const avg = grid.get(dow * 24 + hour) ?? 0}
        <div
          class="relative h-3 rounded-xs bg-raised {dow * 24 + hour === nowKey
            ? 'ring-1 ring-fg'
            : ''}"
          title={$c.dmBusyCell({ day: dayName(dow), hour, count: Math.round(avg) }).value}
        >
          <div
            class="absolute inset-0 rounded-xs bg-accent"
            style:opacity={avg > 0 ? 0.12 + 0.88 * (avg / top) : 0}
          ></div>
        </div>
      {/each}
    {/each}
    <span></span>
    {#each { length: 24 } as _, hour (hour)}
      <span class="font-mono text-3xs leading-none text-fg-faint">
        {hour % 6 === 0 ? hour : ""}
      </span>
    {/each}
  </div>
  <p class="m-0 text-3xs text-fg-faint">{$c.dmBusyHint.value}</p>
</div>
