<script lang="ts">
  import { dict } from "$lib/i18n";
  import Star from "~icons/lucide/star";
  import { PageHeader, Figure } from "$lib/components/app";
  import { profile } from "$lib/stores/profile.svelte";
  import { num } from "$lib/format";
  import SavedServers, { type Entry } from "./SavedServers.svelte";

  /** The servers the player starred, with their live line, head-count and details. */
  const f = dict("favorites");
  const nav = dict("nav");

  const entries = $derived(
    (profile.data?.favorites ?? []).map((fav): Entry => ({
      ip: fav.ip,
      port: fav.port,
      name: fav.name,
      password: fav.password,
      fav,
    })),
  );
</script>

<SavedServers
  kind="favorites"
  view="favorites"
  {entries}
  onremove={(e) => e.fav && profile.confirmRemoveFavorite(e.fav)}
  emptyIcon={Star}
  emptyTitle={$f.noFavorites.value}
  emptyHint={$f.noFavoritesHint.value}
>
  {#snippet header(s, toolbar)}
    <PageHeader title={$nav.favorites.value}>
      {#snippet stats()}
        <Figure label={$f.statSaved.value} value={num(s.total)} tone="text-accent" />
        <Figure label={$f.statOnline.value} value={`${s.listed}/${s.total}`} />
        <Figure label={$f.statPlayers.value} value={num(s.players)} tone="text-ok" />
        <Figure
          label={$f.statBestPing.value}
          value={s.bestPing == null ? "—" : `${s.bestPing} ms`}
        />
        <Figure label={$f.statAvgPing.value} value={s.avgPing == null ? "—" : `${s.avgPing} ms`} />
      {/snippet}
      <div class="flex min-w-0 flex-wrap items-center gap-1.5">{@render toolbar()}</div>
    </PageHeader>
  {/snippet}
</SavedServers>
