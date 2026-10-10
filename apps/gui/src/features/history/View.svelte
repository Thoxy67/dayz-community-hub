<script lang="ts">
  import { dict } from "$lib/i18n";
  import History from "~icons/lucide/history";
  import Trash from "~icons/lucide/trash-2";
  import { PageHeader, Figure } from "$lib/components/app";
  import { Button } from "$lib/components/ui/button";
  import { profile } from "$lib/stores/profile.svelte";
  import { relative, dateTime } from "$lib/format";
  import SavedServers, { type Entry } from "$features/favorites/SavedServers.svelte";

  /** Every server joined, newest first: one click back into any of them. */
  const h = dict("history");
  const f = dict("favorites");
  const nav = dict("nav");

  const entries = $derived(
    (profile.data?.history ?? []).map((hist): Entry => ({
      ip: hist.ip,
      port: hist.port,
      name: hist.name,
      ts: hist.ts,
      hist,
    })),
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
  {#snippet header(s, toolbar, tools)}
    <PageHeader title={$nav.history.value}>
      {#snippet stats()}
        <Figure label={$h.statEntries.value} amount={s.total} tone="text-accent" />
        <Figure
          label={$h.statLastPlayed.value}
          value={s.lastTs ? relative(s.lastTs) : "—"}
          title={s.lastTs ? dateTime(s.lastTs) : ""}
        />
        <Figure label={$h.statThisWeek.value} amount={s.thisWeek} />
        <Figure label={$f.statOnline.value} value={`${s.listed}/${s.total}`} />
        <Figure label={$f.statPlayers.value} amount={s.players} tone="text-ok" />
        <Figure
          label={$f.statBestPing.value}
          value={s.bestPing == null ? "—" : `${s.bestPing} ms`}
        />
      {/snippet}
      <!-- The tools as in the server browser, then this list's own action
           where the browser has Refresh. -->
      {#snippet actions()}
        {@render tools()}
        {#if s.total > 0}
          <Button
            variant="danger"
            title={$h.clearAllTitle.value}
            onclick={() => profile.clearHistory()}
          >
            <Trash class="size-icon-sm" />{$h.clearAll.value}
          </Button>
        {/if}
      {/snippet}
      {@render toolbar()}
    </PageHeader>
  {/snippet}
</SavedServers>
