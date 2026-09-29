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
   * the server.
   */
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
    return out.slice(0, 14);
  });
</script>

{#if picks.length === 0}
  <p class="m-0 p-3 text-2xs text-fg-faint">{$c.noRecent.value}</p>
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
