<script lang="ts">
  import { dict, getLocale } from "$lib/i18n";
  import { duration } from "$lib/format";
  import type { DayStatDto } from "$lib/ipc/types";

  /**
   * A year of days, a column per week (Monday on top), each day as dark as
   * the time played on it, in four steps of one hue. Days are numbered from
   * 1970-01-01 in the player's own time, as the backend counts them.
   */
  let { days }: { days: readonly DayStatDto[] } = $props();
  const s = dict("stats");

  const WEEKS = 53;
  const byDay = $derived(new Map(days.map((d) => [d.day, d.secs])));
  const today = Math.floor((Date.now() / 1000 - new Date().getTimezoneOffset() * 60) / 86_400);
  // The Monday that starts the first column (1970-01-01 was a Thursday).
  const first = today - ((today + 3) % 7) - (WEEKS - 1) * 7;

  /** The thresholds between the four steps: quartiles of the days played. */
  const steps = $derived.by(() => {
    const v = days.map((d) => d.secs).filter((x) => x > 0);
    v.sort((a, b) => a - b);
    const q = (p: number) => v[Math.min(v.length - 1, Math.floor(p * v.length))] ?? 0;
    return [q(0.25), q(0.5), q(0.75)];
  });
  const level = (secs: number) =>
    secs <= 0 ? 0 : secs <= steps[0]! ? 1 : secs <= steps[1]! ? 2 : secs <= steps[2]! ? 3 : 4;
  const OPACITY = [0, 0.3, 0.5, 0.75, 1];

  const fmt = $derived(
    new Intl.DateTimeFormat(getLocale(), {
      weekday: "short",
      day: "numeric",
      month: "short",
      year: "numeric",
      timeZone: "UTC",
    }),
  );
  const monthFmt = $derived(
    new Intl.DateTimeFormat(getLocale(), { month: "short", timeZone: "UTC" }),
  );
  const dateOf = (day: number) => new Date(day * 86_400_000);
  /** A month's name over the column where it starts. */
  const monthLabel = (week: number) => {
    const d = dateOf(first + week * 7);
    return d.getUTCDate() <= 7 ? monthFmt.format(d) : "";
  };
</script>

<div class="flex flex-col gap-1.5">
  <div class="grid grid-cols-[repeat(53,minmax(0,1fr))] gap-[3px]">
    {#each { length: WEEKS } as _, w (w)}
      <span
        class="h-3 overflow-visible font-mono text-3xs leading-none whitespace-nowrap text-fg-faint"
        >{monthLabel(w)}</span
      >
    {/each}
  </div>
  <div
    class="grid grid-flow-col grid-cols-[repeat(53,minmax(0,1fr))] grid-rows-7 gap-[3px]"
    role="img"
    aria-label={$s.activityHint.value}
  >
    {#each { length: WEEKS * 7 } as _, i (i)}
      {@const day = first + i}
      {@const secs = byDay.get(day) ?? 0}
      {#if day <= today}
        <div
          class="relative aspect-square rounded-[2px] bg-raised {day === today
            ? 'ring-1 ring-fg'
            : ''}"
          title={$s.activityCell({
            date: fmt.format(dateOf(day)),
            duration: secs > 0 ? duration(secs) : "—",
          }).value}
        >
          <div
            class="absolute inset-0 rounded-[2px] bg-accent"
            style:opacity={OPACITY[level(secs)]}
          ></div>
        </div>
      {:else}
        <span></span>
      {/if}
    {/each}
  </div>
  <div class="flex items-center gap-3 text-3xs text-fg-faint">
    <span class="min-w-0 flex-1">{$s.activityHint.value}</span>
    <span class="flex shrink-0 items-center gap-1">
      {$s.legendLess.value}
      {#each OPACITY as o, i (i)}
        <span class="relative size-2.5 rounded-[2px] bg-raised">
          <span class="absolute inset-0 rounded-[2px] bg-accent" style:opacity={o}></span>
        </span>
      {/each}
      {$s.legendMore.value}
    </span>
  </div>
</div>
