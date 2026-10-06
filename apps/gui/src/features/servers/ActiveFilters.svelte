<script lang="ts">
  import { dict } from "$lib/i18n";
  import X from "~icons/lucide/x";
  import FilterX from "~icons/lucide/filter-x";
  import { filters, type Tri } from "./filters.svelte";

  /**
   * The filters in force, in words, each with its own ✕, and one button
   * that clears them all. The chips on the toolbar say a state with an icon
   * and a tick or a cross; this line says it in a sentence a newcomer reads.
   */
  const c = dict("servers");

  type Chip = { key: string; text: string; clear: () => void };

  const tri = (key: string, value: Tri, label: string, clear: () => void): Chip[] =>
    value === "all"
      ? []
      : [
          {
            key,
            text: value === "only" ? $c.filterYes({ label }).value : $c.filterNo({ label }).value,
            clear,
          },
        ];

  const chips = $derived<Chip[]>([
    ...(filters.query
      ? [
          {
            key: "search",
            text: $c.filterSearch({ query: filters.query }).value,
            clear: () => filters.setSearch(""),
          },
        ]
      : []),
    ...(filters.map
      ? [
          {
            key: "map",
            text: $c.filterValue({ label: $c.colMap.value, value: filters.map }).value,
            clear: () => (filters.map = null),
          },
        ]
      : []),
    ...tri("fp", filters.firstPerson, "1PP", () => (filters.firstPerson = "all")),
    ...tri("pwd", filters.password, $c.passwordProtected.value, () => (filters.password = "all")),
    ...tri("be", filters.battleye, "BattlEye", () => (filters.battleye = "all")),
    ...tri("mods", filters.modded, $c.colMods.value, () => (filters.modded = "all")),
    ...tri("off", filters.official, $c.official.value, () => (filters.official = "all")),
    ...(filters.maxPing > 0
      ? [
          {
            key: "ping",
            text: `${$c.colPing.value} ≤ ${filters.maxPing} ms`,
            clear: () => (filters.maxPing = 0),
          },
        ]
      : []),
    ...(filters.hideEmpty
      ? [{ key: "empty", text: $c.hideEmpty.value, clear: () => (filters.hideEmpty = false) }]
      : []),
    ...(filters.hideFull
      ? [{ key: "full", text: $c.hideFull.value, clear: () => (filters.hideFull = false) }]
      : []),
  ]);
</script>

{#if chips.length > 0}
  <div class="flex min-w-0 flex-wrap items-center gap-1">
    {#each chips as chip (chip.key)}
      <span
        class="inline-flex h-control-sm items-center gap-1 rounded-sm border border-accent/35 bg-accent/8 pr-0.5 pl-2 text-2xs text-fg"
      >
        {chip.text}
        <button
          type="button"
          class="grid size-4 place-items-center rounded-xs text-fg-faint hover:bg-accent/15 hover:text-fg"
          title={$c.filterRemove({ label: chip.text }).value}
          aria-label={$c.filterRemove({ label: chip.text }).value}
          onclick={chip.clear}
        >
          <X class="size-3" />
        </button>
      </span>
    {/each}
    <button
      type="button"
      class="ml-1 inline-flex h-control-sm items-center gap-1 rounded-sm px-1.5 text-2xs font-medium text-accent hover:bg-accent/10"
      onclick={() => filters.clear()}
    >
      <FilterX class="size-3" />{$c.clearFilters.value}
    </button>
  </div>
{/if}
