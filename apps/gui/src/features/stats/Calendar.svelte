<script lang="ts">
  import { dict, getLocale } from "$lib/i18n";
  import ChevronLeft from "~icons/lucide/chevron-left";
  import ChevronRight from "~icons/lucide/chevron-right";
  import { ChartTip } from "$lib/components/app";
  import { IconButton } from "$lib/components/ui/button";
  import { cn } from "$lib/cx";
  import { duration } from "$lib/format";
  import type { DayStatDto } from "$lib/ipc/types";

  /**
   * A year of days, a column per week (Monday on top), each day as dark as
   * the time played on it, in four steps of one hue. Days are numbered from
   * 1970-01-01 in the player's own time, as the backend counts them.
   *
   * The pointer or the keyboard (focus it; ←/→ a week, ↑/↓ a day) picks a
   * day and shows what was played; a click or Enter hands it to `onpick`.
   * ‹ › move the year shown, back to the first session.
   */
  let {
    days,
    first,
    onpick,
  }: {
    days: readonly DayStatDto[];
    /** The first session, Unix seconds: how far back the year can go. */
    first: number | null;
    onpick: (day: number) => void;
  } = $props();
  const s = dict("stats");
  const cm = dict("common");

  const WEEKS = 53;
  const byDay = $derived(new Map(days.map((d) => [d.day, d])));
  const offset = -new Date().getTimezoneOffset() * 60;
  const today = Math.floor((Date.now() / 1000 + offset) / 86_400);
  const firstDay = $derived(first == null ? today : Math.floor((first + offset) / 86_400));
  /** The last day shown: today, or a year back per ‹. */
  let end = $state(today);
  // The Monday that starts the first column (1970-01-01 was a Thursday).
  const start = $derived(end - ((end + 3) % 7) - (WEEKS - 1) * 7);

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
      weekday: "long",
      day: "numeric",
      month: "long",
      year: "numeric",
      timeZone: "UTC",
    }),
  );
  const monthFmt = $derived(
    new Intl.DateTimeFormat(getLocale(), { month: "short", timeZone: "UTC" }),
  );
  const spanFmt = $derived(
    new Intl.DateTimeFormat(getLocale(), { month: "short", year: "numeric", timeZone: "UTC" }),
  );
  const dateOf = (day: number) => new Date(day * 86_400_000);
  /** A month's name over the column where it starts. */
  const monthLabel = (week: number) => {
    const d = dateOf(start + week * 7);
    return d.getUTCDate() <= 7 ? monthFmt.format(d) : "";
  };

  // ── the day picked by the pointer or the keyboard ──────────────────────
  let active = $state<number | null>(null);
  let anchor = $state<DOMRect | null>(null);
  let grid: HTMLDivElement | undefined = $state();

  function pick(day: number) {
    active = day;
    queueMicrotask(() => {
      anchor =
        grid?.querySelector<HTMLElement>(`[data-day="${day}"]`)?.getBoundingClientRect() ?? null;
    });
  }
  function clear() {
    active = null;
    anchor = null;
  }
  function shift(by: number) {
    end = Math.min(today, Math.max(firstDay + WEEKS * 7 - 7, end + by));
    clear();
  }

  function onkeydown(e: KeyboardEvent) {
    const moves: Record<string, number> = {
      ArrowLeft: -7,
      ArrowRight: 7,
      ArrowUp: -1,
      ArrowDown: 1,
    };
    if (e.key === "Enter" && active !== null) {
      e.preventDefault();
      onpick(active);
      return;
    }
    const m = moves[e.key];
    if (m === undefined) return;
    e.preventDefault();
    const next = Math.min(today, (active ?? today) + m);
    // Off the edge: the year moves with it.
    if (next < start) end = Math.max(end - WEEKS * 7 + 7, next + (WEEKS - 1) * 7);
    pick(next);
  }

  const tip = $derived.by(() => {
    if (active === null) return null;
    const d = byDay.get(active);
    return {
      date: fmt.format(dateOf(active)),
      time: d?.secs ? duration(d.secs) : "—",
      sessions: d?.sessions ?? 0,
    };
  });
</script>

<div class="flex flex-col gap-1.5">
  <div class="flex items-center gap-1">
    <IconButton
      icon={ChevronLeft}
      size="icon-xs"
      label={$s.prevYear.value}
      disabled={start <= firstDay}
      onclick={() => shift(-WEEKS * 7 + 7)}
    />
    <span class="min-w-32 text-center font-mono text-2xs text-fg-muted">
      {spanFmt.format(dateOf(start))} – {spanFmt.format(dateOf(end))}
    </span>
    <IconButton
      icon={ChevronRight}
      size="icon-xs"
      label={$s.nextYear.value}
      disabled={end >= today}
      onclick={() => shift(WEEKS * 7 - 7)}
    />
  </div>
  <div class="grid grid-cols-[repeat(53,minmax(0,1fr))] gap-[3px]">
    {#each { length: WEEKS } as _, w (w)}
      <span
        class="h-3 overflow-visible font-mono text-3xs leading-none whitespace-nowrap text-fg-faint"
        >{monthLabel(w)}</span
      >
    {/each}
  </div>
  <!-- The keyboard lives on the grid (arrows, Enter), which announces the
       picked cell; the cells only answer the pointer. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    bind:this={grid}
    class="grid grid-flow-col grid-cols-[repeat(53,minmax(0,1fr))] grid-rows-7 gap-[3px] rounded-xs outline-none focus-visible:ring-2 focus-visible:ring-accent/60 focus-visible:ring-offset-2 focus-visible:ring-offset-panel"
    role="application"
    tabindex="0"
    aria-label={`${$s.activityHint.value} ${$cm.chartKeys.value}`}
    aria-roledescription="calendar heatmap"
    {onkeydown}
    onfocus={() => active === null && pick(Math.min(today, end))}
    onblur={clear}
    onpointerleave={clear}
  >
    {#each { length: WEEKS * 7 } as _, i (i)}
      {@const day = start + i}
      {@const secs = byDay.get(day)?.secs ?? 0}
      {#if day <= today}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div
          data-day={day}
          class={cn(
            "relative aspect-square cursor-pointer rounded-[2px] bg-raised",
            day === today && "ring-1 ring-fg",
            active === day && "z-10 ring-2 ring-fg",
          )}
          onpointerenter={() => pick(day)}
          onclick={() => onpick(day)}
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
  <span class="sr-only" aria-live="polite">{tip ? `${tip.date}, ${tip.time}` : ""}</span>
</div>

<ChartTip {anchor}>
  {#if tip}
    <span class="font-semibold first-letter:uppercase">{tip.date}</span>
    <span class="flex gap-2">
      <span class="num font-mono text-accent">{tip.time}</span>
      {#if tip.sessions}
        <span class="text-fg-muted"
          >{tip.sessions === 1
            ? $s.placeSessionsOne.value
            : $s.placeSessions({ count: tip.sessions }).value}</span
        >
      {/if}
    </span>
  {/if}
</ChartTip>
