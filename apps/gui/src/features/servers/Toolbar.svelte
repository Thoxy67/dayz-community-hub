<script lang="ts">
  import { dict } from "$lib/i18n";
  import Lock from "~icons/lucide/lock";
  import Puzzle from "~icons/lucide/puzzle";
  import ShieldCheck from "~icons/lucide/shield-check";
  import SlidersHorizontal from "~icons/lucide/sliders-horizontal";
  import Eye from "~icons/lucide/eye";
  import EyeOff from "~icons/lucide/eye-off";
  import FilterX from "~icons/lucide/filter-x";
  import { Input } from "$lib/components/ui/input";
  import { Select } from "$lib/components/ui/select";
  import { Popover } from "$lib/components/ui/popover";
  import { Switch } from "$lib/components/ui/switch";
  import { Segmented } from "$lib/components/ui/segmented";
  import { Button } from "$lib/components/ui/button";
  import { cn } from "$lib/cx";
  import { profile } from "$lib/stores/profile.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { filters } from "./filters.svelte";
  import TriChip from "./TriChip.svelte";

  /**
   * One line of filters. What is asked for most (search, the three-way
   * flags, the map) is on the line; the rest (ping ceiling, empty and full
   * servers, excluded addresses) is behind "More filters", whose badge says
   * how many of those are on. The line never wraps, whatever the width.
   */
  let { shown }: { shown: number } = $props();
  const c = dict("servers");

  const maps = $derived(servers.maps.map((m) => ({ value: m.map, label: `${m.map} · ${m.count}` })));
  const PINGS = ["0", "50", "100", "150", "250"] as const;
  const more = $derived(
    (filters.maxPing > 0 ? 1 : 0) + (filters.hideEmpty ? 1 : 0) + (filters.hideFull ? 1 : 0) + (filters.showExcluded ? 1 : 0),
  );
  let open = $state(false);
</script>

<div class="flex min-w-0 items-center gap-1.5">
  <Input
    type="search"
    class="w-44 min-w-32 flex-1 xl:max-w-80"
    placeholder={$c.searchPlaceholder.value}
    clearLabel={$c.clearSearch.value}
    bind:value={() => filters.search, (v) => filters.setSearch(String(v ?? ""))}
  />

  <div class="flex shrink-0 items-center gap-1">
    <TriChip bind:value={filters.firstPerson} label="1PP" title={$c.filterFpTitleAll.value} />
    <TriChip bind:value={filters.password} label={$c.passwordProtected.value} icon={Lock} compact title={$c.filterPwdTitleAll.value} />
    <TriChip bind:value={filters.battleye} label="BattlEye" icon={ShieldCheck} compact title={$c.filterBeTitleAll.value} />
    <TriChip bind:value={filters.modded} label={$c.colMods.value} icon={Puzzle} compact title={$c.filterModsTitleAll.value} />
  </div>

  <Select
    class="w-40 shrink-0"
    aria-label={$c.colMap.value}
    placeholder={$c.allMaps.value}
    options={maps}
    bind:value={filters.map}
  />

  <Popover
    label={$c.moreFilters.value}
    bind:open
    placement="bottom-end"
    triggerClass={cn(
      "relative h-control gap-1.5 border px-2 text-xs",
      more ? "border-accent/60 text-accent" : "border-border text-fg-muted",
    )}
    panelClass="w-72"
  >
    {#snippet trigger()}
      <span class="flex items-center gap-1.5">
        <SlidersHorizontal class="size-icon-sm" />
        <span class="max-2xl:hidden">{$c.moreFilters.value}</span>
        {#if more}
          <span class="num grid size-4 place-items-center rounded-full bg-accent font-mono text-3xs text-accent-fg">{more}</span>
        {/if}
      </span>
    {/snippet}
    <div class="flex flex-col">
      <div class="flex flex-col gap-1.5 border-b border-border p-2.5">
        <span class="label-stencil text-fg-faint">{$c.maxPing.value}</span>
        <Segmented
          aria-label={$c.maxPing.value}
          fill
          size="xs"
          value={String(filters.maxPing) as (typeof PINGS)[number]}
          onchange={(v) => (filters.maxPing = Number(v))}
          options={PINGS.map((p) => ({ value: p, label: p === "0" ? $c.anyPing.value : `≤${p}` }))}
        />
      </div>
      <Switch bind:checked={filters.hideEmpty} label={$c.hideEmpty.value} />
      <Switch bind:checked={filters.hideFull} label={$c.hideFull.value} />
      {#if profile.excludedIps.size > 0}
        <button
          class="flex items-center gap-2 border-t border-border px-2.5 py-2 text-left text-xs text-fg-muted hover:bg-raised hover:text-fg"
          onclick={() => (filters.showExcluded = !filters.showExcluded)}
        >
          {#if filters.showExcluded}<Eye class="size-icon-sm" />{:else}<EyeOff class="size-icon-sm" />{/if}
          <span class="flex-1">{filters.showExcluded ? $c.excludedHide.value : $c.excludedReveal.value}</span>
          <span class="num font-mono text-2xs text-fg-faint">{profile.excludedIps.size}</span>
        </button>
      {/if}
    </div>
  </Popover>

  {#if filters.active}
    <Button variant="ghost" size="icon" title={$c.clearFilters.value} aria-label={$c.clearFilters.value} onclick={() => filters.clear()}>
      <FilterX class="size-icon-sm" />
    </Button>
  {/if}

  <span class="ml-auto shrink-0 pl-1 font-mono text-2xs whitespace-nowrap text-fg-faint">
    <span class="num text-fg">{shown.toLocaleString()}</span> / {servers.total.toLocaleString()}
  </span>
</div>
