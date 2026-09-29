<script lang="ts">
  import { dict } from "$lib/i18n";
  import { Players } from "$lib/components/ui/players";
  import { serverData } from "$lib/stores/server-data.svelte";
  import { servers } from "$lib/stores/servers.svelte";

  /** Who is on a server right now, from the freshest source; click to ask it directly. */
  let { ip, queryPort, compact = false }: { ip: string; queryPort: number; compact?: boolean } = $props();
  const c = dict("servers");
  const listed = $derived(servers.find(ip, queryPort));
  const n = $derived(listed ? servers.count(listed) : serverData.players(ip, queryPort));
</script>

<button
  class="justify-self-start rounded-xs px-0.5 text-left hover:bg-raised"
  title={$c.clickRefreshPlayers.value}
  onclick={(e) => {
    e.stopPropagation();
    void serverData.refreshA2s(ip, queryPort);
  }}
>
  <Players
    players={n.players}
    max={n.max}
    bots={n.bots}
    {compact}
    loading={serverData.a2s(ip, queryPort).loading}
    botsLabel={$c.bots({ count: n.bots }).value}
  />
</button>
