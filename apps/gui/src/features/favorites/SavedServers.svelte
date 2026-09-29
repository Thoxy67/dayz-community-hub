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
  import type { Component, Snippet } from "svelte";
  import { dict } from "$lib/i18n";
  import PlugZap from "~icons/lucide/plug-zap";
  import Trash from "~icons/lucide/trash-2";
  import PanelRight from "~icons/lucide/panel-right";
  import Search from "~icons/lucide/search";
  import { VirtualList } from "$lib/components/ui/virtual-list";
  import { Copy } from "$lib/components/ui/copy";
  import { Tag } from "$lib/components/ui/tag";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Split } from "$lib/components/ui/split";
  import { Input } from "$lib/components/ui/input";
  import { Chip } from "$lib/components/ui/chip";
  import { Topo } from "$lib/components/ui/topo";
  import { cn } from "$lib/cx";
  import {
    Empty,
    FavoriteButton,
    JoinButton,
    ModsCount,
    PingButton,
    PlayersButton,
    ServerFlags,
    SortHead,
    TableHead,
    TimeOfDay,
  } from "$lib/components/app";
  import ServerDetail from "$features/servers/detail/ServerDetail.svelte";
  import { app, type ViewId } from "$lib/stores/app.svelte";
  import { servers, keyOf } from "$lib/stores/servers.svelte";
  import { serverData } from "$lib/stores/server-data.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { connect } from "$lib/stores/connect.svelte";
  import { dialogs } from "$lib/stores/dialogs.svelte";
  import { dateTime, relative } from "$lib/format";

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
  let sortCol = $state<SortCol>(kind === "history" ? "recent" : "name");
  let sortAsc = $state(kind !== "history");

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
  const toggleFav = (r: Row) => void profile.toggleFavorite(r.name, r.ip, r.port);

  // ── keyboard, only while this view shows and nothing else has the keys ──
  function onkeydown(e: KeyboardEvent) {
    if (app.view !== view || dialogs.pending || connect.request) return;
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
      case "Escape":
        selectedKey = null;
        break;
      default:
        return;
    }
    e.preventDefault();
  }

  const COLS = $derived(
    kind === "history"
      ? "grid-cols-[4.75rem_6rem_minmax(0,1fr)_7rem_3rem_7.5rem_10rem]"
      : "grid-cols-[4.75rem_6rem_minmax(0,1fr)_7.5rem_3rem_10rem]",
  );
  const ROW = 50;
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
  <TableHead grid="grid items-center gap-3 {COLS}">
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
      label={`${$sv.colMap.value} · ${$sv.colTime.value}`}
      active={sortCol === "map"}
      asc={sortAsc}
      onclick={() => sortBy("map")}
    />
    <span class="text-center">{$sv.colMods.value}</span>
    {#if kind === "history"}<SortHead
        label={$h.colLastPlayed.value}
        active={sortCol === "recent"}
        asc={sortAsc}
        onclick={() => sortBy("recent")}
      />{/if}
    <span></span>
  </TableHead>
{/snippet}

{#snippet row(r: Row)}
  {@const on = selectedKey === r.key}
  {@const s = r.listed}
  {@const qp = s?.query_port ?? r.port}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div
    role="row"
    tabindex="-1"
    aria-selected={on}
    class={cn(
      "group relative grid h-full cursor-pointer items-center gap-3 border-b border-border/50 px-2 transition-colors",
      COLS,
      on ? "bg-accent/10" : "hover:bg-raised/50",
      !s && "opacity-70",
    )}
    onclick={() => (selectedKey = r.key)}
    ondblclick={() => join(r)}
  >
    {#if on}<span class="absolute inset-y-0 left-0 w-0.5 bg-accent"></span>{/if}

    <PingButton ip={r.ip} queryPort={qp} />
    <PlayersButton ip={r.ip} queryPort={qp} />

    <div class="min-w-0 overflow-hidden">
      <div class="flex min-w-0 items-center gap-1.5">
        <span class="truncate text-xs font-semibold text-fg" title={r.name}>{r.name}</span>
        <ServerFlags
          password={s?.password}
          firstPerson={s?.first_person_only}
          battleye={s?.battl_eye}
          savedPassword={!!r.password}
        />
        {#if !s}
          <Tag tone="warn" title={$f.serverOfflineHint.value}>{$f.notInList.value}</Tag>
        {/if}
      </div>
      <div class="flex min-w-0 items-center gap-2">
        <Copy
          class="shrink-0"
          text={s ? `${r.ip}:${s.game_port}` : r.key}
          title={$sv.copyIp({ address: s ? `${r.ip}:${s.game_port}` : r.key }).value}
        />
        {#if s}
          <span
            class="min-w-0 truncate font-mono text-3xs text-fg-faint"
            title="query {s.query_port} · {s.version}">q{s.query_port} · {s.version}</span
          >
        {/if}
      </div>
    </div>

    <div class="min-w-0">
      {#if s}
        <div class="truncate text-xs text-map" title={s.map}>{s.map}</div>
        {#if s.time}<TimeOfDay time={s.time} class="text-3xs" />{/if}
      {:else}
        <span class="text-2xs text-fg-faint">—</span>
      {/if}
    </div>

    <div class="flex justify-center">
      <ModsCount count={s?.mods_count ?? 0} />
    </div>

    {#if kind === "history"}
      <div class="min-w-0" title={r.ts ? dateTime(r.ts) : ""}>
        {#if r.ts}
          <div class="truncate text-2xs text-fg">{(void now, relative(r.ts))}</div>
          <div class="truncate font-mono text-3xs text-fg-faint">{dateTime(r.ts)}</div>
        {/if}
      </div>
    {/if}

    <div class="flex items-center justify-end gap-0.5">
      <div
        class={cn(
          "flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100",
          on && "opacity-100",
        )}
      >
        {#if kind === "history"}<FavoriteButton name={r.name} ip={r.ip} port={r.port} />{/if}
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
      </div>
      <JoinButton ip={r.ip} port={r.port} password={r.password} compact={!on} />
    </div>
  </div>
{/snippet}

{#snippet listPane()}
  <div class="flex min-h-0 flex-1 flex-col bg-panel">
    <VirtualList
      bind:this={list}
      items={shown}
      rowHeight={ROW}
      key={(r) => r.key}
      header={head}
      {row}
      {onrange}
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
          onclose={() => (showDetail = false)}
        />
      {/key}
    {:else}
      <div class="relative grid flex-1 place-items-center overflow-hidden p-pad">
        <Topo opacity={0.35} />
        <p class="relative m-0 max-w-56 text-center text-xs text-fg-faint">{$f.selectHint.value}</p>
      </div>
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
  {:else if showDetail}
    <Split
      id="{kind}-detail"
      initial={380}
      min={300}
      max={620}
      keep={520}
      main={listPane}
      aside={detailPane}
    />
  {:else}
    {@render listPane()}
  {/if}
</div>
