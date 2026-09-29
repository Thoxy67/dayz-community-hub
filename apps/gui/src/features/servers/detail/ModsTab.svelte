<script lang="ts">
  import { dict } from "$lib/i18n";
  import Puzzle from "~icons/lucide/puzzle";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Download from "~icons/lucide/download";
  import ArrowUpCircle from "~icons/lucide/circle-arrow-up";
  import { Button } from "$lib/components/ui/button";
  import { Chip } from "$lib/components/ui/chip";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Empty, ModStatusList } from "$lib/components/app";
  import { mods } from "$lib/stores/mods.svelte";
  import { loadServerMods } from "./server-mods.svelte";
  import type { DetailModel } from "./model.svelte";

  /** What the server runs against what is installed, filterable, with the fix one click away. */
  let { m }: { m: DetailModel } = $props();
  const c = dict("detail");

  let filter = $state<"all" | "missing" | "stale">("all");
  const items = $derived(
    filter === "all"
      ? m.modRows
      : filter === "missing"
        ? m.modTotals.missing
        : m.modTotals.stale,
  );
  const retry = () => m.listed && loadServerMods(m.listed.ip, m.listed.query_port, true);
  const a2sMods = $derived(m.a2s?.mods_from_a2s ?? []);
</script>

<div class="flex min-h-0 flex-1 flex-col gap-2 px-pad py-2.5">
  {#if m.listed && m.listed.mods_count > 0}
    {#if m.modsEntry?.loading && !m.modsEntry.mods}
      <div class="flex flex-col gap-1">
        {#each Array.from({ length: Math.min(8, m.listed.mods_count) }, (_, i) => i) as i (i)}
          <div class="h-7 animate-pulse rounded-xs bg-raised/60"></div>
        {/each}
      </div>
    {:else if m.modsEntry?.error}
      <Empty icon={Puzzle} title={$c.modsFailed({ count: m.listed.mods_count }).value} compact>
        {#snippet action()}
          <Button onclick={retry}><RefreshCw class="size-3" />{$c.retry.value}</Button>
        {/snippet}
      </Empty>
    {:else if m.modRows.length > 0}
      <div class="flex flex-wrap items-center gap-1">
        <Chip active={filter === "all"} onclick={() => (filter = "all")}>{$c.filterAll.value} · {m.modRows.length}</Chip>
        <Chip active={filter === "missing"} onclick={() => (filter = "missing")}>
          <span class={m.modTotals.missing.length ? "text-err" : ""}>{$c.filterMissing.value} · {m.modTotals.missing.length}</span>
        </Chip>
        <Chip active={filter === "stale"} onclick={() => (filter = "stale")}>
          <span class={m.modTotals.stale.length ? "text-warn" : ""}>{$c.filterStale.value} · {m.modTotals.stale.length}</span>
        </Chip>
        <button
          class="ml-auto grid size-control-sm place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg disabled:opacity-40"
          title={$c.retry.value}
          aria-label={$c.retry.value}
          disabled={m.modsEntry?.loading}
          onclick={retry}
        >
          {#if m.modsEntry?.loading}<Spinner class="size-3.5" />{:else}<RefreshCw class="size-3.5" />{/if}
        </button>
      </div>
      {#if m.modTotals.missing.length || m.modTotals.stale.length}
        <div class="flex flex-wrap gap-1.5">
          {#if m.modTotals.missing.length}
            <Button variant="accent" size="xs" onclick={() => mods.install(m.modTotals.missing.map((r) => r.id))}>
              <Download class="size-3" />{$c.installMissing({ count: m.modTotals.missing.length }).value}
            </Button>
          {/if}
          {#if m.modTotals.stale.length}
            <Button size="xs" onclick={() => mods.updateMany(m.modTotals.stale.map((r) => r.id))}>
              <ArrowUpCircle class="size-3" />{$c.updateStale({ count: m.modTotals.stale.length }).value}
            </Button>
          {/if}
        </div>
      {/if}
      {#if items.length > 0}
        <ModStatusList {items} summary={filter === "all"} class="min-h-0 flex-1 [&>ul]:max-h-none [&>ul]:flex-1" />
      {:else}
        <p class="m-0 py-3 text-center text-2xs text-fg-faint">{$c.noneInFilter.value}</p>
      {/if}
    {:else}
      <p class="m-0 text-2xs text-fg-faint italic">{$c.modsNotReported({ count: m.listed.mods_count }).value}</p>
    {/if}
  {:else if a2sMods.length > 0}
    <p class="m-0 text-2xs text-fg-muted">{$c.modsFromA2s.value}</p>
    <ul class="m-0 flex min-h-0 flex-1 list-none flex-col overflow-y-auto rounded-sm border border-border bg-bg p-0">
      {#each a2sMods as name, i (i)}
        <li class="flex h-7 shrink-0 items-center gap-2 border-b border-border/50 px-2 text-2xs text-fg last:border-b-0">
          <span class="size-1 shrink-0 rounded-full bg-mods"></span><span class="truncate">{name}</span>
        </li>
      {/each}
    </ul>
  {:else}
    <Empty icon={Puzzle} title={$c.noMods.value} compact />
  {/if}
</div>
