<script lang="ts">
  import { dict } from "$lib/i18n";
  import Star from "~icons/lucide/star";
  import History from "~icons/lucide/history";
  import { Signal } from "$lib/components/ui/signal";
  import { profile } from "$lib/stores/profile.svelte";
  import { servers, keyOf } from "$lib/stores/servers.svelte";
  import { relative } from "$lib/format";
  import { direct } from "./direct.svelte";

  /**
   * Addresses to start from: favourites first (they carry saved passwords),
   * then the servers joined most recently. A click fills the form and asks
   * the server. `wide`: as cards over the whole pane, with what the list
   * knows of each (map, players), while no server is shown there.
   */
  let { wide = false }: { wide?: boolean } = $props();
  const c = dict("connect");

  type Pick = {
    key: string;
    name: string;
    ip: string;
    port: number;
    ts?: number;
    password?: string | null;
    fav: boolean;
  };

  const picks = $derived.by((): Pick[] => {
    const seen = new Set<string>();
    const out: Pick[] = [];
    for (const f of profile.data?.favorites ?? []) {
      const k = `${f.ip}:${f.port}`;
      if (seen.has(k)) continue;
      seen.add(k);
      out.push({ key: k, name: f.name, ip: f.ip, port: f.port, password: f.password, fav: true });
    }
    for (const h of profile.data?.history ?? []) {
      const k = `${h.ip}:${h.port}`;
      if (seen.has(k)) continue;
      seen.add(k);
      out.push({ key: k, name: h.name, ip: h.ip, port: h.port, ts: h.ts, fav: false });
    }
    return out.slice(0, wide ? 24 : 14);
  });
</script>

{#if picks.length === 0}
  <p class="m-0 p-3 text-2xs text-fg-faint">{$c.noRecent.value}</p>
{:else if wide}
  <ul class="m-0 grid list-none grid-cols-[repeat(auto-fill,minmax(15rem,1fr))] gap-2 p-0">
    {#each picks as p (p.key)}
      {@const s = servers.find(p.ip, p.port)}
      {@const k = s ? keyOf(s) : p.key}
      <li>
        <button
          class="flex h-full w-full flex-col gap-1.5 rounded-md border border-border bg-raised/30 p-2.5 text-left hover:border-accent/50 hover:bg-raised/70"
          onclick={() =>
            direct.load(p.ip, s?.game_port ?? p.port, s?.query_port, p.password ?? undefined)}
        >
          <span class="flex min-w-0 items-start gap-2">
            {#if p.fav}
              <Star
                class="mt-0.5 size-3.5 shrink-0 fill-warn text-warn"
                aria-label={$c.fromFavorites.value}
              />
            {:else}
              <History class="mt-0.5 size-3.5 shrink-0 text-fg-faint" />
            {/if}
            <span class="line-clamp-2 min-w-0 flex-1 text-xs font-semibold text-fg">{p.name}</span>
            {#if s}<Signal ms={servers.ping.get(k)} size="xs" label={false} />{/if}
          </span>
          <span class="flex min-w-0 items-center gap-2 font-mono text-3xs text-fg-faint">
            <span class="truncate">{p.ip}:{s?.game_port ?? p.port}</span>
            {#if p.ts}<span class="ml-auto shrink-0">{relative(p.ts)}</span>{/if}
          </span>
          {#if s}
            <span class="flex items-center gap-2 text-2xs">
              <span class="truncate text-map">{s.map}</span>
              <span class="ml-auto num font-mono text-fg-muted"
                >{s.players}<span class="text-fg-faint">/{s.max_players}</span></span
              >
            </span>
          {/if}
        </button>
      </li>
    {/each}
  </ul>
{:else}
  <ul class="m-0 list-none p-0">
    {#each picks as p (p.key)}
      {@const s = servers.find(p.ip, p.port)}
      {@const k = s ? keyOf(s) : p.key}
      <li>
        <button
          class="flex w-full items-center gap-2 border-b border-border/40 px-3 py-1.5 text-left last:border-0 hover:bg-raised/60"
          onclick={() =>
            direct.load(p.ip, s?.game_port ?? p.port, s?.query_port, p.password ?? undefined)}
        >
          {#if p.fav}
            <Star class="size-3 shrink-0 text-warn" aria-label={$c.fromFavorites.value} />
          {:else}
            <History class="size-3 shrink-0 text-fg-faint" />
          {/if}
          <span class="min-w-0 flex-1">
            <span class="block truncate text-2xs text-fg">{p.name}</span>
            <span class="flex gap-2 font-mono text-3xs text-fg-faint">
              <span>{p.ip}:{s?.game_port ?? p.port}</span>
              {#if p.ts}<span class="truncate">{relative(p.ts)}</span>{/if}
            </span>
          </span>
          {#if s}<Signal ms={servers.ping.get(k)} size="xs" label={false} />{/if}
        </button>
      </li>
    {/each}
  </ul>
{/if}
