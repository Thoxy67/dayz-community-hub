<script lang="ts">
  import { dict, getLocale } from "$lib/i18n";
  import { cn } from "$lib/cx";
  import ChartTip from "./ChartTip.svelte";

  /**
   * A week of hours, Monday first, each cell as bright as its value against
   * the week's highest: when a server is busy, when you play. One hue, light
   * to dark; the hour it is now is ringed. `values[day][hour]` is already in
   * the player's own time (day 0 is Monday). `cell` names a cell for its
   * tooltip; `hint` says what the grid shows.
   *
   * The pointer or the keyboard (focus the grid, then the arrows) picks a
   * cell: it is outlined, its day and hour light up on the axes, and its
   * tooltip shows.
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
  const cm = dict("common");

  const top = $derived(Math.max(1, ...values.flat()));
  const dayName = $derived.by(() => {
    const f = new Intl.DateTimeFormat(getLocale(), { weekday: "short", timeZone: "UTC" });
    // 2 January 2023 was a Monday.
    return (day: number) => f.format(new Date(Date.UTC(2023, 0, 2 + day)));
  });
  const now = new Date();
  // JavaScript's Sunday is 0; here Monday is.
  const nowDay = (now.getDay() + 6) % 7;
  const nowHour = now.getHours();

  let active = $state<{ day: number; hour: number } | null>(null);
  let anchor = $state<DOMRect | null>(null);
  let grid: HTMLDivElement | undefined = $state();

  function pick(day: number, hour: number) {
    active = { day, hour };
    anchor =
      grid?.querySelector<HTMLElement>(`[data-cell="${day}-${hour}"]`)?.getBoundingClientRect() ??
      null;
  }
  function clear() {
    active = null;
    anchor = null;
  }

  function onkeydown(e: KeyboardEvent) {
    const a = active ?? { day: nowDay, hour: nowHour };
    const moves: Record<string, [number, number]> = {
      ArrowUp: [-1, 0],
      ArrowDown: [1, 0],
      ArrowLeft: [0, -1],
      ArrowRight: [0, 1],
      Home: [0, -24],
      End: [0, 24],
    };
    const m = moves[e.key];
    if (!m) return;
    e.preventDefault();
    pick(Math.max(0, Math.min(6, a.day + m[0])), Math.max(0, Math.min(23, a.hour + m[1])));
  }

  const tip = $derived(
    active ? cell(dayName(active.day), active.hour, values[active.day]?.[active.hour] ?? 0) : "",
  );
</script>

<div class="flex flex-col gap-1">
  <!-- The keyboard lives on the grid (arrows, Enter), which announces the
       picked cell; the cells only answer the pointer. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={grid}
    class="grid grid-cols-[auto_repeat(24,minmax(0,1fr))] gap-px rounded-xs outline-none focus-visible:ring-2 focus-visible:ring-accent/60 focus-visible:ring-offset-2 focus-visible:ring-offset-panel"
    role="application"
    tabindex="0"
    aria-label={`${hint} ${$cm.chartKeys.value}`}
    aria-roledescription="heatmap"
    {onkeydown}
    onfocus={() => !active && pick(nowDay, nowHour)}
    onblur={clear}
    onpointerleave={clear}
  >
    {#each { length: 7 } as _, day (day)}
      <span
        class={cn(
          "pr-1.5 font-mono text-3xs leading-none transition-colors",
          active?.day === day ? "font-semibold text-fg" : "text-fg-faint",
        )}>{dayName(day)}</span
      >
      {#each { length: 24 } as _, hour (hour)}
        {@const v = values[day]?.[hour] ?? 0}
        {@const on = active?.day === day && active?.hour === hour}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          data-cell="{day}-{hour}"
          class={cn(
            "relative h-3 rounded-xs bg-raised",
            day === nowDay && hour === nowHour && "ring-1 ring-fg",
            on && "z-10 ring-2 ring-fg",
            active && !on && (active.day === day || active.hour === hour) && "brightness-95",
          )}
          onpointerenter={() => pick(day, hour)}
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
      <span
        class={cn(
          "font-mono text-3xs leading-none transition-colors",
          active?.hour === hour ? "font-semibold text-fg" : "text-fg-faint",
        )}
      >
        {hour % 6 === 0 || active?.hour === hour ? hour : ""}
      </span>
    {/each}
  </div>
  <p class="m-0 text-3xs text-fg-faint">{hint}</p>
  <span class="sr-only" aria-live="polite">{tip}</span>
</div>

<ChartTip {anchor}>{tip}</ChartTip>
