<script lang="ts">
  import { dict } from "$lib/i18n";
  import ChartColumn from "~icons/lucide/chart-no-axes-column";
  import CalendarDays from "~icons/lucide/calendar-days";
  import MapPin from "~icons/lucide/map-pin";
  import Clock from "~icons/lucide/clock";
  import MapIcon from "~icons/lucide/map";
  import History from "~icons/lucide/history";
  import FileDown from "~icons/lucide/file-down";
  import FileUp from "~icons/lucide/file-up";
  import Play from "~icons/lucide/play";
  import Trash from "~icons/lucide/trash-2";
  import Radio from "~icons/lucide/radio";
  import {
    Empty,
    Figure,
    JoinButton,
    PageHeader,
    Section,
    WeekHeat,
    mapName,
  } from "$lib/components/app";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Segmented } from "$lib/components/ui/segmented";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Tag } from "$lib/components/ui/tag";
  import { cn } from "$lib/cx";
  import { dateTime, duration, relative } from "$lib/format";
  import type { PlaceStatDto, SessionDto, StatsRange } from "$lib/ipc/types";
  import { app } from "$lib/stores/app.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { describe, offline } from "$features/offline/offline.svelte";
  import Calendar from "./Calendar.svelte";
  import { stats as play } from "./stats.svelte";

  /**
   * How much you play, where and when: totals for a period, a year of days,
   * the servers and maps played most, the hours of the week, and every
   * session ever. The backend works it all out of the session log; the
   * play history is part of the profile, so exporting the profile keeps it.
   */
  const s = dict("stats");
  const n = dict("nav");

  // Asked when the view first shows, and again on every change after that.
  let started = false;
  $effect(() => {
    if (app.view !== "stats" || started) return;
    started = true;
    play.listen();
    void play.load();
    void play.loadHistory(false);
  });

  // A session running: its length moves on between two answers.
  let now = $state(Date.now());
  $effect(() => {
    if (!play.data?.current) return;
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });
  const current = $derived(play.data?.current ?? null);
  const currentSecs = $derived(current ? Math.max(current.secs, now / 1000 - current.start) : 0);

  const RANGES: StatsRange[] = ["week", "month", "year", "all"];
  const rangeLabel = (r: StatsRange) =>
    ({
      week: $s.rangeWeek.value,
      month: $s.rangeMonth.value,
      year: $s.rangeYear.value,
      all: $s.rangeAll.value,
    })[r];

  /** A place's name as the player knows it. */
  const nameOf = (p: { kind: string; name: string }) =>
    p.kind === "offline"
      ? describe(p.name).map
      : p.kind === "unknown"
        ? $s.unknownPlace.value
        : p.name;

  const PLACES_SHOWN = 10;
  let allPlaces = $state(false);
  const places = $derived(play.data?.places_played ?? []);
  const shownPlaces = $derived(allPlaces ? places : places.slice(0, PLACES_SHOWN));
  const topPlace = $derived(Math.max(1, ...places.map((p) => p.secs)));
  const maps = $derived(play.data?.maps ?? []);
  const topMap = $derived(Math.max(1, ...maps.map((m) => m.secs)));
  const empty = $derived(play.data !== null && (play.data.first ?? null) === null);
</script>

