<script lang="ts">
  import { dict, getLocale } from "$lib/i18n";
  import ChartLine from "~icons/lucide/chart-line";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import ExternalLink from "~icons/lucide/external-link";
  import CalendarClock from "~icons/lucide/calendar-clock";
  import Link from "~icons/lucide/link";
  import Megaphone from "~icons/lucide/megaphone";
  import Info from "~icons/lucide/info";
  import TrendingUp from "~icons/lucide/trending-up";
  import TrendingDown from "~icons/lucide/trending-down";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Tag } from "$lib/components/ui/tag";
  import { Flag } from "$lib/components/ui/flag";
  import { Empty, Facts, PlayerChart, Section, type Fact } from "$lib/components/app";
  import { serverData } from "$lib/stores/server-data.svelte";
  import { openUrl } from "$lib/ipc/native";
  import { bytes, date, dateTime, num } from "$lib/format";
  import type { DetailModel } from "./model.svelte";
  import PopulationWarning from "./PopulationWarning.svelte";
  import { untilText } from "./until";

  /**
   * The server's long view, from DayZ Metrics (no key needed): how it ranks,
   * how reliably it is up, how full it gets, when it restarts and wipes,
   * whether its player count can be trusted, and a day of players drawn
   * large.
   */
  let { m }: { m: DetailModel } = $props();
  const c = dict("detail");

  const entry = $derived(m.metricsEntry);
  const x = $derived(entry.data);
  const refresh = () => serverData.fetchMetrics(m.ip, m.gamePort, m.queryPort, true);

  const uptimeTone = (u: number) => (u >= 90 ? "text-ok" : u >= 70 ? "text-warn" : "text-err");
  const country = $derived.by(() => {
    if (!x?.country) return null;
    try {
      return (
        new Intl.DisplayNames([getLocale()], { type: "region" }).of(x.country.toUpperCase()) ??
        x.country
      );
    } catch {
      return x.country;
    }
  });
  // Specta writes f64 as `number | null` (a NaN serialises to null).
  const history = $derived(
    (x?.player_history ?? []).filter((p): p is [number, number] => p[1] != null),
  );
  const nextRestart = $derived(x?.restart?.next_restart ? untilText(x.restart.next_restart) : null);
  const source = (s: string | null | undefined) =>
    s === "announced" ? $c.dmAnnounced.value : s === "predicted" ? $c.dmPredicted.value : (s ?? "");

  const facts = $derived.by((): Fact[] => {
    if (!x) return [];
    const f: Fact[] = [];
    if (x.time_accel != null) {
      f.push({
        label: $c.dmTimeSpeed.value,
        value: $c.dmTimeSpeedValue({ day: x.time_accel, night: x.night_time_accel ?? x.time_accel })
          .value,
      });
    }
    if (x.vanilla_band) {
      f.push({
        label: $c.dmStyle.value,
        value: x.vanilla_band,
        title: x.vanilla_score != null ? `${Math.round(x.vanilla_score)} / 100` : undefined,
        tone: "text-mods",
      });
    }
    if (x.playstyle) f.push({ label: $c.dmStyle.value, value: x.playstyle });
    if (x.mod_total_bytes) f.push({ label: $c.dmModsSize.value, value: bytes(x.mod_total_bytes) });
    if (x.first_seen)
      f.push({
        label: $c.dmTrackedSince.value,
        value: date(x.first_seen),
        title: dateTime(Date.parse(x.first_seen) / 1000),
      });
    if (x.ping_lo != null && x.ping_hi != null) {
      f.push({
        label: $c.dmPingRange.value,
        value: `${Math.round(x.ping_lo)}–${Math.round(x.ping_hi)} ms${x.ping_jitter != null ? ` ±${Math.round(x.ping_jitter)}` : ""}`,
        tone: "text-fg-muted",
      });
    }
    return f;
  });

  const links = $derived.by(() => {
    if (!x) return [] as { label: string; url: string }[];
    const l: { label: string; url: string }[] = [];
    if (x.discord) l.push({ label: $c.dmDiscord.value, url: x.discord });
    if (x.website) l.push({ label: $c.dmWebsite.value, url: x.website });
    for (const k of x.links) if (!l.some((e) => e.url === k.url)) l.push(k);
    return l;
  });
