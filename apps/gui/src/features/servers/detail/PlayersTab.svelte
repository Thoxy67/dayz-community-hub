<script lang="ts">
  import { dict } from "$lib/i18n";
  import Users from "~icons/lucide/users";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import { Input } from "$lib/components/ui/input";
  import { Button } from "$lib/components/ui/button";
  import { Empty } from "$lib/components/app";
  import { serverData } from "$lib/stores/server-data.svelte";
  import { duration } from "$lib/format";
  import type { DetailModel } from "./model.svelte";

  /** Everyone the server says is on it, longest first, searchable. */
  let { m }: { m: DetailModel } = $props();
  const c = dict("detail");

  let query = $state("");
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return q ? m.players.filter((p) => (p.name ?? "").toLowerCase().includes(q)) : m.players;
  });
  const longest = $derived(Math.max(1, ...m.players.map((p) => p.duration ?? 0)));
</script>

<div class="flex min-h-0 flex-1 flex-col gap-2 px-pad py-2.5">
  {#if m.live.loading && !m.a2s}
    <div class="flex flex-col gap-1">
      {#each [0, 1, 2, 3, 4, 5] as i (i)}<div
          class="h-6 animate-pulse rounded-xs bg-raised/60"
        ></div>{/each}
    </div>
  {:else if m.live.error && !m.a2s}
    <Empty icon={Users} title={$c.liveFailed.value} compact>
      {#snippet action()}
        <Button onclick={() => serverData.refreshA2s(m.ip, m.queryPort)}
          ><RefreshCw class="size-3" />{$c.retry.value}</Button
        >
      {/snippet}
    </Empty>
  {:else if m.players.length === 0}
    <Empty
      icon={Users}
      title={m.a2s && m.a2s.players === 0 ? $c.a2sNoPlayers.value : $c.a2sNamesNotReported.value}
      compact
    />
  {:else}
    <Input type="search" size="xs" placeholder={$c.searchPlayers.value} bind:value={query} />
    <div
      class="grid grid-cols-[1.5rem_minmax(0,1fr)_3.5rem_4rem] gap-x-2 px-2 font-display text-3xs font-bold tracking-[0.1em] text-fg-faint uppercase"
    >
      <span class="text-right">#</span><span>{$c.players.value}</span><span class="text-right"
        >{$c.score.value}</span
      ><span class="text-right">{$c.timeHere.value}</span>
    </div>
    <ol
      class="m-0 flex min-h-0 flex-1 list-none flex-col overflow-y-auto rounded-sm border border-border bg-bg p-0"
    >
      {#each shown as p, i (i)}
        <li
          class="relative grid h-7 shrink-0 grid-cols-[1.5rem_minmax(0,1fr)_3.5rem_4rem] items-center gap-x-2 border-b border-border/50 px-2 text-2xs last:border-b-0"
        >
          <!-- Time on the server as a faint bar behind the row. -->
          <span
            class="pointer-events-none absolute inset-y-0 left-0 bg-accent/6"
            style="width: {((p.duration ?? 0) / longest) * 100}%"
          ></span>
          <span class="num relative text-right font-mono text-fg-faint">{i + 1}</span>
          <span class="relative truncate text-fg" data-selectable>{p.name || "—"}</span>
          <span class="num relative text-right font-mono text-fg-muted">{p.score ?? 0}</span>
          <span class="num relative text-right font-mono text-fg">{duration(p.duration ?? 0)}</span>
        </li>
      {:else}
        <li class="px-2 py-3 text-center text-2xs text-fg-faint">{$c.noMatch.value}</li>
      {/each}
    </ol>
  {/if}
</div>
