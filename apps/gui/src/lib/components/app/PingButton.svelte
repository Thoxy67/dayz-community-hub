<script lang="ts">
  import { dict } from "$lib/i18n";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import { Signal } from "$lib/components/ui/signal";
  import { servers } from "$lib/stores/servers.svelte";

  /**
   * A server's signal, live from the ping store; click to ask again. `warn`
   * marks a "full" the last query could not confirm.
   */
  let {
    ip,
    queryPort,
    size = "sm",
    label = true,
  }: { ip: string; queryPort: number; size?: "xs" | "sm" | "md"; label?: boolean } = $props();
  const c = dict("servers");
  const key = $derived(`${ip}:${queryPort}`);
  const listed = $derived(servers.find(ip, queryPort));
  const warn = $derived.by(() => {
    if (!listed || !servers.a2sFailures.has(key)) return false;
    const n = servers.count(listed);
    return n.max > 0 && n.players >= n.max;
  });
</script>

<button
  class="flex items-center gap-1 justify-self-start rounded-xs px-0.5 hover:bg-raised"
  title={$c.clickPing.value}
  onclick={(e) => {
    e.stopPropagation();
    void servers.pingOne(ip, queryPort);
  }}
>
  <Signal ms={servers.ping.get(key)} pending={servers.pending.has(key)} {size} {label} />
  {#if warn}<TriangleAlert
      class="size-3 text-warn"
      aria-label={$c.playerCountUnverified.value}
    />{/if}
</button>
