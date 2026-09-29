<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import History from "~icons/lucide/history";
  import Trash from "~icons/lucide/trash-2";
  import { ViewHeader } from "$lib/components/ui/view-header";
  import { Stat, StatStrip } from "$lib/components/ui/stat";
  import { Button } from "$lib/components/ui/button";
  import { profile } from "$lib/stores/profile.svelte";
  import { num, relative, dateTime } from "$lib/format";
  import SavedServers, { type Entry } from "$features/favorites/SavedServers.svelte";

  /** Every server joined, newest first: one click back into any of them. */
  const h = useIntlayer("history");
  const f = useIntlayer("favorites");
  const nav = useIntlayer("nav");

  const entries = $derived(
    (profile.data?.history ?? []).map((hist): Entry => ({ ip: hist.ip, port: hist.port, name: hist.name, ts: hist.ts, hist })),
  );
</script>

<SavedServers
  kind="history"
  view="history"
  {entries}
  onremove={(e) => e.hist && profile.removeHistory(e.hist)}
  emptyIcon={History}
  emptyTitle={$h.noHistory.value}
  emptyHint={$h.noHistoryHint.value}
>
  {#snippet header(s, toolbar)}
    <ViewHeader title={$nav.history.value} icon={History}>
      {#snippet stats()}
        <StatStrip>
          <Stat label={$h.statEntries.value} value={num(s.total)} tone="text-accent" />
          <Stat
            label={$h.statLastPlayed.value}
            value={s.lastTs ? relative(s.lastTs) : "—"}
            title={s.lastTs ? dateTime(s.lastTs) : ""}
            grow
          />
          <Stat label={$h.statThisWeek.value} value={num(s.thisWeek)} />
          <Stat label={$f.statOnline.value} value={`${s.listed}/${s.total}`} />
          <Stat label={$f.statPlayers.value} value={num(s.players)} tone="text-ok" />
          <Stat label={$f.statBestPing.value} value={s.bestPing == null ? "—" : `${s.bestPing} ms`} />
        </StatStrip>
      {/snippet}
      {#snippet actions()}
        {#if s.total > 0}
          <Button variant="danger" size="xs" title={$h.clearAllTitle.value} onclick={() => profile.clearHistory()}>
            <Trash class="size-3" />{$h.clearAll.value}
          </Button>
        {/if}
      {/snippet}
      {@render toolbar()}
    </ViewHeader>
  {/snippet}
</SavedServers>
