<script lang="ts">
  import { dict } from "$lib/i18n";
  import ScrollText from "~icons/lucide/scroll-text";
  import { Input } from "$lib/components/ui/input";
  import { Copy } from "$lib/components/ui/copy";
  import { Empty } from "$lib/components/app";
  import type { DetailModel } from "./model.svelte";

  /** The server's configuration as it reports it over A2S, searchable, each value copyable. */
  let { m }: { m: DetailModel } = $props();
  const c = dict("detail");

  let query = $state("");
  const rules = $derived(m.a2s?.rules ?? []);
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return q ? rules.filter((r) => r.name.toLowerCase().includes(q) || r.value.toLowerCase().includes(q)) : rules;
  });
</script>

<div class="flex min-h-0 flex-1 flex-col gap-2 px-pad py-2.5">
  {#if m.live.loading && !m.a2s}
    <div class="flex flex-col gap-1">
      {#each [0, 1, 2, 3] as i (i)}<div class="h-6 animate-pulse rounded-xs bg-raised/60"></div>{/each}
    </div>
  {:else if rules.length === 0}
    <Empty icon={ScrollText} title={$c.noRules.value} compact />
  {:else}
    <Input type="search" size="xs" placeholder={$c.searchRules.value} bind:value={query} />
    <dl class="m-0 grid min-h-0 flex-1 auto-rows-min grid-cols-[minmax(0,2fr)_minmax(0,3fr)] overflow-y-auto rounded-sm border border-border bg-bg font-mono text-3xs">
      {#each shown as r (r.name)}
        <dt class="truncate border-b border-border/50 px-2 py-1 text-fg-faint" title={r.name}>{r.name}</dt>
        <dd class="m-0 min-w-0 border-b border-border/50 px-2 py-1"><Copy text={r.value} title={$c.copyValue.value} class="text-3xs text-fg-muted" /></dd>
      {:else}
        <p class="col-span-2 m-0 px-2 py-3 text-center text-2xs text-fg-faint">{$c.noMatch.value}</p>
      {/each}
    </dl>
  {/if}
</div>
