<script lang="ts" module>
  import type { FavoriteDto, HistoryDto } from "$lib/ipc/types";

  /** A saved server: a favourite or a history entry, as the list shows it. */
  export type Entry = {
    ip: string;
    port: number;
    name: string;
    /** Favourites only: the join password kept with it. */
    password?: string | null;
    /** History only: when it was last joined (Unix seconds). */
    ts?: number;
    fav?: FavoriteDto;
    hist?: HistoryDto;
  };

  /** The figures a view header shows over the list. */
  export type Stats = {
    total: number;
    listed: number;
    players: number;
    bestPing: number | null;
    avgPing: number | null;
    lastTs: number | null;
    thisWeek: number;
  };
</script>

<script lang="ts">
  import { untrack, type Component, type Snippet } from "svelte";
  import { dict } from "$lib/i18n";
  import PlugZap from "~icons/lucide/plug-zap";
  import Trash from "~icons/lucide/trash-2";
  import PanelRight from "~icons/lucide/panel-right";
  import Search from "~icons/lucide/search";
  import { VirtualList } from "$lib/components/ui/virtual-list";
  import { Tag } from "$lib/components/ui/tag";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Chip } from "$lib/components/ui/chip";
  import ServerIcon from "~icons/lucide/server";
  import {
    Empty,
    LIST_GRID,
    LIST_GRID_EXTRA,
    LIST_NARROW_HIDDEN,
    LIST_ROW_PX,
    ServerListRow,
    SortHead,
    TableHead,
    MasterDetail,
  } from "$lib/components/app";
  import ServerDetail from "$features/servers/detail/ServerDetail.svelte";
  import { app, type ViewId } from "$lib/stores/app.svelte";
  import { servers, keyOf } from "$lib/stores/servers.svelte";
  import { serverData } from "$lib/stores/server-data.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { connect } from "$lib/stores/connect.svelte";
  import { dialogs } from "$lib/stores/dialogs.svelte";
  import { dateTime, relative } from "$lib/format";
  import { indexOf, pad, padActions } from "$lib/gamepad";

  let {
    kind,
    view,
    entries,
    onremove,
    emptyIcon,
    emptyTitle,
    emptyHint,
    header,
  }: {
    kind: "favorites" | "history";
    /** The view this list lives in: its keys only work while it shows. */
    view: ViewId;
    entries: readonly Entry[];
    onremove: (e: Entry) => void;
    emptyIcon: Component<{ class?: string }>;
    emptyTitle: string;
    emptyHint: string;
    /** The view header, given the figures worked out here. */
    header: Snippet<[stats: Stats, toolbar: Snippet]>;
  } = $props();

  const f = dict("favorites");
  const h = dict("history");
  const sv = dict("servers");
  const nav = dict("nav");
  const p = dict("pad");

  // ── rows ────────────────────────────────────────────────────────────────
  type Row = Entry & {
    key: string;
    pingKey: string;
    listed: ReturnType<typeof servers.find>;
  };

  const rows = $derived(
    entries.map((e): Row => {
      const listed = servers.find(e.ip, e.port);
      return {
        ...e,
        key: `${e.ip}:${e.port}`,
        pingKey: listed ? keyOf(listed) : `${e.ip}:${e.port}`,
        listed,
      };
    }),
  );

  const countOf = (r: Row) =>
    r.listed ? servers.count(r.listed) : serverData.players(r.ip, r.port);

  // ── search, filter, sort ────────────────────────────────────────────────
  type SortCol = "name" | "players" | "ping" | "map" | "recent";
  let query = $state("");
  let listedOnly = $state(false);
  // A list is one kind for its whole life: its first sort follows it.
  const isHistory = untrack(() => kind === "history");
  let sortCol = $state<SortCol>(isHistory ? "recent" : "name");
  let sortAsc = $state(!isHistory);

  function sortBy(col: SortCol) {
    if (sortCol === col) sortAsc = !sortAsc;
    else {
      sortCol = col;
      sortAsc = col === "name" || col === "map" || col === "ping";
    }
  }

  const pingOf = (r: Row) => {
    const ms = servers.ping.get(r.pingKey);
    return ms == null || ms >= 5000 ? Infinity : ms;
  };

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = rows.filter(
      (r) =>
        (!listedOnly || r.listed) &&
        (!q ||
          r.name.toLowerCase().includes(q) ||
          r.key.includes(q) ||
          (r.listed?.map.toLowerCase().includes(q) ?? false)),
    );
    const dir = sortAsc ? 1 : -1;
    const by: Record<SortCol, (a: Row, b: Row) => number> = {
      name: (a, b) => a.name.localeCompare(b.name),
      players: (a, b) => countOf(a).players - countOf(b).players,
      ping: (a, b) => pingOf(a) - pingOf(b),
      map: (a, b) => (a.listed?.map ?? "~").localeCompare(b.listed?.map ?? "~"),
      recent: (a, b) => (a.ts ?? 0) - (b.ts ?? 0),
    };
    return list.sort((a, b) => dir * by[sortCol](a, b));
  });

  // ── the figures ─────────────────────────────────────────────────────────
  // Re-read each minute so "3 minutes ago" and "this week" keep moving.
  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 60_000);
    return () => clearInterval(t);
  });

  const stats = $derived.by((): Stats => {
    let players = 0;
    let listed = 0;
    const pings: number[] = [];
    let lastTs: number | null = null;
    let thisWeek = 0;
    const weekAgo = now / 1000 - 7 * 86400;
    for (const r of rows) {
      if (r.listed) {
        listed++;
        players += countOf(r).players;
      }
      const p = pingOf(r);
      if (p !== Infinity) pings.push(p);
      if (r.ts != null) {
        if (lastTs === null || r.ts > lastTs) lastTs = r.ts;
        if (r.ts >= weekAgo) thisWeek++;
      }
    }
    return {
      total: rows.length,
      listed,
      players,
      bestPing: pings.length ? Math.min(...pings) : null,
      avgPing: pings.length ? Math.round(pings.reduce((a, b) => a + b, 0) / pings.length) : null,
      lastTs,
      thisWeek,
    };
  });

  // ── selection and the detail pane ───────────────────────────────────────
  let selectedKey = $state<string | null>(null);
  let showDetail = $state(true);
  const selected = $derived(shown.find((r) => r.key === selectedKey) ?? null);
  let list: { scrollToIndex: (i: number) => void } | undefined = $state();

  function select(i: number) {
    if (i < 0) return;
    const r = shown[i];
    if (!r) return;
    selectedKey = r.key;
    list?.scrollToIndex(i);
  }

  // The rail's Rejoin card opens the history on the last server played.
  $effect(() => {
    if (app.view === view && app.focus === "last" && kind === "history" && rows.length > 0) {
      sortCol = "recent";
      sortAsc = false;
      query = "";
      listedOnly = false;
      const newest = [...rows].sort((a, b) => (b.ts ?? 0) - (a.ts ?? 0))[0]!;
      selectedKey = newest.key;
      showDetail = true;
      app.focus = null;
    }
  });

  // ── pings for what is on screen ─────────────────────────────────────────
  let rangeTimer: ReturnType<typeof setTimeout> | undefined;
  function onrange(first: number, last: number) {
    clearTimeout(rangeTimer);
    const keys = shown.slice(first, last).map((r) => r.pingKey);
    rangeTimer = setTimeout(() => servers.pingVisible(keys), 200);
  }

  // ── actions ─────────────────────────────────────────────────────────────
  const join = (r: Row) => void connect.address(r.ip, r.port);
  const ping = (r: Row) =>
    void (r.listed ? servers.pingOne(r.ip, r.listed.query_port) : servers.pingOne(r.ip, r.port));
  const direct = (r: Row) =>
    connect.openInDirect(
      r.ip,
      r.listed?.game_port ?? r.port,
      r.listed?.query_port,
      r.password ?? undefined,
    );
  const share = (r: Row) =>
    void connect.copyLink({
      ip: r.ip,
      gamePort: r.listed?.game_port ?? r.port,
      queryPort: r.listed?.query_port ?? r.port,
      name: r.name,
    });
  const toggleFav = (r: Row) => void profile.toggleFavorite(r.name, r.ip, r.port);

  // ── a controller: X joins, Y stars (or unstars) the server under the focus ─
  let listEl: HTMLElement | undefined = $state();
  /** The row a pad is on, else the selected one. */
  function padRow(): Row | null {
    void pad.focused;
    const a = document.activeElement;
    const i = listEl?.contains(a) ? indexOf(a) : null;
    return (i !== null ? shown[i] : null) ?? selected;
  }
  padActions(() => view, {
    primary: {
      label: () => $p.join.value,
      when: () => padRow() !== null,
      run: () => {
        const r = padRow();
        if (r) join(r);
      },
    },
    secondary: {
      label: () => $p.favorite.value,
      when: () => padRow() !== null,
      run: () => {
        const r = padRow();
        if (r) toggleFav(r);
      },
    },
  });

  // ── keyboard, only while this view shows and nothing else has the keys ──
  function onkeydown(e: KeyboardEvent) {
    if (app.view !== view || dialogs.open || connect.request) return;
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    if (
      (e.target as HTMLElement)?.closest(
        "input, textarea, select, [contenteditable], [role=dialog]",
      )
    )
      return;
    const i = selected ? shown.indexOf(selected) : -1;
    const r = selected;
    switch (e.key) {
      case "ArrowDown":
        select(Math.min(shown.length - 1, i + 1));
        break;
      case "ArrowUp":
        select(Math.max(0, i - 1));
        break;
      case "Enter":
        if (r) join(r);
        break;
      case "Delete":
        if (r) onremove(r);
        break;
      case "f":
      case "F":
        if (r) toggleFav(r);
        break;
      case "i":
      case "I":
        showDetail = !showDetail;
        break;
      case "p":
      case "P":
        if (r) ping(r);
        break;
      case "d":
      case "D":
        if (r) direct(r);
        break;
      case "l":
      case "L":
        if (r) share(r);
        break;
      case "Escape":
        selectedKey = null;
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  const GRID = $derived(kind === "history" ? LIST_GRID_EXTRA : LIST_GRID);
  const ROW = LIST_ROW_PX;
</script>

<svelte:window {onkeydown} />

{#snippet toolbar()}
  <div class="relative min-w-52 flex-1">
    <Input
      class="w-full"
      bind:value={query}
      type="search"
      placeholder={$f.searchPlaceholder.value}
      clearLabel={$sv.clearSearch.value}
    />
  </div>
  <Chip
    active={listedOnly}
    title={$f.onlineOnlyTitle.value}
    onclick={() => (listedOnly = !listedOnly)}
  >
    {$f.onlineOnly.value}
  </Chip>
  <span class="num font-mono text-2xs text-fg-faint"
    >{shown.length}<span class="opacity-60">/{rows.length}</span></span
  >
  <span class="ml-auto hidden text-3xs text-fg-faint lg:inline">{$f.keysHint.value}</span>
  <IconButton
    icon={PanelRight}
    label={$f.detailsTitle.value}
    active={showDetail}
    onclick={() => (showDetail = !showDetail)}
  />
{/snippet}

{#snippet head()}
  <TableHead grid={GRID}>
    <span></span>
    <SortHead
      label={$sv.colPing.value}
      active={sortCol === "ping"}
      asc={sortAsc}
      onclick={() => sortBy("ping")}
    />
    <SortHead
      label={$sv.colPlayers.value}
      active={sortCol === "players"}
      asc={sortAsc}
      onclick={() => sortBy("players")}
    />
    <SortHead
      label={$sv.colServer.value}
      active={sortCol === "name"}
      asc={sortAsc}
      onclick={() => sortBy("name")}
    />
    <SortHead
      class={LIST_NARROW_HIDDEN}
      label={`${$sv.colMap.value} · ${$sv.colTime.value}`}
      active={sortCol === "map"}
      asc={sortAsc}
      onclick={() => sortBy("map")}
    />
    <span class="uppercase">{$sv.colMods.value}</span>
    {#if kind === "history"}
      <SortHead
        label={$h.colLastPlayed.value}
        active={sortCol === "recent"}
        asc={sortAsc}
        onclick={() => sortBy("recent")}
      />
    {/if}
    <span></span>
  </TableHead>
{/snippet}

{#snippet row(r: Row)}
  {@const s = r.listed}
  <ServerListRow
    ip={r.ip}
    queryPort={s?.query_port ?? r.port}
    gamePort={s?.game_port ?? r.port}
    joinPort={r.port}
    name={r.name}
    map={s?.map}
    time={s?.time}
    version={s?.version}
    environment={s?.environment}
    modsCount={s?.mods_count ?? 0}
    password={s?.password}
    firstPerson={s?.first_person_only}
    battleye={s?.battl_eye}
    official={s?.official}
    mimicsOfficial={s?.mimics_official}
    savedPassword={r.password}
    listed={!!s}
    selected={selectedKey === r.key}
    wide={kind === "history"}
    onselect={() => (selectedKey = r.key)}
    onjoin={() => join(r)}
  >
    {#snippet tag()}
      {#if !s}<Tag tone="warn" title={$f.serverOfflineHint.value}>{$f.notInList.value}</Tag>{/if}
    {/snippet}
    {#snippet extra()}
      {#if kind === "history" && r.ts}
        <div title={dateTime(r.ts)}>
          <div class="truncate text-2xs text-fg">{(void now, relative(r.ts))}</div>
          <div class="truncate font-mono text-3xs text-fg-faint">{dateTime(r.ts)}</div>
        </div>
      {/if}
    {/snippet}
    {#snippet actions()}
      <IconButton
        icon={PlugZap}
        size="icon-xs"
        label={$f.openDirect.value}
        onclick={(e) => {
          e.stopPropagation();
          direct(r);
        }}
      />
      <IconButton
        icon={Trash}
        size="icon-xs"
        label={kind === "history" ? $h.remove.value : $sv.removeFavorite.value}
        iconClass="hover:text-err"
        onclick={(e) => {
          e.stopPropagation();
          onremove(r);
        }}
      />
    {/snippet}
  </ServerListRow>
{/snippet}

{#snippet listPane()}
  <div class="@container flex min-h-0 flex-1 flex-col bg-panel" bind:this={listEl}>
    <VirtualList
      bind:this={list}
      items={shown}
      rowHeight={ROW}
      key={(r) => r.key}
      header={head}
      {row}
      {onrange}
      padCurrent={() => (selected ? shown.indexOf(selected) : -1)}
      aria-label={$nav[kind].value}
    >
      {#snippet empty()}
        <Empty icon={Search} title={$f.noMatch.value} compact class="min-h-64" />
      {/snippet}
    </VirtualList>
  </div>
{/snippet}

{#snippet detailPane()}
  <aside class="flex min-h-0 flex-col border-l border-border bg-bg">
    {#if selected}
      {#key selected.key}
        <ServerDetail
          ip={selected.ip}
          port={selected.port}
          name={selected.name}
          onclose={() => (selectedKey = null)}
        />
      {/key}
    {:else}
      <Empty icon={ServerIcon} title={$f.selectHint.value} />
    {/if}
  </aside>
{/snippet}

<div class="flex min-h-0 flex-1 flex-col">
  {@render header(stats, toolbar)}
  {#if rows.length === 0}
    <Empty icon={emptyIcon} title={emptyTitle}>
      {emptyHint}
      {#snippet action()}
        <Button variant="accent" onclick={() => app.go("servers")}>{$f.browseServers.value}</Button>
      {/snippet}
    </Empty>
  {:else}
    <MasterDetail
      id="{kind}-detail"
      show={showDetail}
      selected={selected !== null}
      initial={380}
      min={300}
      main={listPane}
      detail={detailPane}
    />
  {/if}
</div>
