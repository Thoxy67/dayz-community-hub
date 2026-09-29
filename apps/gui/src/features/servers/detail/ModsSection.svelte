<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import Puzzle from "~icons/lucide/puzzle";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import { Spinner } from "$lib/components/ui/spinner";
  import { ModStatusList, Section } from "$lib/components/app";
  import type { ServerDto } from "$lib/ipc/types";
  import { loadServerMods, serverMods } from "./server-mods.svelte";

  /**
   * What the server runs, against what is installed here, so the player knows
   * before joining what SteamCMD will have to fetch. Listed servers name their
   * mods with Workshop ids; others only by name, over A2S.
   */
  let {
    listed,
    a2sMods = [],
    focus = false,
  }: { listed: ServerDto | undefined; a2sMods?: string[]; focus?: boolean } = $props();
  const c = useIntlayer("detail");

  let el = $state<HTMLElement>();
  $effect(() => {
    if (!focus || !el) return;
    const t = setTimeout(() => el?.scrollIntoView({ behavior: "smooth", block: "start" }), 80);
    return () => clearTimeout(t);
  });

  // Debounced: arrowing through the list should not fire a request per row.
  $effect(() => {
    const s = listed;
    if (!s || s.mods_count === 0) return;
    const t = setTimeout(() => void loadServerMods(s.ip, s.query_port), 180);
    return () => clearTimeout(t);
  });

  const entry = $derived(listed ? serverMods(listed.ip, listed.query_port) : null);
  const items = $derived(
    (entry?.mods ?? []).map((m) => ({ id: m.steam_workshop_id, name: m.name })),
  );
  const count = $derived(entry?.mods?.length ?? listed?.mods_count ?? a2sMods.length);
  const retry = () => listed && loadServerMods(listed.ip, listed.query_port, true);
</script>

<Section bind:el icon={Puzzle} title={count > 0 ? $c.modsCount({ count }).value : $c.noMods.value}>
  {#snippet actions()}
    {#if listed && listed.mods_count > 0}
      <button
        class="grid size-control-sm place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg disabled:opacity-40"
        aria-label={$c.retry.value}
        title={$c.retry.value}
        disabled={entry?.loading}
        onclick={retry}
      >
        {#if entry?.loading}<Spinner class="size-3.5" />{:else}<RefreshCw class="size-3.5" />{/if}
      </button>
    {/if}
  {/snippet}

  {#if listed && listed.mods_count > 0}
    {#if entry?.loading && !entry.mods}
      <div class="flex flex-col gap-1">
        {#each Array.from({ length: Math.min(6, listed.mods_count) }, (_, i) => i) as i (i)}
          <div class="h-5 animate-pulse rounded-xs bg-raised/60"></div>
        {/each}
      </div>
    {:else if entry?.error}
      <div
        class="flex items-start gap-2 rounded-sm border border-warn/30 bg-warn/10 px-2 py-1.5 text-2xs text-warn"
      >
        <span class="min-w-0 flex-1">{$c.modsFailed({ count: listed.mods_count }).value}</span>
        <button class="shrink-0 underline" onclick={retry}>{$c.retry.value}</button>
      </div>
    {:else if items.length > 0}
      <ModStatusList {items} />
    {:else}
      <p class="m-0 text-2xs text-fg-faint italic">
        {$c.modsNotReported({ count: listed.mods_count }).value}
      </p>
    {/if}
  {:else if a2sMods.length > 0}
    <p class="m-0 text-2xs text-fg-faint">{$c.modsFromA2s.value}</p>
    <ul
      class="m-0 flex max-h-60 list-none flex-col overflow-y-auto rounded-sm border border-border bg-bg p-0"
    >
      {#each a2sMods as name, i (i)}
        <li
          class="flex h-6 items-center gap-2 border-b border-border/50 px-2 text-2xs text-fg last:border-b-0"
        >
          <span class="size-1 shrink-0 rounded-full bg-mods"></span><span class="truncate"
            >{name}</span
          >
        </li>
      {/each}
    </ul>
  {:else}
    <p class="m-0 text-2xs text-fg-faint italic">{$c.noMods.value}</p>
  {/if}
</Section>