</script>

<div class="flex min-h-0 flex-1 flex-col">
  <div class="flex items-center gap-2 px-pad pt-2.5">
    <span class="label-stencil text-fg-faint">{$c.dmTitle.value}</span>
    {#if x?.status}
      <Tag tone={x.status === "online" ? "ok" : x.status === "offline" ? "err" : "neutral"}>
        <span class="inline-flex items-center gap-1">
          <span class="size-1.5 rounded-full bg-current"></span>{x.status === "online"
            ? $c.bmOnline.value
            : x.status === "offline"
              ? $c.bmOffline.value
              : x.status}
        </span>
      </Tag>
    {/if}
    <button
      class="ml-auto grid size-control-sm place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg disabled:opacity-40"
      title={$c.dmRefresh.value}
      aria-label={$c.dmRefresh.value}
      disabled={entry.loading}
      onclick={refresh}
    >
      {#if entry.loading}<Spinner class="size-3.5" />{:else}<RefreshCw class="size-3.5" />{/if}
    </button>
  </div>

  {#if entry.loading && !x}
    <div class="flex flex-col gap-2 px-pad py-2.5">
      <div class="h-14 animate-pulse rounded-sm bg-raised/60"></div>
      <div class="h-36 animate-pulse rounded-sm bg-raised/60"></div>
      <div class="h-20 animate-pulse rounded-sm bg-raised/60"></div>
    </div>
  {:else if x}
    <div class="flex flex-col gap-2.5 px-pad py-2.5">
      <PopulationWarning {m} />

      <!-- The figures a player weighs a server by, big where they matter. -->
      <div
        class="grid grid-cols-2 gap-px overflow-hidden rounded-sm border border-border bg-border @min-[26rem]:grid-cols-4"
      >
        <div class="bg-panel px-2 py-1.5">
          <div class="label-stencil text-fg-faint">{$c.bmRank.value}</div>
          <div class="title-display num text-xl text-accent">
            {x.rank_pos != null ? `#${num(x.rank_pos)}` : "—"}
          </div>
        </div>
        <div class="bg-panel px-2 py-1.5">
          <div class="label-stencil text-fg-faint">{$c.dmUptime7d.value}</div>
          <div
            class="title-display num text-xl {x.uptime_7d != null
              ? uptimeTone(x.uptime_7d)
              : 'text-fg-faint'}"
          >
            {x.uptime_7d != null ? `${x.uptime_7d.toFixed(1)}%` : "—"}
          </div>
        </div>
        <div class="bg-panel px-2 py-1.5">
          <div class="label-stencil text-fg-faint">{$c.dmPeak7d.value}</div>
          <div class="title-display num text-xl text-fg">
            {x.peak_7d != null ? num(x.peak_7d) : "—"}<span class="text-sm text-fg-faint"
              >/{x.max_players != null ? num(x.max_players) : "?"}</span
            >
          </div>
        </div>
        <div class="bg-panel px-2 py-1.5">
          <div class="label-stencil text-fg-faint">{$c.dmAvg7d.value}</div>
          <div class="flex items-baseline gap-1.5">
            <span class="title-display num text-xl text-fg">
              {x.avg_players_7d != null ? num(Math.round(x.avg_players_7d)) : "—"}
            </span>
            {#if x.wow_pct != null}
              {@const up = x.wow_pct >= 0}
              <span
                class="inline-flex items-center gap-0.5 font-mono text-2xs {up
                  ? 'text-ok'
                  : 'text-err'}"
                title={$c.dmTrend.value}
              >
                {#if up}<TrendingUp class="size-3" />{:else}<TrendingDown class="size-3" />{/if}{up
                  ? "+"
                  : ""}{Math.round(x.wow_pct)}%
              </span>
            {/if}
          </div>
        </div>
      </div>

      {#if history.length > 1}
        <!-- Drawn taller than the kit's default: here the chart is the tab's point. -->
        <div class="[&_svg]:h-36">
          <PlayerChart points={history} max={x.max_players ?? undefined} />
        </div>
      {/if}
    </div>

    {#if x.restart || x.wipe}
      <Section icon={CalendarClock} title={$c.dmSchedule.value}>
        <dl
          class="m-0 grid grid-cols-[auto_minmax(0,1fr)] items-baseline gap-x-3 gap-y-1.5 text-2xs"
        >
          {#if x.restart?.next_restart}
            <dt class="text-fg-faint">{$c.dmNextRestart.value}</dt>
            <dd class="m-0 text-fg" title={dateTime(Date.parse(x.restart.next_restart) / 1000)}>
              {nextRestart
                ? $c.dmIn({ time: nextRestart }).value
                : dateTime(Date.parse(x.restart.next_restart) / 1000)}
              {#if x.restart.period_hours}
                <span class="text-fg-faint"
                  >· {$c.dmEvery({ hours: x.restart.period_hours }).value}</span
                >
              {/if}
              {#if x.restart.slots_utc.length}
                <span class="mt-0.5 block font-mono text-3xs text-fg-faint"
                  >{x.restart.slots_utc.join(" · ")} UTC</span
                >
              {/if}
            </dd>
          {/if}
          {#if x.wipe?.next}
            <dt class="text-fg-faint">{$c.dmNextWipe.value}</dt>
            <dd class="m-0 text-fg">
              {x.wipe.next}
              {#if x.wipe.days_until != null}
                <span class="text-fg-muted"
                  >· {$c.dmInDays({ days: Math.round(x.wipe.days_until) }).value}</span
                >
              {/if}
              <Tag tone={x.wipe.next_source === "announced" ? "ok" : "neutral"}
                >{source(x.wipe.next_source)}</Tag
              >
            </dd>
          {/if}
          {#if x.wipe?.last}
            <dt class="text-fg-faint">{$c.dmLastWipe.value}</dt>
            <dd class="m-0 text-fg-muted">
              {x.wipe.last}
              {#if x.wipe.days_since != null}· {$c.dmDaysAgo({
                  days: Math.round(x.wipe.days_since),
                }).value}{/if}
            </dd>
          {/if}
        </dl>
      </Section>
    {/if}

    {#if facts.length || country}
      <Section icon={Info} title={$c.details.value}>
        {#if country && x.country}
          <p class="m-0 flex items-center gap-1.5 text-2xs text-fg-muted">
            <Flag code={x.country} class="size-3.5" />{country}
          </p>
        {/if}
        <Facts items={facts} />
      </Section>
    {/if}

    {#if x.notices.length}
      <Section icon={Megaphone} title={$c.dmNotices.value}>
        <ul class="m-0 flex list-disc flex-col gap-1 pl-4 text-2xs text-fg-muted">
          {#each x.notices as n, i (i)}<li data-selectable>{n}</li>{/each}
        </ul>
      </Section>
    {/if}

    {#if links.length}
      <Section icon={Link} title={$c.dmLinks.value}>
        <div class="flex flex-wrap gap-1.5">
          {#each links as l (l.url)}
            <button
              class="inline-flex max-w-full items-center gap-1 rounded-sm border border-border px-2 py-1 text-2xs text-fg-muted hover:border-border-strong hover:text-fg"
              title={l.url}
              onclick={() => openUrl(l.url)}
            >
              <ExternalLink class="size-3 shrink-0" /><span class="truncate">{l.label}</span>
            </button>
          {/each}
        </div>
      </Section>
    {/if}

    <div class="flex items-center gap-3 px-pad py-2.5 text-2xs text-fg-faint">
      <button
        class="inline-flex items-center gap-1.5 hover:text-accent"
        onclick={() => openUrl(x.url)}
      >
        <ExternalLink class="size-3" />{$c.openDm.value}
      </button>
      <span class="ml-auto">{$c.dmSource.value}</span>
    </div>
  {:else if entry.error}
    <div class="px-pad py-2.5">
      {#if entry.error.startsWith("Not listed")}
        <Empty icon={ChartLine} title={$c.dmNotListed.value} compact />
      {:else}
        <div
          class="flex items-start gap-2 rounded-sm border border-err/30 bg-err/10 px-2 py-1.5 text-2xs text-err"
        >
          <span class="min-w-0 flex-1 break-words">{entry.error}</span>
          <button class="shrink-0 underline" onclick={refresh}>{$c.retry.value}</button>
        </div>
      {/if}
    </div>
  {/if}
</div>
