<script lang="ts">
  import { dict } from "$lib/i18n";
  import ChartLine from "~icons/lucide/chart-line";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import ExternalLink from "~icons/lucide/external-link";
  import BadgeCheck from "~icons/lucide/badge-check";
  import Lock from "~icons/lucide/lock";
  import Eye from "~icons/lucide/eye";
  import Crosshair from "~icons/lucide/crosshair";
  import Puzzle from "~icons/lucide/puzzle";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Tag } from "$lib/components/ui/tag";
  import { Flag } from "$lib/components/ui/flag";
  import { serverData } from "$lib/stores/server-data.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { openUrl } from "$lib/ipc/native";
  import { getLocale } from "$lib/i18n";
  import { distanceKm, date } from "$lib/format";
  import { PlayerChart, Section } from "$lib/components/app";

  /** BattleMetrics' view of the server: rank, uptime, where it is, a day of players. */
  let { ip, port, queryPort, name }: { ip: string; port: number; queryPort: number; name: string } =
    $props();
  const c = dict("detail");

  const hasKey = $derived(!!profile.data?.battlemetrics_api_key);
  const entry = $derived(serverData.bm(ip, port, queryPort));
  const bm = $derived(entry.data);

  $effect(() => {
    if (!hasKey) return;
    const t = setTimeout(() => void serverData.fetchBm(ip, port, queryPort, name), 120);
    return () => clearTimeout(t);
  });

  function age(iso: string): string {
    const days = Math.floor((Date.now() - new Date(iso).getTime()) / 86_400_000);
    const w = $c;
    if (days < 1) return w.ageToday.value;
    if (days === 1) return w.ageDay.value;
    if (days < 30) return w.ageDays({ count: days }).value;
    const months = Math.floor(days / 30);
    if (months === 1) return w.ageMonth.value;
    if (months < 12) return w.ageMonths({ count: months }).value;
    const years = Math.floor(days / 365);
    return years === 1 ? w.ageYear.value : w.ageYears({ count: years }).value;
  }

  const country = $derived.by(() => {
    if (!bm?.country) return null;
    try {
      return (
        new Intl.DisplayNames([getLocale()], { type: "region" }).of(bm.country.toUpperCase()) ??
        bm.country
      );
    } catch {
      return bm.country;
    }
  });
  const km = $derived(
    bm?.location && bm.location[0] != null && bm.location[1] != null && profile.data?.user_location
      ? Math.round(
          distanceKm(profile.data.user_location as [number, number], [
            bm.location[0],
            bm.location[1],
          ]),
        )
      : null,
  );
  const uptoneTone = (u: number) => (u >= 90 ? "text-ok" : u >= 70 ? "text-warn" : "text-err");
</script>

