<script lang="ts">
  import { dict } from "$lib/i18n";
  import Lock from "~icons/lucide/lock";
  import Puzzle from "~icons/lucide/puzzle";
  import ShieldCheck from "~icons/lucide/shield-check";
  import Eye from "~icons/lucide/eye";
  import EyeOff from "~icons/lucide/eye-off";
  import FilterX from "~icons/lucide/filter-x";
  import { Input } from "$lib/components/ui/input";
  import { Select } from "$lib/components/ui/select";
  import { Chip } from "$lib/components/ui/chip";
  import { Button } from "$lib/components/ui/button";
  import { profile } from "$lib/stores/profile.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { filters } from "./filters.svelte";
  import TriChip from "./TriChip.svelte";

  let { shown }: { shown: number } = $props();
  const c = dict("servers");

  // Every map, with how many servers run it, busiest first (counted by the backend).
  const maps = $derived(servers.maps.map((m) => ({ value: m.map, label: `${m.map} · ${m.count}` })));

  const pingOptions = [
    { value: "0", label: "" },
    { value: "50", label: "≤ 50 ms" },
    { value: "100", label: "≤ 100 ms" },
    { value: "150", label: "≤ 150 ms" },
    { value: "250", label: "≤ 250 ms" },
  ];
</script>

<div class="flex flex-wrap items-center gap-1.5">
  <Input
    type="search"
    class="w-72 max-w-full"
    placeholder={$c.searchPlaceholder.value}
    clearLabel={$c.clearSearch.value}
    bind:value={() => filters.search, (v) => filters.setSearch(String(v ?? ""))}
  />

  <span class="mx-0.5 h-4 w-px bg-border"></span>

  <TriChip bind:value={filters.firstPerson} label="1PP" title={$c.filterFpTitleAll.value} />
  <TriChip bind:value={filters.password} label={$c.passwordProtected.value} icon={Lock} title={$c.filterPwdTitleAll.value} />
  <TriChip bind:value={filters.battleye} label="BattlEye" icon={ShieldCheck} title={$c.filterBeTitleAll.value} />
  <TriChip bind:value={filters.modded} label={$c.colMods.value} icon={Puzzle} title={$c.filterModsTitleAll.value} />

  <span class="mx-0.5 h-4 w-px bg-border"></span>

  <Select
    class="w-44"
    aria-label={$c.colMap.value}
    placeholder={$c.allMaps.value}
    options={maps}
    bind:value={filters.map}
  />
  <Select
    class="w-28"
    aria-label={$c.maxPing.value}
    options={pingOptions.map((o) => ({ ...o, label: o.value === "0" ? $c.anyPing.value : o.label }))}
    bind:value={() => String(filters.maxPing), (v) => (filters.maxPing = Number(v ?? 0))}
  />
  <Chip active={filters.hideEmpty} onclick={() => (filters.hideEmpty = !filters.hideEmpty)}>{$c.hideEmpty.value}</Chip>
  <Chip active={filters.hideFull} onclick={() => (filters.hideFull = !filters.hideFull)}>{$c.hideFull.value}</Chip>

  {#if profile.excludedIps.size > 0}
    <Chip
      active={filters.showExcluded}
      title={filters.showExcluded ? $c.excludedHide.value : $c.excludedReveal.value}
      onclick={() => (filters.showExcluded = !filters.showExcluded)}
    >
      {#if filters.showExcluded}<Eye class="size-3" />{:else}<EyeOff class="size-3" />{/if}
      {$c.excludedCount({ count: profile.excludedIps.size }).value}
    </Chip>
  {/if}

  {#if filters.active}
    <Button variant="ghost" size="xs" onclick={() => filters.clear()}>
      <FilterX class="size-3" />{$c.clearFilters.value}
    </Button>
  {/if}

  <span class="ml-auto font-mono text-2xs text-fg-faint">
    <span class="num text-fg">{shown.toLocaleString()}</span> / {servers.total.toLocaleString()}
  </span>
</div>
