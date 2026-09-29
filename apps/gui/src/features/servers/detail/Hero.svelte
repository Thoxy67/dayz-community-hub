<script lang="ts">
  import { dict, getLocale } from "$lib/i18n";
  import X from "~icons/lucide/x";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Topo } from "$lib/components/ui/topo";
  import { Flag } from "$lib/components/ui/flag";
  import {
    OsIcon,
    PingButton,
    PlayersButton,
    ServerFlags,
    TimeOfDay,
    ModsCount,
  } from "$lib/components/app";
  import { mapName } from "$lib/components/app/map-name";
  import { serverData } from "$lib/stores/server-data.svelte";
  import type { DetailModel } from "./model.svelte";
  import PopulationWarning from "./PopulationWarning.svelte";

  /**
   * The top of the panel: the map is what a player recognises first, so it
   * is the anchor, set large over contour lines in the map's colour; then the
   * server's name, what it asks of you, where it is, and the live strip
   * (line, head-count, clock, mods) that decides whether to join now.
   */
  let { m, onclose, onmods }: { m: DetailModel; onclose?: () => void; onmods: () => void } =
    $props();
  const c = dict("detail");

  const country = $derived.by(() => {
    const code = m.country;
    if (!code) return null;
    try {
      return {
        code,
        name:
          new Intl.DisplayNames([getLocale()], { type: "region" }).of(code.toUpperCase()) ?? code,
      };
    } catch {
      return { code, name: code };
    }
  });
  const version = $derived(m.listed?.version || m.a2s?.version || "");
  const queue = $derived(m.a2s?.dayz?.login_queue ?? 0);
</script>

<header class="relative shrink-0 overflow-hidden border-b border-border">
  <!-- The map's ground: a wash of its colour over survey contours. -->
  <div
    class="pointer-events-none absolute inset-0 bg-gradient-to-br from-map/18 via-map/5 to-transparent"
  ></div>
  <Topo class="text-map" opacity={0.28} />

  <div class="relative flex flex-col gap-2 px-pad pt-2.5 pb-2.5">
    <div class="flex items-start gap-2">
      <div class="min-w-0 flex-1">
        <div class="flex items-baseline gap-2">
          <span class="truncate title-display text-2xl leading-none text-map" title={m.map}>
            {mapName(m.map) || "—"}
          </span>
          {#if m.map && mapName(m.map).toLowerCase() !== m.map.toLowerCase()}
            <span class="truncate font-mono text-3xs text-fg-faint">{m.map}</span>
          {/if}
        </div>
        <h2
          class="m-0 mt-1.5 line-clamp-2 text-sm leading-snug font-semibold break-words text-fg"
          title={m.title}
          data-selectable
        >
          {m.title}
        </h2>
      </div>
      <div class="flex shrink-0 items-center gap-0.5">
        <button
          class="grid size-control-sm place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg disabled:opacity-40"
          title={$c.refreshLive.value}
          aria-label={$c.refreshLive.value}
          disabled={m.live.loading}
          onclick={() => serverData.refreshA2s(m.ip, m.queryPort)}
        >
          {#if m.live.loading}<Spinner class="size-3.5" />{:else}<RefreshCw class="size-3.5" />{/if}
        </button>
        {#if onclose}
          <button
            class="grid size-control-sm place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg"
            title={$c.close.value}
            aria-label={$c.close.value}
            onclick={onclose}><X class="size-3.5" /></button
          >
        {/if}
      </div>
    </div>

    <div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-2xs text-fg-faint">
      {#if m.listed}
        <ServerFlags
          official={m.listed.official}
          mimicsOfficial={m.listed.mimics_official}
          password={m.listed.password}
          firstPerson={m.listed.first_person_only}
          battleye={m.listed.battl_eye}
        />
        <OsIcon environment={m.listed.environment} class="size-3" />
      {:else if m.a2s?.dayz}
        <ServerFlags
          official={m.a2s.dayz.official}
          firstPerson={m.a2s.dayz.first_person_only}
          battleye={m.a2s.dayz.battleye}
        />
      {/if}
      {#if country}
        <span class="inline-flex items-center gap-1"
          ><Flag code={country.code} class="size-3.5" />{country.name}</span
        >
      {/if}
      {#if version}<span class="font-mono">v{version}</span>{/if}
      {#if m.listed?.excluded}<span class="text-err">{$c.excluded.value}</span>{/if}
    </div>

    <!-- The live strip: what decides whether to join right now. -->
    <div
      class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 rounded-sm border border-border bg-panel/80 px-2.5 py-2 @min-[26rem]:grid-cols-[auto_1fr_auto_auto]"
    >
      <div class="flex flex-col gap-1">
        <span class="label-stencil text-fg-faint">{$c.ping.value}</span>
        <PingButton ip={m.ip} queryPort={m.queryPort} size="md" />
      </div>
      <div class="flex min-w-0 flex-col gap-1">
        <span class="label-stencil text-fg-faint">{$c.players.value}</span>
        <PlayersButton ip={m.ip} queryPort={m.queryPort} />
        {#if queue > 0}
          <span class="num text-3xs text-warn"
            >{$c.loginQueue.value} · {$c.queueWaiting({ count: queue }).value}</span
          >
        {/if}
      </div>
      <div class="flex flex-col gap-1">
        <span class="label-stencil text-fg-faint">{$c.time.value}</span>
        <TimeOfDay time={m.a2s?.dayz?.game_time ?? m.listed?.time} />
      </div>
      <div class="flex flex-col gap-1">
        <span class="label-stencil text-fg-faint">{$c.tabMods.value}</span>
        {#if m.listed}
          <ModsCount count={m.listed.mods_count} onclick={onmods} />
        {:else}
          <span class="font-mono text-2xs text-fg-muted">{m.modsCount || "—"}</span>
        {/if}
      </div>
    </div>

    <PopulationWarning {m} compact />

    {#if !m.listed}
      <p class="m-0 flex items-start gap-1.5 text-2xs leading-snug text-warn">
        <TriangleAlert class="mt-px size-3.5 shrink-0" />{$c.notListed.value}
      </p>
    {/if}
  </div>
</header>