<Section icon={ChartLine} title={$c.bmTitle.value}>
  {#snippet actions()}
    {#if hasKey}
      <button
        class="grid size-control-sm place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg disabled:opacity-40"
        title={$c.bmRefreshTitle.value}
        aria-label={$c.bmRefreshTitle.value}
        disabled={entry.loading}
        onclick={() => serverData.fetchBm(ip, port, queryPort, name, true)}
      >
        {#if entry.loading}<Spinner class="size-3.5" />{:else}<RefreshCw class="size-3.5" />{/if}
      </button>
    {/if}
  {/snippet}

  {#if !hasKey}
    <p class="m-0 text-2xs leading-snug text-fg-faint">{$c.bmConfigure.value}</p>
  {:else if entry.loading && !bm}
    <div class="flex items-center gap-2 text-2xs text-fg-faint">
      <Spinner class="size-3.5" />{$c.bmLoading.value}
    </div>
  {:else if bm}
    <div class="flex flex-wrap gap-1">
      <Tag tone={bm.status === "online" ? "ok" : bm.status === "offline" ? "err" : "neutral"}>
        <span class="inline-flex items-center gap-1">
          <span class="size-1.5 rounded-full bg-current"></span>{bm.status === "online"
            ? $c.bmOnline.value
            : bm.status === "offline"
              ? $c.bmOffline.value
              : bm.status}
        </span>
      </Tag>
      {#if bm.official}<Tag tone="accent"
          ><span class="inline-flex items-center gap-1"
            ><BadgeCheck class="size-3" />{$c.bmOfficial.value}</span
          ></Tag
        >{/if}
      {#if bm.private}<Tag tone="err"
          ><span class="inline-flex items-center gap-1"
            ><Lock class="size-3" />{$c.bmPrivate.value}</span
          ></Tag
        >{/if}
      {#if bm.third_person === true}<Tag
          ><span class="inline-flex items-center gap-1"><Eye class="size-3" />{$c.bm3pp.value}</span
          ></Tag
        >{/if}
      {#if bm.third_person === false}<Tag tone="warn"
          ><span class="inline-flex items-center gap-1"
            ><Crosshair class="size-3" />{$c.bm1pp.value}</span
          ></Tag
        >{/if}
      {#if bm.modded}<Tag
          ><span class="inline-flex items-center gap-1 text-mods"
            ><Puzzle class="size-3" />{$c.bmModded.value}</span
          ></Tag
        >{/if}
    </div>

    <!-- The figures a player weighs a server by, big where they matter. -->
    <div class="grid grid-cols-3 gap-px overflow-hidden rounded-sm border border-border bg-border">
      <div class="bg-panel px-2 py-1.5">
        <div class="label-stencil text-fg-faint">{$c.bmRank.value}</div>
        <div class="title-display num text-xl text-accent">
          {bm.rank !== null ? `#${bm.rank}` : "—"}
        </div>
      </div>
      <div class="bg-panel px-2 py-1.5">
        <div class="label-stencil text-fg-faint">{$c.bmUptime.value}</div>
        <div
          class="title-display num text-xl {bm.uptime !== null
            ? uptoneTone(bm.uptime)
            : 'text-fg-faint'}"
        >
          {bm.uptime !== null ? `${bm.uptime.toFixed(1)}%` : "—"}
        </div>
      </div>
      <div class="bg-panel px-2 py-1.5">
        <div class="label-stencil text-fg-faint">{$c.bmPlayers.value}</div>
        <div class="title-display num text-xl text-fg">
          {bm.players ?? "—"}<span class="text-sm text-fg-faint">/{bm.max_players ?? "?"}</span>
        </div>
      </div>
    </div>

    {#if bm.player_history.length > 1}
      <PlayerChart points={bm.player_history} max={bm.max_players} />
    {/if}

    <dl class="m-0 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-2xs">
      <dt class="text-fg-faint">{$c.bmName.value}</dt>
      <dd class="m-0 truncate text-fg-muted" title={bm.name}>{bm.name}</dd>
      {#if bm.country}
        <dt class="text-fg-faint">{$c.bmCountry.value}</dt>
        <dd class="m-0 flex items-center gap-1.5 text-fg-muted">
          <Flag code={bm.country} class="size-3.5" />{country}
          {#if km !== null}<span class="ml-auto font-mono text-fg-faint"
              >{$c.distanceKm({ km }).value}</span
            >{/if}
        </dd>
      {/if}
      {#if bm.created_at}
        <dt class="text-fg-faint">{$c.bmFirstSeen.value}</dt>
        <dd class="m-0 text-fg-muted" title={date(bm.created_at)}>
          {$c.ago({ time: age(bm.created_at) }).value}
        </dd>
      {/if}
      {#if bm.server_steam_id}
        <dt class="text-fg-faint">{$c.bmSteamId.value}</dt>
        <dd class="m-0">
          <button
            class="font-mono text-fg-muted hover:text-accent"
            title={$c.bmOpenSteam.value}
            onclick={() => openUrl(`https://steamcommunity.com/profiles/${bm.server_steam_id}`)}
            >{bm.server_steam_id}</button
          >
        </dd>
      {/if}
    </dl>

    <button
      class="inline-flex items-center gap-1.5 self-start text-2xs text-fg-faint hover:text-accent"
      onclick={() => openUrl(`https://www.battlemetrics.com/servers/dayz/${bm.id}`)}
    >
      <ExternalLink class="size-3" />{$c.bmView.value}
    </button>
  {:else if entry.error}
    <div
      class="flex items-start gap-2 rounded-sm border border-err/30 bg-err/10 px-2 py-1.5 text-2xs text-err"
    >
      <span class="min-w-0 flex-1 break-words">{entry.error}</span>
      <button
        class="shrink-0 underline"
        onclick={() => serverData.fetchBm(ip, port, queryPort, name, true)}>{$c.retry.value}</button
      >
    </div>
  {:else}
    <p class="m-0 text-2xs text-fg-faint">{$c.bmNotFound.value}</p>
  {/if}
</Section>