{#snippet kindTag(p: { kind: string })}
  {#if p.kind === "offline"}<Tag tone="accent">{$s.offlineTag.value}</Tag>{/if}
{/snippet}

{#snippet bar(value: number, top: number)}
  <!-- One hue, a thin bar from the start, rounded at its end. -->
  <span class="h-1.5 min-w-0 flex-1 overflow-hidden rounded-full bg-raised">
    <span
      class="block h-full rounded-full bg-accent"
      style:width="{Math.max(2, (value / top) * 100)}%"
    ></span>
  </span>
{/snippet}

{#snippet placeAction(p: PlaceStatDto | SessionDto)}
  {#if p.kind === "server" && p.ip && p.port}
    <JoinButton ip={p.ip} port={p.port} compact quiet />
  {:else if p.kind === "offline"}
    <IconButton
      icon={Play}
      class="border-accent/45 bg-transparent text-accent group-hover:border-accent group-hover:bg-accent group-hover:text-accent-fg"
      variant="default"
      label={describe(p.name).map}
      onclick={() => offline.launch(p.name)}
    />
  {/if}
{/snippet}

<div class="flex h-full min-h-0 flex-col">
  <PageHeader title={$n.stats.value}>
    {#snippet stats()}
      {#if play.data}
        {@const d = play.data}
        <Figure label={$s.figTime.value} value={duration(d.total_secs)} tone="text-accent" />
        <Figure label={$s.figSessions.value} value={String(d.sessions)} />
        <Figure label={$s.figPlaces.value} value={String(d.places)} />
        <Figure
          secondary
          label={$s.figAverage.value}
          value={d.average_secs ? duration(d.average_secs) : "—"}
        />
        <Figure
          secondary
          label={$s.figLongest.value}
          value={d.longest ? duration(d.longest.secs) : "—"}
          title={d.longest ? `${nameOf(d.longest)} · ${dateTime(d.longest.start)}` : ""}
        />
        <Figure
          label={$s.figStreak.value}
          value={$s.streakDays({ count: d.streak }).value}
          tone={d.streak > 0 ? "text-ok" : "text-fg-muted"}
          title={$s.streakTitle({ best: $s.streakDays({ count: d.best_streak }).value }).value}
        />
      {/if}
    {/snippet}
    {#snippet actions()}
      <Button onclick={() => profile.exportTo(false)} title={$s.exportHint.value}>
        <FileDown class="size-icon-sm" />{$s.exportProfile.value}
      </Button>
      <Button onclick={() => profile.importFrom()} title={$s.exportHint.value}>
        <FileUp class="size-icon-sm" />{$s.importProfile.value}
      </Button>
    {/snippet}
    <div class="flex items-center gap-2">
      <Segmented
        aria-label={$s.rangeLabel.value}
        value={play.range}
        onchange={(r) => play.setRange(r)}
        options={RANGES.map((r) => ({ value: r, label: rangeLabel(r) }))}
      />
      {#if current}
        <span
          class="ml-auto flex min-w-0 items-center gap-1.5 rounded-sm border border-ok/40 bg-ok/10 px-2 py-1 text-2xs text-ok"
        >
          <Radio class="size-icon-sm shrink-0 animate-pulse" />
          <span class="truncate"
            >{$s.playingNow({ name: nameOf(current), duration: duration(currentSecs) }).value}</span
          >
        </span>
      {/if}
    </div>
  </PageHeader>

  <div class="min-h-0 flex-1 overflow-y-auto">
    {#if play.loading}
      <div class="grid h-full place-items-center"><Spinner class="size-6 text-accent" /></div>
    {:else if play.error}
      <Empty icon={ChartColumn} title={$s.emptyTitle.value}>
        <span class="font-mono text-2xs" data-selectable>{play.error}</span>
      </Empty>
    {:else if empty}
      <Empty icon={ChartColumn} title={$s.emptyTitle.value}>{$s.emptyHint.value}</Empty>
    {:else if play.data}
      {@const d = play.data}
      <div class="flex flex-col gap-px bg-border">
        <Section title={$s.sectionActivity.value} icon={CalendarDays} class="bg-panel">
          <div class="max-w-[64rem]"><Calendar days={d.days} /></div>
        </Section>

        <div class="grid grid-cols-[minmax(0,1.6fr)_minmax(0,1fr)] gap-px max-[1100px]:grid-cols-1">
          <Section title={$s.sectionPlaces.value} icon={MapPin} class="bg-panel">
            {#if places.length === 0}
              <p class="m-0 text-2xs text-fg-faint">{$s.nothingInRange.value}</p>
            {:else}
              <ol class="m-0 flex list-none flex-col p-0">
                {#each shownPlaces as p, i (`${p.kind}:${p.ip}:${p.port}:${p.name}`)}
                  <li
                    class="group grid grid-cols-[1.5rem_minmax(0,1fr)_minmax(6rem,14rem)_5rem_2rem] items-center gap-x-3 border-b border-border/40 py-1.5 last:border-b-0"
                  >
                    <span class="num text-right font-mono text-2xs text-fg-faint">{i + 1}</span>
                    <span class="flex min-w-0 flex-col gap-0.5">
                      <span class="flex min-w-0 items-center gap-1.5">
                        <span class="truncate text-xs font-semibold text-fg" title={p.name}
                          >{nameOf(p)}</span
                        >
                        {@render kindTag(p)}
                      </span>
                      <span class="flex min-w-0 gap-2 text-3xs text-fg-faint">
                        {#if p.map}<span class="truncate text-map">{mapName(p.map)}</span>{/if}
                        <span class="shrink-0"
                          >{p.sessions === 1
                            ? $s.placeSessionsOne.value
                            : $s.placeSessions({ count: p.sessions }).value}</span
                        >
                        <span class="shrink-0">· {relative(p.last)}</span>
                      </span>
                    </span>
                    <span class="flex items-center">{@render bar(p.secs, topPlace)}</span>
                    <span
                      class="num text-right font-mono text-2xs text-fg"
                      title={p.unmeasured ? $s.unmeasuredHint.value : undefined}
                      >{p.secs ? duration(p.secs) : "—"}</span
                    >
                    <span class="flex justify-end">{@render placeAction(p)}</span>
                  </li>
                {/each}
              </ol>
              {#if places.length > PLACES_SHOWN}
                <button
                  class="mt-1 self-start rounded-xs px-1 text-2xs font-medium text-accent hover:bg-accent/10"
                  onclick={() => (allPlaces = !allPlaces)}
                >
                  {allPlaces
                    ? $s.showFewerPlaces.value
                    : $s.showAllPlaces({ count: places.length }).value}
                </button>
              {/if}
            {/if}
          </Section>

          <div class="flex flex-col gap-px">
            <Section title={$s.sectionWhen.value} icon={Clock} class="bg-panel">
              <WeekHeat
                values={d.week_hours}
                hint={$s.whenHint.value}
                cell={(day, hour, v) =>
                  $s.whenCell({ day, hour, duration: v > 0 ? duration(v) : "—" }).value}
              />
            </Section>
            <Section title={$s.sectionMaps.value} icon={MapIcon} class="flex-1 bg-panel">
              {#if maps.length === 0}
                <p class="m-0 text-2xs text-fg-faint">{$s.nothingInRange.value}</p>
              {:else}
                <ul class="m-0 flex list-none flex-col gap-1.5 p-0">
                  {#each maps as m (m.map)}
                    <li class="grid grid-cols-[7rem_minmax(0,1fr)_4.5rem] items-center gap-2">
                      <span class="truncate text-xs text-map">{mapName(m.map)}</span>
                      {@render bar(m.secs, topMap)}
                      <span class="num text-right font-mono text-2xs text-fg"
                        >{m.secs ? duration(m.secs) : "—"}</span
                      >
                    </li>
                  {/each}
                </ul>
              {/if}
            </Section>
          </div>
        </div>

        <Section title={$s.sectionHistory.value} icon={History} class="bg-panel">
          {#snippet actions()}
            <span class="num font-mono text-2xs text-fg-faint"
              >{$s.historyCount({ shown: play.rows.length, total: play.total }).value}</span
            >
          {/snippet}
          <Input
            type="search"
            class="mb-2 max-w-96"
            placeholder={$s.historySearch.value}
            bind:value={() => play.search, (v) => play.setSearch(String(v ?? ""))}
          />
          <ul class="m-0 flex list-none flex-col p-0">
            {#each play.rows as r (`${r.start}:${r.name}`)}
              <li
                class="group grid grid-cols-[9.5rem_minmax(0,1fr)_7rem_8.5rem_4.5rem] items-center gap-x-3 border-b border-border/40 py-1 text-xs last:border-b-0"
              >
                <span class="font-mono text-2xs text-fg-muted" title={relative(r.start)}
                  >{dateTime(r.start)}</span
                >
                <span class="flex min-w-0 items-center gap-1.5">
                  <span class="truncate text-fg" title={r.name}>{nameOf(r)}</span>
                  {@render kindTag(r)}
                </span>
                <span class="truncate text-2xs text-map">{r.map ? mapName(r.map) : ""}</span>
                <span
                  class={cn(
                    "num text-right font-mono text-2xs",
                    r.open ? "text-ok" : r.measured ? "text-fg" : "text-fg-faint",
                  )}
                  title={r.measured ? undefined : $s.unmeasuredHint.value}
                >
                  {r.open
                    ? `${duration(r.secs)} · ${$s.running.value}`
                    : r.measured
                      ? duration(r.secs)
                      : $s.unmeasured.value}
                </span>
                <span class="flex items-center justify-end gap-1">
                  <span class="opacity-0 group-hover:opacity-100 focus-within:opacity-100">
                    {#if !r.open}
                      <IconButton
                        icon={Trash}
                        size="icon-xs"
                        hoverTone="danger"
                        label={$s.deleteSession.value}
                        onclick={() => play.forget(r)}
                      />
                    {/if}
                  </span>
                  {@render placeAction(r)}
                </span>
              </li>
            {/each}
          </ul>
          {#if play.nextPage > 0}
            <Button class="mt-2" onclick={() => play.loadHistory(true)}>
              {$s.showMore({ count: play.nextPage }).value}
            </Button>
          {/if}
        </Section>
      </div>
    {/if}
  </div>
</div>
