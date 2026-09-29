<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import Star from "~icons/lucide/star";
  import { ViewHeader } from "$lib/components/ui/view-header";
  import { Stat, StatStrip } from "$lib/components/ui/stat";
  import { profile } from "$lib/stores/profile.svelte";
  import { num } from "$lib/format";
  import SavedServers, { type Entry } from "./SavedServers.svelte";

  /** The servers the player starred, with their live line, head-count and details. */
  const f = useIntlayer("favorites");
  const nav = useIntlayer("nav");

  const entries = $derived(
    (profile.data?.favorites ?? []).map(
      (fav): Entry => ({ ip: fav.ip, port: fav.port, name: fav.name, password: fav.password, fav }),
    ),
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
    <ViewHeader title={$nav.favorites.value} icon={Star}>
      {#snippet stats()}
        <StatStrip>
          <Stat label={$f.statSaved.value} value={num(s.total)} tone="text-accent" />
          <Stat label={$f.statOnline.value} value={`${s.listed}/${s.total}`} />
          <Stat label={$f.statPlayers.value} value={num(s.players)} tone="text-ok" />
          <Stat label={$f.statBestPing.value} value={s.bestPing == null ? "—" : `${s.bestPing} ms`} />
          <Stat label={$f.statAvgPing.value} value={s.avgPing == null ? "—" : `${s.avgPing} ms`} />
        </StatStrip>
      {/snippet}
      {@render toolbar()}
    </ViewHeader>
  {/snippet}
</SavedServers>
