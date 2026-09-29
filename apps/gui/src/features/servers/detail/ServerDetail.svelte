<script lang="ts">
  import { dict } from "$lib/i18n";
  import LayoutDashboard from "~icons/lucide/layout-dashboard";
  import Users from "~icons/lucide/users";
  import Puzzle from "~icons/lucide/puzzle";
  import ScrollText from "~icons/lucide/scroll-text";
  import ChartLine from "~icons/lucide/chart-line";
  import { TabStrip } from "$lib/components/ui/tabs";
  import { serverData } from "$lib/stores/server-data.svelte";
  import { detailModel } from "./model.svelte";
  import { detailTab, type DetailTab } from "./tab.svelte";
  import { loadServerMods } from "./server-mods.svelte";
  import Hero from "./Hero.svelte";
  import ActionBar from "./ActionBar.svelte";
  import OverviewTab from "./OverviewTab.svelte";
  import PlayersTab from "./PlayersTab.svelte";
  import ModsTab from "./ModsTab.svelte";
  import RulesTab from "./RulesTab.svelte";
  import StatsTab from "./StatsTab.svelte";
  import ModsWarnIcon from "./ModsWarnIcon.svelte";
  import ModsErrIcon from "./ModsErrIcon.svelte";

  /**
   * Everything known about one server. The map-coloured hero says which
   * server and whether it is worth joining now; the tabs hold the detail
   * (overview, players, mods against what is installed, rules, BattleMetrics'
   * long view); the foot keeps join and the other actions in reach. Shared by
   * the server list, favourites, history and Direct Connect; works for a
   * server the public list does not have, from what it answers directly.
   */
  let {
    ip,
    port,
    name = "",
    onclose,
    focusMods = false,
  }: {
    ip: string;
    port: number;
    name?: string;
    onclose?: () => void;
    focusMods?: boolean;
  } = $props();

  const c = dict("detail");
  const m = detailModel(() => ({ ip, port, name }));

  // Ask the server itself whenever another one is shown, and fetch the mods
  // it runs; both debounced, so arrowing through a list does not query every
  // row it passes.
  $effect(() => {
    const [i, p] = [ip, port];
    const t = setTimeout(() => void serverData.refreshA2s(i, p), 150);
    return () => clearTimeout(t);
  });
  $effect(() => {
    const s = m.listed;
    if (!s || s.mods_count === 0) return;
    const [i, q] = [s.ip, s.query_port];
    const t = setTimeout(() => void loadServerMods(i, q), 180);
    return () => clearTimeout(t);
  });
  $effect(() => {
    if (!m.bmEnabled) return;
    const [i, g, q, n] = [m.ip, m.gamePort, m.queryPort, m.title];
    const t = setTimeout(() => void serverData.fetchBm(i, g, q, n), 200);
    return () => clearTimeout(t);
  });

  // `focusMods` (the mods count in a row was clicked) opens the Mods tab.
  $effect(() => {
    if (focusMods) detailTab.current = "mods";
  });
  // Stats only exist with a BattleMetrics token.
  $effect(() => {
    if (!m.bmEnabled && detailTab.current === "stats") detailTab.current = "overview";
  });

  const modsIcon = $derived(
    m.modTotals.missing.length ? ModsErrIcon : m.modTotals.stale.length ? ModsWarnIcon : Puzzle,
  );
  // Under ~480 px the icons go (except the mods warning), so the labels
  // stay whole rather than every tab reading "Pla…".
  let width = $state(0);
  const wide = $derived(width === 0 || width >= 480);
  const icon = <T,>(i: T) => (wide ? i : undefined);
  const tabs = $derived([
    { id: "overview" as DetailTab, label: $c.tabOverview.value, icon: icon(LayoutDashboard) },
    { id: "players" as DetailTab, label: $c.tabPlayers.value, icon: icon(Users), count: m.count?.players ?? m.players.length },
    {
      id: "mods" as DetailTab,
      label: $c.tabMods.value,
      icon: modsIcon === Puzzle ? icon(Puzzle) : modsIcon,
      count: m.modsCount,
    },
    { id: "rules" as DetailTab, label: $c.tabRules.value, icon: icon(ScrollText), count: m.a2s?.rules?.length ?? 0 },
    ...(m.bmEnabled ? [{ id: "stats" as DetailTab, label: $c.tabStats.value, icon: icon(ChartLine) }] : []),
  ]);
  const go = (t: DetailTab) => (detailTab.current = t);
</script>

<aside
  class="@container flex h-full min-h-0 flex-col bg-panel"
  aria-label={$c.title.value}
  bind:clientWidth={width}
>
  <Hero {m} {onclose} onmods={() => go("mods")} />

  <TabStrip
    bind:value={detailTab.current}
    {tabs}
    size="xs"
    fill={wide}
    panels="server-detail"
    aria-label={$c.sections.value}
    class="px-1"
  />

  <div
    class="flex min-h-0 flex-1 flex-col overflow-y-auto"
    id="server-detail-panel-{detailTab.current}"
    role="tabpanel"
    aria-labelledby="server-detail-tab-{detailTab.current}"
  >
    {#if detailTab.current === "overview"}
      <OverviewTab {m} {go} />
    {:else if detailTab.current === "players"}
      <PlayersTab {m} />
    {:else if detailTab.current === "mods"}
      <ModsTab {m} />
    {:else if detailTab.current === "rules"}
      <RulesTab {m} />
    {:else if detailTab.current === "stats"}
      <StatsTab ip={m.ip} port={m.gamePort} queryPort={m.queryPort} name={m.title} />
    {/if}
  </div>

  <ActionBar {m} />
</aside>
