<script lang="ts">
  import { getLocale } from "$lib/i18n";

  /**
   * A week of hours, Monday first, each cell as bright as its value against
   * the week's highest: when a server is busy, when you play. One hue, light
   * to dark; the hour it is now is ringed. `values[day][hour]` is already in
   * the player's own time (day 0 is Monday). `cell` names a cell for its
   * tooltip; `hint` says what the grid shows.
   */
  let {
    values,
    cell,
    hint,
  }: {
    values: readonly (readonly number[])[];
    cell: (day: string, hour: number, value: number) => string;
    hint: string;
  } = $props();

  const top = $derived(Math.max(1, ...values.flat()));
  const dayName = $derived.by(() => {
    const f = new Intl.DateTimeFormat(getLocale(), { weekday: "short", timeZone: "UTC" });
    // 2 January 2023 was a Monday.
    return (day: number) => f.format(new Date(Date.UTC(2023, 0, 2 + day)));
  });
  const now = new Date();
  // JavaScript's Sunday is 0; here Monday is.
  const nowKey = ((now.getDay() + 6) % 7) * 24 + now.getHours();
</script>

<div class="flex flex-col gap-1">
  <div class="grid grid-cols-[auto_repeat(24,minmax(0,1fr))] gap-px" role="img" aria-label={hint}>
    {#each { length: 7 } as _, day (day)}
      <span class="pr-1.5 font-mono text-3xs leading-none text-fg-faint">{dayName(day)}</span>
      {#each { length: 24 } as _, hour (hour)}
        {@const v = values[day]?.[hour] ?? 0}
        <div
          class="relative h-3 rounded-xs bg-raised {day * 24 + hour === nowKey
            ? 'ring-1 ring-fg'
            : ''}"
          title={cell(dayName(day), hour, v)}
        >
          <div
            class="absolute inset-0 rounded-xs bg-accent"
            style:opacity={v > 0 ? 0.12 + 0.88 * (v / top) : 0}
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
  <p class="m-0 text-3xs text-fg-faint">{hint}</p>
</div>
